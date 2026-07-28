/// <reference lib="webworker" />

import { decodeKps1 } from "../protocol/kps1";
import {
  decodeKsr1Result,
  decodeWasmReplayInfo,
  encodeKsw1Command,
  parseKpw1PublicationBundle,
  WasmWorkerCommandKind,
  type WasmReplayInfo,
} from "../protocol/workerProtocol";

export {};
declare const self: DedicatedWorkerGlobalScope;

const MAX_KSB11_BYTES = 16 * 1024 * 1024;
const MAX_REPLAY_BATCH = 256;
const MAX_REPLAY_BYTES = 1024 * 1024;

interface ReplayWasmExports {
  readonly memory: WebAssembly.Memory;
  readonly ksa64_wasm_alloc: (length: number) => number;
  readonly ksa64_wasm_dealloc: (pointer: number, length: number) => void;
  readonly ksa64_wasm_submit: (pointer: number, length: number) => number;
  readonly ksa64_wasm_open_replay: (
    pointer: number, length: number, role: number, nonceLow: number, nonceHigh: number,
  ) => number;
  readonly ksa64_wasm_result_ptr: () => number;
  readonly ksa64_wasm_result_len: () => number;
}

type ReplayWorkerCommand =
  | { readonly type: "initialize-replay"; readonly archive: ArrayBuffer; readonly role: string }
  | { readonly type: "read-replay"; readonly requestId: number; readonly cursor: string; readonly count: number }
  | { readonly type: "close" };

let wasm: ReplayWasmExports | undefined;
let replayInfo: WasmReplayInfo | undefined;
let closed = false;

function roleCode(role: string): number {
  switch (role) {
    case "observer": return 1;
    case "guided-operator": return 2;
    case "flight-controller": return 3;
    case "flight-software-engineer": return 4;
    case "sim-director": return 5;
    default: throw new Error("replay requires a supported immutable role");
  }
}

async function loadWasm(): Promise<ReplayWasmExports> {
  const response = await fetch("/wasm/ksa64-session.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error("local KSA64 WASM module is unavailable; run npm run build:wasm");
  const bytes = await response.arrayBuffer();
  const instance = await WebAssembly.instantiate(bytes, {});
  const exports = instance.instance.exports as Partial<ReplayWasmExports>;
  if (exports.memory === undefined || exports.ksa64_wasm_alloc === undefined ||
      exports.ksa64_wasm_dealloc === undefined || exports.ksa64_wasm_submit === undefined ||
      exports.ksa64_wasm_open_replay === undefined || exports.ksa64_wasm_result_ptr === undefined ||
      exports.ksa64_wasm_result_len === undefined) {
    throw new Error("local KSA64 WASM module has an incompatible replay ABI");
  }
  return exports as ReplayWasmExports;
}

function result(): Uint8Array {
  if (wasm === undefined) throw new Error("replay WASM is not initialized");
  return new Uint8Array(
    wasm.memory.buffer,
    wasm.ksa64_wasm_result_ptr(),
    wasm.ksa64_wasm_result_len(),
  ).slice();
}

function invoke(kind: WasmWorkerCommandKind, values: readonly number[] = []): Uint8Array {
  if (wasm === undefined) throw new Error("replay WASM is not initialized");
  const command = encodeKsw1Command({
    kind,
    arg0: values[0], arg1: values[1], arg2: values[2], arg3: values[3], arg4: values[4],
  });
  const pointer = wasm.ksa64_wasm_alloc(command.byteLength);
  try {
    new Uint8Array(wasm.memory.buffer, pointer, command.byteLength).set(command);
    const callStatus = wasm.ksa64_wasm_submit(pointer, command.byteLength);
    const decoded = decodeKsr1Result(result());
    if (callStatus !== 0 || decoded.status !== 0) {
      throw new Error(`replay command failed: ${decoded.status}`);
    }
    return decoded.payload;
  } finally {
    wasm.ksa64_wasm_dealloc(pointer, command.byteLength);
  }
}

function freshNonce(): bigint {
  const words = new Uint32Array(2);
  self.crypto.getRandomValues(words);
  const value = (BigInt(words[1] ?? 0) << 32n) | BigInt(words[0] ?? 0);
  return value === 0n ? 1n : value;
}

async function initialize(archive: ArrayBuffer, roleName: string): Promise<void> {
  if (archive.byteLength === 0 || archive.byteLength > MAX_KSB11_BYTES) {
    throw new Error("KSB11 replay archive exceeds the bounded object contract");
  }
  wasm = await loadWasm();
  const nonce = freshNonce();
  const pointer = wasm.ksa64_wasm_alloc(archive.byteLength);
  try {
    new Uint8Array(wasm.memory.buffer, pointer, archive.byteLength).set(new Uint8Array(archive));
    const callStatus = wasm.ksa64_wasm_open_replay(
      pointer,
      archive.byteLength,
      roleCode(roleName),
      Number(nonce & 0xffff_ffffn),
      Number(nonce >> 32n),
    );
    const decoded = decodeKsr1Result(result());
    if (callStatus !== 0 || decoded.status !== 0) {
      throw new Error(`Rust rejected the KSB11 replay: ${decoded.status}`);
    }
    replayInfo = decodeWasmReplayInfo(decoded.payload);
    if (replayInfo.sessionNonce !== nonce || replayInfo.role !== roleCode(roleName)) {
      throw new Error("Rust replay metadata violated immutable role or nonce binding");
    }
  } finally {
    wasm.ksa64_wasm_dealloc(pointer, archive.byteLength);
  }
  self.postMessage({
    type: "ready",
    sessionNonce: nonce.toString(),
    frameCount: replayInfo.frameCount.toString(),
  });
}

function readReplay(requestId: number, cursorText: string, requestedCount: number): void {
  if (wasm === undefined || replayInfo === undefined) throw new Error("replay is not ready");
  if (!Number.isSafeInteger(requestId) || requestId <= 0 ||
      !Number.isInteger(requestedCount) || requestedCount < 1 || requestedCount > MAX_REPLAY_BATCH ||
      !/^[0-9]+$/u.test(cursorText)) {
    throw new Error("invalid bounded replay request");
  }
  const cursor = BigInt(cursorText);
  if (cursor > replayInfo.frameCount) throw new Error("replay cursor is beyond end of stream");
  const payload = invoke(WasmWorkerCommandKind.ReplayRead, [
    Number(cursor & 0xffff_ffffn),
    Number(cursor >> 32n),
    requestedCount,
    MAX_REPLAY_BYTES,
  ]);
  const frames = parseKpw1PublicationBundle(payload);
  let next = cursor;
  for (const frame of frames) {
    const decoded = decodeKps1(frame, { expectedSessionNonce: replayInfo.sessionNonce });
    if (decoded.sequence !== next + 1n) throw new Error("Rust replay returned a nonconsecutive frame");
    next += 1n;
    self.postMessage(frame, [frame]);
  }
  self.postMessage({
    type: "advanced",
    requestId,
    cursor: next.toString(),
    frameCount: replayInfo.frameCount.toString(),
    complete: next === replayInfo.frameCount,
  });
}

self.onmessage = (event: MessageEvent<ReplayWorkerCommand>) => {
  switch (event.data.type) {
    case "initialize-replay":
      void initialize(event.data.archive, event.data.role).catch((error: unknown) => {
        closed = true;
        self.postMessage({ type: "rejected", detail: error instanceof Error ? error.message : "Rust rejected the replay" });
      });
      break;
    case "read-replay":
      try {
        readReplay(event.data.requestId, event.data.cursor, event.data.count);
      } catch (error) {
        closed = true;
        self.postMessage({ type: "incomplete", detail: error instanceof Error ? error.message : "replay worker failed" });
      }
      break;
    case "close":
      closed = true;
      try { invoke(WasmWorkerCommandKind.Destroy); } catch { /* worker is closing */ }
      self.close();
      break;
  }
};

self.addEventListener("error", () => { closed = true; });
