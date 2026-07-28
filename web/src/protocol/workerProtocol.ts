export const KSW1_MAGIC = "KSW1";
export const KSR1_MAGIC = "KSR1";
export const KPW1_MAGIC = "KPW1";
export const WASM_WORKER_ABI_MAJOR = 1;
export const KSW1_COMMAND_LENGTH = 32;
export const KSR1_HEADER_LENGTH = 12;

export enum WasmWorkerCommandKind {
  Catalog = 1,
  Start = 2,
  Prepare = 3,
  Pace = 4,
  Advance = 5,
  Poll = 6,
  Action = 7,
  Evidence = 8,
  RunScripted = 9,
  Destroy = 10,
  Summary = 11,
  ReplayInfo = 12,
  ReplayRead = 13,
}

export interface WasmWorkerCommand {
  readonly kind: WasmWorkerCommandKind;
  readonly arg0?: number;
  readonly arg1?: number;
  readonly arg2?: number;
  readonly arg3?: number;
  readonly arg4?: number;
}

export class WasmWorkerProtocolError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WasmWorkerProtocolError";
  }
}

function magic(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes.subarray(0, 4));
}

function u32(value: number | undefined): number {
  if (value === undefined) return 0;
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new RangeError("worker command arguments must be u32");
  }
  return value;
}

export function encodeKsw1Command(command: WasmWorkerCommand): Uint8Array {
  if (!Number.isInteger(command.kind) || command.kind < 1 || command.kind > 13) {
    throw new RangeError("unknown worker command kind");
  }
  const bytes = new Uint8Array(KSW1_COMMAND_LENGTH);
  const view = new DataView(bytes.buffer);
  bytes.set(new TextEncoder().encode(KSW1_MAGIC), 0);
  view.setUint16(4, WASM_WORKER_ABI_MAJOR, true);
  view.setUint16(6, command.kind, true);
  view.setUint32(8, u32(command.arg0), true);
  view.setUint32(12, u32(command.arg1), true);
  view.setUint32(16, u32(command.arg2), true);
  view.setUint32(20, u32(command.arg3), true);
  view.setUint32(24, u32(command.arg4), true);
  return bytes;
}

export interface WasmWorkerResult {
  readonly status: number;
  readonly payload: Uint8Array;
}

export function decodeKsr1Result(input: ArrayBuffer | Uint8Array): WasmWorkerResult {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.byteLength < KSR1_HEADER_LENGTH || magic(bytes) !== KSR1_MAGIC) {
    throw new WasmWorkerProtocolError("invalid KSR1 worker result");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint16(4, true) !== WASM_WORKER_ABI_MAJOR) {
    throw new WasmWorkerProtocolError("unsupported KSR1 worker version");
  }
  const payloadLength = view.getUint32(8, true);
  if (payloadLength !== bytes.byteLength - KSR1_HEADER_LENGTH) {
    throw new WasmWorkerProtocolError("KSR1 result length mismatch");
  }
  return { status: view.getInt16(6, true), payload: bytes.slice(KSR1_HEADER_LENGTH) };
}

export function parseKpw1PublicationBundle(input: ArrayBuffer | Uint8Array): readonly ArrayBuffer[] {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.byteLength < 8 || magic(bytes) !== KPW1_MAGIC) {
    throw new WasmWorkerProtocolError("invalid KPW1 publication bundle");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint16(4, true) !== WASM_WORKER_ABI_MAJOR) {
    throw new WasmWorkerProtocolError("unsupported KPW1 publication version");
  }
  const count = view.getUint16(6, true);
  let at = 8;
  const frames: ArrayBuffer[] = [];
  for (let index = 0; index < count; index += 1) {
    if (at + 4 > bytes.byteLength) throw new WasmWorkerProtocolError("truncated KPW1 frame length");
    const length = view.getUint32(at, true);
    at += 4;
    if (length === 0 || at + length > bytes.byteLength) {
      throw new WasmWorkerProtocolError("truncated KPW1 frame");
    }
    frames.push(bytes.slice(at, at + length).buffer);
    at += length;
  }
  if (at !== bytes.byteLength) throw new WasmWorkerProtocolError("trailing KPW1 bytes");
  return frames;
}

export const WASM_REPLAY_INFO_LENGTH = 72;

export interface WasmReplayInfo {
  readonly frameCount: bigint;
  readonly definitionIdentity: number;
  readonly actionIdentity: number;
  readonly evidenceIdentity: number;
  readonly role: number;
  readonly sessionNonce: bigint;
  readonly manifestSha256: Uint8Array;
}

export function decodeWasmReplayInfo(payload: Uint8Array): WasmReplayInfo {
  if (payload.byteLength !== WASM_REPLAY_INFO_LENGTH || magic(payload) !== "KPRI") {
    throw new WasmWorkerProtocolError("invalid replay metadata");
  }
  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  if (view.getUint16(4, true) !== WASM_WORKER_ABI_MAJOR ||
      view.getUint16(6, true) !== WASM_REPLAY_INFO_LENGTH ||
      payload.subarray(29, 32).some((value) => value !== 0)) {
    throw new WasmWorkerProtocolError("unsupported replay metadata");
  }
  const value: WasmReplayInfo = {
    frameCount: view.getBigUint64(8, true),
    definitionIdentity: view.getUint32(16, true),
    actionIdentity: view.getUint32(20, true),
    evidenceIdentity: view.getUint32(24, true),
    role: payload[28] ?? 0,
    sessionNonce: view.getBigUint64(32, true),
    manifestSha256: payload.slice(40, 72),
  };
  if (value.frameCount === 0n || value.definitionIdentity === 0 ||
      value.actionIdentity === 0 || value.evidenceIdentity === 0 ||
      value.role < 1 || value.role > 5 || value.sessionNonce === 0n ||
      value.manifestSha256.every((byte) => byte === 0)) {
    throw new WasmWorkerProtocolError("invalid replay metadata identity");
  }
  return value;
}

export interface WasmWorkerSummary {
  readonly releaseEpoch: number;
  readonly acceptedActions: number;
  readonly evidenceLength: number;
}

export function decodeWasmWorkerSummary(payload: Uint8Array): WasmWorkerSummary {
  if (payload.byteLength !== 12) throw new WasmWorkerProtocolError("invalid worker summary");
  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  return {
    releaseEpoch: view.getUint32(0, true),
    acceptedActions: view.getUint32(4, true),
    evidenceLength: view.getUint32(8, true),
  };
}
