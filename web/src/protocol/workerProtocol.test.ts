import { describe, expect, it } from "vitest";
import { encodeKps1, Kps1MessageKind } from "./kps1";
import {
  decodeKsr1Result,
  decodeWasmReplayInfo,
  decodeWasmWorkerSummary,
  encodeKsw1Command,
  KSR1_MAGIC,
  KSW1_COMMAND_LENGTH,
  parseKpw1PublicationBundle,
  WasmWorkerCommandKind,
  WasmWorkerProtocolError,
} from "./workerProtocol";

function result(status: number, payload: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(12 + payload.length);
  bytes.set(new TextEncoder().encode(KSR1_MAGIC), 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(4, 1, true);
  view.setInt16(6, status, true);
  view.setUint32(8, payload.length, true);
  bytes.set(payload, 12);
  return bytes;
}

describe("KSW1/KSR1 worker protocol", () => {
  it("encodes the fixed Rust ABI command", () => {
    const bytes = encodeKsw1Command({ kind: WasmWorkerCommandKind.Action, arg0: 2, arg1: 0x1234 });
    expect(bytes).toHaveLength(KSW1_COMMAND_LENGTH);
    const view = new DataView(bytes.buffer);
    expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("KSW1");
    expect(view.getUint16(6, true)).toBe(WasmWorkerCommandKind.Action);
    expect(view.getUint32(8, true)).toBe(2);
    expect(view.getUint32(12, true)).toBe(0x1234);
  });

  it("parses bounded KPS1 publication bundles", () => {
    const frame = encodeKps1({
      kind: Kps1MessageKind.Snapshot, flags: 1, sessionNonce: 7n, sequence: 1n, correlationId: 0n, payload: new Uint8Array(),
    });
    const bundle = new Uint8Array(8 + 4 + frame.byteLength);
    bundle.set(new TextEncoder().encode("KPW1"), 0);
    const view = new DataView(bundle.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, 1, true);
    view.setUint32(8, frame.byteLength, true);
    bundle.set(frame, 12);
    const parsed = parseKpw1PublicationBundle(bundle);
    expect(parsed).toHaveLength(1);
    expect(new Uint8Array(parsed[0] ?? new ArrayBuffer(0))).toEqual(frame);
  });

  it("rejects malformed results and publication bundles", () => {
    expect(() => decodeKsr1Result(new Uint8Array([1, 2]))).toThrow(WasmWorkerProtocolError);
    expect(() => parseKpw1PublicationBundle(new Uint8Array([1, 2]))).toThrow(WasmWorkerProtocolError);
    const malformed = new Uint8Array(12);
    malformed.set(new TextEncoder().encode("KPW1"));
    new DataView(malformed.buffer).setUint16(4, 1, true);
    new DataView(malformed.buffer).setUint16(6, 1, true);
    expect(() => parseKpw1PublicationBundle(malformed)).toThrow(WasmWorkerProtocolError);
  });

  it("preserves explicit incomplete worker failures", () => {
    const decoded = decodeKsr1Result(result(-11, new Uint8Array()));
    expect(decoded.status).toBe(-11);
    expect(decoded.payload).toHaveLength(0);
  });

  it("decodes strict Rust replay metadata", () => {
    const bytes = new Uint8Array(72);
    const view = new DataView(bytes.buffer);
    bytes.set(new TextEncoder().encode("KPRI"));
    view.setUint16(4, 1, true); view.setUint16(6, 72, true);
    view.setBigUint64(8, 21_600n, true);
    view.setUint32(16, 1, true); view.setUint32(20, 2, true); view.setUint32(24, 3, true);
    bytes[28] = 2; view.setBigUint64(32, 99n, true); bytes.fill(0xab, 40);
    expect(decodeWasmReplayInfo(bytes)).toMatchObject({
      frameCount: 21_600n, definitionIdentity: 1, actionIdentity: 2,
      evidenceIdentity: 3, role: 2, sessionNonce: 99n,
    });
    bytes[29] = 1;
    expect(() => decodeWasmReplayInfo(bytes)).toThrow("unsupported replay metadata");
  });

  it("encodes stateless replay info/read commands", () => {
    const info = encodeKsw1Command({ kind: WasmWorkerCommandKind.ReplayInfo });
    const read = encodeKsw1Command({ kind: WasmWorkerCommandKind.ReplayRead, arg0: 7, arg2: 64 });
    expect(new DataView(info.buffer).getUint16(6, true)).toBe(12);
    expect(new DataView(read.buffer).getUint16(6, true)).toBe(13);
    expect(new DataView(read.buffer).getUint32(8, true)).toBe(7);
    expect(new DataView(read.buffer).getUint32(16, true)).toBe(64);
  });

  it("decodes the sealed-evidence summary", () => {
    const bytes = new Uint8Array(12);
    const view = new DataView(bytes.buffer);
    view.setUint32(0, 21591, true);
    view.setUint32(4, 4, true);
    view.setUint32(8, 2911464, true);
    expect(decodeWasmWorkerSummary(bytes)).toEqual({ releaseEpoch: 21591, acceptedActions: 4, evidenceLength: 2911464 });
  });
});
