export const KPS1_MAGIC = "KPS1";
export const KPS1_HEADER_LENGTH = 48;
export const KPS1_MAJOR = 1;
export const KPS1_MINOR = 0;
export const KPS1_MAX_PAYLOAD = 256 * 1024;
export const KPS1_EVIDENCE_CHUNK_MAX = 64 * 1024;
export const KPS1_EVIDENCE_OBJECT_MAX = 16 * 1024 * 1024;

const REQUIRED_FLAG_MASK = 0xffff0000;
const MAX_U64 = (1n << 64n) - 1n;

export enum Kps1Flag {
  Response = 1 << 0,
  Final = 1 << 1,
  Coalesced = 1 << 2,
  Lossy = 1 << 3,
  ResyncRequired = 1 << 4,
}

export enum Kps1MessageKind {
  HandshakeRequest = 0x0001,
  HandshakeResponse = 0x0002,
  LifecycleControl = 0x0010,
  PaceControl = 0x0011,
  ReplayControl = 0x0012,
  Snapshot = 0x0100,
  Procedure = 0x0101,
  Disposition = 0x0102,
  PredictionPath = 0x0103,
  TimelineEvent = 0x0104,
  ReleaseSampleBatch = 0x0105,
  TransportStatus = 0x0106,
  EventBatch = 0x0107,
  GlobalDisplayDefinition = 0x0110,
  GlobalDisplaySampleBatch = 0x0111,
  GlobalDisplayPathChunk = 0x0112,
  GlobalDisplayTransition = 0x0113,
  GlobalReplayIndex = 0x0114,
  ActionIntent = 0x0200,
  ActionReceipt = 0x0201,
  ActionProposal = 0x0202,
  EvidenceMetadata = 0x0300,
  EvidenceChunk = 0x0301,
  Error = 0x7fff,
}

const MESSAGE_KINDS = new Set<number>(
  Object.values(Kps1MessageKind).filter((value): value is number => typeof value === "number"),
);

export interface Kps1Frame {
  readonly kind: Kps1MessageKind;
  readonly flags: number;
  readonly sessionNonce: bigint;
  readonly sequence: bigint;
  readonly correlationId: bigint;
  readonly payload: Uint8Array;
}

export interface Kps1DecodeOptions {
  readonly expectedSessionNonce?: bigint;
  readonly maximumPayload?: number;
}

export type Kps1ErrorCode =
  | "bad-magic"
  | "bad-version"
  | "bad-header-length"
  | "unknown-message-kind"
  | "unknown-required-flags"
  | "invalid-session"
  | "invalid-sequence"
  | "invalid-correlation"
  | "payload-too-large"
  | "length-mismatch"
  | "crc-mismatch";

export class Kps1ProtocolError extends Error {
  constructor(
    readonly code: Kps1ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "Kps1ProtocolError";
  }
}

const crcTable = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < table.length; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = (value & 1) !== 0 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

export function crc32(parts: readonly Uint8Array[]): number {
  let crc = 0xffffffff;
  for (const bytes of parts) {
    for (const byte of bytes) {
      crc = (crcTable[(crc ^ byte) & 0xff] ?? 0) ^ (crc >>> 8);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function assertU64(value: bigint, field: string): void {
  if (value < 0n || value > MAX_U64) {
    throw new RangeError(`${field} must fit in an unsigned 64-bit integer`);
  }
}

function correlationRequiresNonzero(kind: Kps1MessageKind): boolean {
  return (
    kind === Kps1MessageKind.HandshakeRequest ||
    kind === Kps1MessageKind.HandshakeResponse ||
    kind === Kps1MessageKind.LifecycleControl ||
    kind === Kps1MessageKind.PaceControl ||
    kind === Kps1MessageKind.ReplayControl ||
    kind === Kps1MessageKind.ActionIntent
  );
}

function correlationRequiresZero(kind: Kps1MessageKind): boolean {
  return (
    kind === Kps1MessageKind.Snapshot ||
    kind === Kps1MessageKind.Procedure ||
    kind === Kps1MessageKind.Disposition ||
    kind === Kps1MessageKind.PredictionPath ||
    kind === Kps1MessageKind.TimelineEvent ||
    kind === Kps1MessageKind.ReleaseSampleBatch ||
    kind === Kps1MessageKind.TransportStatus ||
    kind === Kps1MessageKind.EventBatch ||
    kind === Kps1MessageKind.ActionProposal ||
    kind === Kps1MessageKind.EvidenceMetadata ||
    kind === Kps1MessageKind.EvidenceChunk
  );
}

function payloadLimit(kind: Kps1MessageKind, requestedMaximum: number): number {
  return kind === Kps1MessageKind.EvidenceChunk
    ? Math.min(requestedMaximum, KPS1_EVIDENCE_CHUNK_MAX)
    : requestedMaximum;
}

function validateFrame(frame: Kps1Frame, maximumPayload = KPS1_MAX_PAYLOAD): void {
  if (!MESSAGE_KINDS.has(frame.kind)) {
    throw new Kps1ProtocolError("unknown-message-kind", `unknown KPS1 message kind ${frame.kind}`);
  }
  if ((frame.flags & REQUIRED_FLAG_MASK) !== 0) {
    throw new Kps1ProtocolError(
      "unknown-required-flags",
      `unsupported required KPS1 flags 0x${(frame.flags & REQUIRED_FLAG_MASK).toString(16)}`,
    );
  }
  if (frame.sessionNonce === 0n && frame.kind !== Kps1MessageKind.HandshakeRequest) {
    throw new Kps1ProtocolError(
      "invalid-session",
      "only a KPS1 handshake request may use a zero session nonce",
    );
  }
  if (frame.sequence === 0n) {
    throw new Kps1ProtocolError("invalid-sequence", "KPS1 sequence must be nonzero");
  }
  assertU64(frame.sessionNonce, "sessionNonce");
  assertU64(frame.sequence, "sequence");
  assertU64(frame.correlationId, "correlationId");
  if (correlationRequiresNonzero(frame.kind) && frame.correlationId === 0n) {
    throw new Kps1ProtocolError(
      "invalid-correlation",
      "this KPS1 message requires a nonzero correlation ID",
    );
  }
  if (correlationRequiresZero(frame.kind) && frame.correlationId !== 0n) {
    throw new Kps1ProtocolError(
      "invalid-correlation",
      "this KPS1 publication requires a zero correlation ID",
    );
  }
  if (frame.payload.byteLength > payloadLimit(frame.kind, maximumPayload)) {
    throw new Kps1ProtocolError(
      "payload-too-large",
      `KPS1 payload is ${frame.payload.byteLength} bytes`,
    );
  }
}

export function encodeKps1(frame: Kps1Frame): Uint8Array {
  validateFrame(frame);
  const encoded = new Uint8Array(KPS1_HEADER_LENGTH + frame.payload.byteLength);
  const view = new DataView(encoded.buffer);
  encoded.set(new TextEncoder().encode(KPS1_MAGIC), 0);
  view.setUint16(4, KPS1_MAJOR, true);
  view.setUint16(6, KPS1_MINOR, true);
  view.setUint16(8, KPS1_HEADER_LENGTH, true);
  view.setUint16(10, frame.kind, true);
  view.setUint32(12, frame.flags >>> 0, true);
  view.setBigUint64(16, frame.sessionNonce, true);
  view.setBigUint64(24, frame.sequence, true);
  view.setBigUint64(32, frame.correlationId, true);
  view.setUint32(40, frame.payload.byteLength, true);
  encoded.set(frame.payload, KPS1_HEADER_LENGTH);
  view.setUint32(
    44,
    crc32([encoded.subarray(0, 44), encoded.subarray(KPS1_HEADER_LENGTH)]),
    true,
  );
  return encoded;
}

export function decodeKps1(
  encoded: ArrayBuffer | Uint8Array,
  options: Kps1DecodeOptions = {},
): Kps1Frame {
  const bytes =
    encoded instanceof Uint8Array
      ? new Uint8Array(encoded.buffer, encoded.byteOffset, encoded.byteLength)
      : new Uint8Array(encoded);
  if (bytes.byteLength < KPS1_HEADER_LENGTH) {
    throw new Kps1ProtocolError("length-mismatch", "truncated KPS1 header");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = new TextDecoder().decode(bytes.subarray(0, 4));
  if (magic !== KPS1_MAGIC) {
    throw new Kps1ProtocolError("bad-magic", `expected ${KPS1_MAGIC}, received ${magic}`);
  }
  if (view.getUint16(4, true) !== KPS1_MAJOR || view.getUint16(6, true) !== KPS1_MINOR) {
    throw new Kps1ProtocolError("bad-version", "unsupported KPS1 protocol version");
  }
  if (view.getUint16(8, true) !== KPS1_HEADER_LENGTH) {
    throw new Kps1ProtocolError("bad-header-length", "invalid KPS1 header length");
  }

  const kind = view.getUint16(10, true) as Kps1MessageKind;
  const payloadLength = view.getUint32(40, true);
  const expectedLength = KPS1_HEADER_LENGTH + payloadLength;
  if (bytes.byteLength !== expectedLength) {
    throw new Kps1ProtocolError(
      "length-mismatch",
      `KPS1 frame length ${bytes.byteLength} does not match ${expectedLength}`,
    );
  }

  const frame: Kps1Frame = {
    kind,
    flags: view.getUint32(12, true),
    sessionNonce: view.getBigUint64(16, true),
    sequence: view.getBigUint64(24, true),
    correlationId: view.getBigUint64(32, true),
    payload: bytes.slice(KPS1_HEADER_LENGTH),
  };
  validateFrame(frame, options.maximumPayload ?? KPS1_MAX_PAYLOAD);
  if (
    options.expectedSessionNonce !== undefined &&
    frame.sessionNonce !== options.expectedSessionNonce
  ) {
    throw new Kps1ProtocolError("invalid-session", "stale or mismatched KPS1 session nonce");
  }

  const expectedCrc = view.getUint32(44, true);
  const actualCrc = crc32([bytes.subarray(0, 44), bytes.subarray(KPS1_HEADER_LENGTH)]);
  if (actualCrc !== expectedCrc) {
    throw new Kps1ProtocolError(
      "crc-mismatch",
      `KPS1 CRC mismatch: expected 0x${expectedCrc.toString(16)}, got 0x${actualCrc.toString(16)}`,
    );
  }
  return frame;
}

export class Kps1SequenceValidator {
  private nextSequence: bigint;

  constructor(
    private readonly sessionNonce: bigint,
    firstSequence = 1n,
  ) {
    if (sessionNonce === 0n || firstSequence === 0n) {
      throw new RangeError("session nonce and first sequence must be nonzero");
    }
    this.nextSequence = firstSequence;
  }

  accept(frame: Kps1Frame): void {
    if (frame.sessionNonce !== this.sessionNonce) {
      throw new Kps1ProtocolError("invalid-session", "stale or mismatched KPS1 session nonce");
    }
    if (frame.sequence !== this.nextSequence) {
      throw new Kps1ProtocolError(
        "invalid-sequence",
        `expected KPS1 sequence ${this.nextSequence}, received ${frame.sequence}`,
      );
    }
    this.nextSequence += 1n;
  }

  cursor(): bigint {
    return this.nextSequence - 1n;
  }
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0 || !/^[0-9a-f]*$/iu.test(hex)) {
    throw new TypeError("hex input must contain complete hexadecimal bytes");
  }
  const bytes = new Uint8Array(hex.length / 2);
  for (let offset = 0; offset < bytes.length; offset += 1) {
    bytes[offset] = Number.parseInt(hex.slice(offset * 2, offset * 2 + 2), 16);
  }
  return bytes;
}
