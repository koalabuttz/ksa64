/// <reference lib="webworker" />

import { GLOBAL_DISPLAY_V1_CAPABILITY } from "../protocol/globalDisplay";
import { decodeKps1 } from "../protocol/kps1";
import { decodePresentationPayload, type ActionProposalView } from "../protocol/presentation";
import {
  decodeKsr1Result,
  decodeWasmReplayInfo,
  encodeKsw1Command,
  parseKpw1PublicationBundle,
  WasmWorkerCommandKind,
} from "../protocol/workerProtocol";

export {};

declare const self: DedicatedWorkerGlobalScope;

interface WasmExports {
  readonly memory: WebAssembly.Memory;
  readonly ksa64_wasm_alloc: (length: number) => number;
  readonly ksa64_wasm_dealloc: (pointer: number, length: number) => void;
  readonly ksa64_wasm_submit: (pointer: number, length: number) => number;
  readonly ksa64_wasm_result_ptr: () => number;
  readonly ksa64_wasm_result_len: () => number;
}

type WorkerCommand =
  | { readonly type: "initialize"; readonly connection: { readonly role?: string }; readonly experience?: "gnss-loss" | "nominal-global" }
  | { readonly type: "action"; readonly action: { readonly kind?: string } }
  | { readonly type: "pace"; readonly pace: "fast" | "realtime" }
  | { readonly type: "close" };

let wasmSessionNonce = 0n;
const RELEASE_PERIOD_MILLIS = 31.25;
let wasm: WasmExports | undefined;
let timer: number | undefined;
let ticking = false;
let closed = false;
let completed = false;
let clientActionSequence = 1;
let proposal: ActionProposalView | undefined;
let activePace: "fast" | "realtime" = "realtime";
let fastGateHeld = false;
let nominalReplay = false;
let nominalReplayCursor = 0n;
let nominalReplayFrameCount = 0n;

function roleCode(role: string | undefined): number {
  switch (role) {
    case "observer": return 1;
    case "guided-operator": return 2;
    case "flight-controller": return 3;
    case "flight-software-engineer": return 4;
    case "sim-director": return 5;
    default: throw new Error("local authority requires a supported immutable role");
  }
}

async function loadWasm(): Promise<WasmExports> {
  const response = await fetch("/wasm/ksa64-session.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error("local KSA64 WASM module is unavailable; run npm run build:wasm");
  const bytes = await response.arrayBuffer();
  const instance = await WebAssembly.instantiate(bytes, {});
  const exports = instance.instance.exports as Partial<WasmExports>;
  if (
    exports.memory === undefined || exports.ksa64_wasm_alloc === undefined ||
    exports.ksa64_wasm_dealloc === undefined || exports.ksa64_wasm_submit === undefined ||
    exports.ksa64_wasm_result_ptr === undefined || exports.ksa64_wasm_result_len === undefined
  ) throw new Error("local KSA64 WASM module has an incompatible ABI");
  return exports as WasmExports;
}

function invoke(kind: WasmWorkerCommandKind, values: readonly number[] = []): Uint8Array {
  if (wasm === undefined) throw new Error("local authority is not initialized");
  const command = encodeKsw1Command({ kind, arg0: values[0], arg1: values[1], arg2: values[2], arg3: values[3], arg4: values[4] });
  const pointer = wasm.ksa64_wasm_alloc(command.byteLength);
  try {
    new Uint8Array(wasm.memory.buffer, pointer, command.byteLength).set(command);
    const submitStatus = wasm.ksa64_wasm_submit(pointer, command.byteLength);
    const raw = new Uint8Array(wasm.memory.buffer, wasm.ksa64_wasm_result_ptr(), wasm.ksa64_wasm_result_len()).slice();
    const result = decodeKsr1Result(raw);
    if (submitStatus !== 0 || result.status !== 0) throw new Error("local authority command failed: " + result.status);
    return result.payload;
  } finally {
    wasm.ksa64_wasm_dealloc(pointer, command.byteLength);
  }
}

function publishPoll(): boolean {
  const payload = invoke(WasmWorkerCommandKind.Poll);
  proposal = undefined;
  for (const frame of parseKpw1PublicationBundle(payload)) {
    const decoded = decodeKps1(frame, { expectedSessionNonce: wasmSessionNonce });
    const presentation = decodePresentationPayload(decoded);
    if (presentation.kind === "action-proposal") proposal = presentation.value;
    if (presentation.kind === "snapshot" && presentation.value.lifecycle === 5) completed = true;
    self.postMessage(frame, [frame]);
  }
  return proposal !== undefined;
}

function scheduleTick(): void {
  if (closed || completed || nominalReplay) return;
  timer = self.setTimeout(() => {
    void tick();
  }, activePace === "fast" ? 1 : RELEASE_PERIOD_MILLIS);
}

async function tick(): Promise<void> {
  if (closed || completed || ticking) return;
  ticking = true;
  let actionGate = false;
  try {
    invoke(WasmWorkerCommandKind.Advance, [activePace === "fast" ? 32 : 1]);
    actionGate = publishPoll();
  } catch (error) {
    closed = true;
    self.postMessage({ type: "incomplete", detail: error instanceof Error ? error.message : "local authority worker failed" });
    return;
  } finally {
    ticking = false;
  }
  fastGateHeld = activePace === "fast" && actionGate;
  if (!fastGateHeld) scheduleTick();
}

function freshSessionNonce(): bigint {
  const words = new Uint32Array(2);
  self.crypto.getRandomValues(words);
  const value = (BigInt(words[1] ?? 0) << 32n) | BigInt(words[0] ?? 0);
  return value === 0n ? 1n : value;
}

function pumpNominalReplay(): void {
  if (closed || completed || !nominalReplay) return;
  try {
    const payload = invoke(WasmWorkerCommandKind.ReplayRead, [
      Number(nominalReplayCursor & 0xffff_ffffn),
      Number(nominalReplayCursor >> 32n),
      256,
      1024 * 1024,
    ]);
    const frames = parseKpw1PublicationBundle(payload);
    if (frames.length === 0 && nominalReplayCursor !== nominalReplayFrameCount) {
      throw new Error("nominal replay made no bounded progress");
    }
    for (const frame of frames) {
      const decoded = decodeKps1(frame, { expectedSessionNonce: wasmSessionNonce });
      nominalReplayCursor += 1n;
      if (decoded.sequence !== nominalReplayCursor) {
        throw new Error("nominal replay returned a nonconsecutive frame");
      }
      self.postMessage(frame, [frame]);
    }
    completed = nominalReplayCursor === nominalReplayFrameCount;
    if (!completed) self.setTimeout(pumpNominalReplay, 0);
  } catch (error) {
    closed = true;
    self.postMessage({ type: "incomplete", detail: error instanceof Error ? error.message : "nominal replay failed" });
  }
}

async function initialize(
  connection: { readonly role?: string },
  experience: "gnss-loss" | "nominal-global" = "gnss-loss",
): Promise<void> {
  wasmSessionNonce = freshSessionNonce();
  wasm = await loadWasm();
  invoke(WasmWorkerCommandKind.Catalog);
  if (experience === "nominal-global") {
    const metadata = decodeWasmReplayInfo(invoke(WasmWorkerCommandKind.OpenNominalGlobalReplay, [
      roleCode(connection.role),
      Number(wasmSessionNonce & 0xffff_ffffn),
      Number(wasmSessionNonce >> 32n),
      Number(GLOBAL_DISPLAY_V1_CAPABILITY & 0xffff_ffffn),
      Number(GLOBAL_DISPLAY_V1_CAPABILITY >> 32n),
    ]));
    if (metadata.sessionNonce !== wasmSessionNonce || metadata.role !== roleCode(connection.role)) {
      throw new Error("nominal replay violated immutable role or nonce binding");
    }
    nominalReplay = true;
    nominalReplayCursor = 0n;
    nominalReplayFrameCount = metadata.frameCount;
    self.postMessage({ type: "ready", sessionNonce: wasmSessionNonce.toString() });
    self.setTimeout(pumpNominalReplay, 0);
    return;
  }
  invoke(WasmWorkerCommandKind.Start, [
    roleCode(connection.role),
    Number(wasmSessionNonce & 0xffff_ffffn),
    Number(wasmSessionNonce >> 32n),
    Number(GLOBAL_DISPLAY_V1_CAPABILITY & 0xffff_ffffn),
    Number(GLOBAL_DISPLAY_V1_CAPABILITY >> 32n),
  ]);
  invoke(WasmWorkerCommandKind.Prepare);
  invoke(WasmWorkerCommandKind.Pace, [2]);
  self.postMessage({ type: "ready", sessionNonce: wasmSessionNonce.toString() });
  publishPoll();
  scheduleTick();
}

function setPace(pace: "fast" | "realtime"): void {
  activePace = pace;
  if (nominalReplay) return;
  fastGateHeld = false;
  invoke(WasmWorkerCommandKind.Pace, [pace === "fast" ? 1 : 2]);
  if (timer !== undefined) self.clearTimeout(timer);
  scheduleTick();
}

function actionCode(kind: string | undefined): number {
  switch (kind) {
    case "review": return 1;
    case "stage": return 2;
    case "commit": return 3;
    case "cancel": return 4;
    default: throw new Error("unsupported high-level action");
  }
}

function submitAction(action: { readonly kind?: string }): void {
  if (proposal === undefined) throw new Error("no current action proposal");
  const code = actionCode(action.kind);
  const payload = invoke(WasmWorkerCommandKind.Action, [
    code,
    proposal.proposalIdentity,
    proposal.loadIdentity,
    proposal.activationEpoch,
    clientActionSequence,
  ]);
  clientActionSequence += 1;
  for (const frame of parseKpw1PublicationBundle(payload)) {
    decodeKps1(frame, { expectedSessionNonce: wasmSessionNonce });
    self.postMessage(frame, [frame]);
  }
  const wasGateHeld = fastGateHeld;
  const actionGate = publishPoll();
  fastGateHeld = activePace === "fast" && actionGate;
  if (wasGateHeld && !fastGateHeld) scheduleTick();
}

self.onmessage = (event: MessageEvent<WorkerCommand>) => {
  switch (event.data.type) {
    case "initialize":
      void initialize(event.data.connection, event.data.experience).catch((error: unknown) => {
        closed = true;
        self.postMessage({ type: "incomplete", detail: error instanceof Error ? error.message : "local authority initialization failed" });
      });
      break;
    case "action":
      try {
        submitAction(event.data.action);
      } catch (error) {
        self.postMessage({ type: "error", detail: error instanceof Error ? error.message : "local action rejected" });
      }
      break;
    case "pace":
      try { setPace(event.data.pace); } catch (error) {
        self.postMessage({ type: "error", detail: error instanceof Error ? error.message : "local pace rejected" });
      }
      break;
    case "close":
      closed = true;
      if (timer !== undefined) self.clearTimeout(timer);
      try { invoke(WasmWorkerCommandKind.Destroy); } catch { /* worker is closing */ }
      self.close();
      break;
  }
};
