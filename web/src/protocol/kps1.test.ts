import { describe, expect, it } from "vitest";
import vectors from "./kps1.vectors.json";
import {
  KPS1_HEADER_LENGTH,
  KPS1_MAX_PAYLOAD,
  Kps1MessageKind,
  Kps1SequenceValidator,
  bytesToHex,
  decodeKps1,
  encodeKps1,
  hexToBytes,
} from "./kps1";

describe("KPS1 protocol", () => {
  it("matches the frozen TypeScript vector", () => {
    const payload = new TextEncoder().encode(vectors.payloadUtf8);
    const encoded = encodeKps1({
      kind: vectors.messageKind as Kps1MessageKind,
      flags: vectors.flags,
      sessionNonce: BigInt(vectors.sessionNonce),
      sequence: BigInt(vectors.sequence),
      correlationId: BigInt(vectors.correlationId),
      payload,
    });
    expect(bytesToHex(encoded)).toBe(vectors.encodedHex);
    expect(vectors.headerLength).toBe(KPS1_HEADER_LENGTH);
    expect(encoded.byteLength).toBe(vectors.headerLength + payload.byteLength);

    const decoded = decodeKps1(hexToBytes(vectors.encodedHex), {
      expectedSessionNonce: BigInt(vectors.sessionNonce),
    });
    expect(decoded.kind).toBe(Kps1MessageKind.ActionReceipt);
    expect(decoded.sequence).toBe(BigInt(vectors.sequence));
    expect(new TextDecoder().decode(decoded.payload)).toBe(vectors.payloadUtf8);
  });

  it("uses the finalized Rust message-kind assignments", () => {
    for (const [name, value] of Object.entries(vectors.messageKinds)) {
      expect(Kps1MessageKind[name as keyof typeof Kps1MessageKind]).toBe(value);
    }
    expect(Kps1MessageKind.EventBatch).toBe(0x0107);
    expect(Kps1MessageKind.ActionProposal).toBe(0x0202);
    expect(Kps1MessageKind.GlobalDisplayCursorState).toBe(0x0115);
    expect(Kps1MessageKind.GlobalDisplayRangeRequest).toBe(0x0210);
  });

  it("enforces Rust-compatible correlation and handshake rules", () => {
    expect(() =>
      encodeKps1({
        kind: Kps1MessageKind.Snapshot,
        flags: 0,
        sessionNonce: 1n,
        sequence: 1n,
        correlationId: 1n,
        payload: new Uint8Array(),
      }),
    ).toThrowError(expect.objectContaining({ code: "invalid-correlation" }));
    expect(() =>
      encodeKps1({
        kind: Kps1MessageKind.ActionIntent,
        flags: 0,
        sessionNonce: 1n,
        sequence: 1n,
        correlationId: 0n,
        payload: new Uint8Array(),
      }),
    ).toThrowError(expect.objectContaining({ code: "invalid-correlation" }));
    expect(() =>
      encodeKps1({
        kind: Kps1MessageKind.HandshakeRequest,
        flags: 0,
        sessionNonce: 0n,
        sequence: 1n,
        correlationId: 1n,
        payload: new Uint8Array(),
      }),
    ).not.toThrow();
  });

  it("rejects corruption before yielding a frame", () => {
    const encoded = hexToBytes(vectors.encodedHex);
    encoded[encoded.length - 1] = (encoded[encoded.length - 1] ?? 0) ^ 0x80;
    expect(() => decodeKps1(encoded)).toThrowError(
      expect.objectContaining({ code: "crc-mismatch" }),
    );
  });

  it("rejects unknown required flags", () => {
    expect(() =>
      encodeKps1({
        kind: Kps1MessageKind.Snapshot,
        flags: 0x00010000,
        sessionNonce: 1n,
        sequence: 1n,
        correlationId: 0n,
        payload: new Uint8Array(),
      }),
    ).toThrowError(expect.objectContaining({ code: "unknown-required-flags" }));
  });

  it("rejects stale sessions and non-exact lengths", () => {
    const encoded = hexToBytes(vectors.encodedHex);
    expect(() => decodeKps1(encoded, { expectedSessionNonce: 99n })).toThrowError(
      expect.objectContaining({ code: "invalid-session" }),
    );
    const withTrailingByte = new Uint8Array(encoded.length + 1);
    withTrailingByte.set(encoded);
    expect(() => decodeKps1(withTrailingByte)).toThrowError(
      expect.objectContaining({ code: "length-mismatch" }),
    );
  });

  it("enforces payload limits", () => {
    expect(() =>
      encodeKps1({
        kind: Kps1MessageKind.Snapshot,
        flags: 0,
        sessionNonce: 1n,
        sequence: 1n,
        correlationId: 0n,
        payload: new Uint8Array(KPS1_MAX_PAYLOAD + 1),
      }),
    ).toThrowError(expect.objectContaining({ code: "payload-too-large" }));
  });

  it("tracks exact ordered sequences", () => {
    const validator = new Kps1SequenceValidator(7n, 40n);
    const frame = {
      kind: Kps1MessageKind.TimelineEvent,
      flags: 0,
      sessionNonce: 7n,
      sequence: 40n,
      correlationId: 0n,
      payload: new Uint8Array(),
    } as const;
    validator.accept(frame);
    expect(validator.cursor()).toBe(40n);
    expect(() => validator.accept({ ...frame, sequence: 42n })).toThrowError(
      expect.objectContaining({ code: "invalid-sequence" }),
    );
  });
});
