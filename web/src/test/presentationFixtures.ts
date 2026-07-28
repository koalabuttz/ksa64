import { crc32, Kps1MessageKind, type Kps1Frame } from "../protocol/kps1";
import type { ActionOperation, NumericRole } from "../protocol/presentation";
import type { GlobalDisplayPathChunkV1 } from "../protocol/globalDisplay";

class Writer {
  readonly bytes: number[] = new Array(12).fill(0);
  constructor(magic: string) { this.bytes.splice(0, 4, ...new TextEncoder().encode(magic)); this.set16(4, 1); this.set16(6, 12); }
  u8(value: number) { this.bytes.push(value & 0xff); }
  bool(value: boolean) { this.u8(value ? 1 : 0); }
  u16(value: number) { this.bytes.push(value & 0xff, (value >>> 8) & 0xff); }
  u32(value: number) { this.bytes.push(value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff); }
  i32(value: number) { this.u32(value >>> 0); }
  u64(value: bigint) { for (let shift = 0n; shift < 64n; shift += 8n) this.u8(Number((value >> shift) & 0xffn)); }
  reserved(count: number) { for (let index = 0; index < count; index += 1) this.u8(0); }
  text(value: string) { const encoded = new TextEncoder().encode(value); this.u16(encoded.length); this.bytes.push(...encoded); }
  finish(): Uint8Array { this.set32(8, this.bytes.length); return Uint8Array.from(this.bytes); }
  private set16(offset: number, value: number) { this.bytes[offset] = value & 0xff; this.bytes[offset + 1] = (value >>> 8) & 0xff; }
  private set32(offset: number, value: number) { for (let byte = 0; byte < 4; byte += 1) this.bytes[offset + byte] = (value >>> (byte * 8)) & 0xff; }
}

export function testFrame(kind: Kps1MessageKind, payload: Uint8Array, sequence = 1n): Kps1Frame {
  return { kind, flags: 0, sessionNonce: 99n, sequence, correlationId: kind === Kps1MessageKind.ActionReceipt ? sequence : 0n, payload };
}

export function snapshotPayload(
  role: NumericRole,
  truth = false,
  publicationSequence = 1n,
  validityMask = 0x1ffn,
): Uint8Array {
  const w = new Writer("POS1"); w.u32(0x12b50001); w.u32(0x12b01001); w.u64(publicationSequence);
  w.u64(truth ? (1n << 63n) | validityMask : validityMask);
  w.u8(role); w.u8(3); w.u8(2); w.u8(2); w.bool(false); w.bool(truth); w.reserved(2);
  w.u32(6000); w.u32(31250); w.u32(3); w.u32(200 * 65536);
  for (const offset of [0, 64]) { w.i32(1000 + offset); w.i32(2000); w.i32(100 * 4096); w.i32(100); w.i32(200); w.i32(300); w.u32(0x1000 + offset); }
  w.u32(7); w.u32(8); w.u32(9); w.u32(3); w.i32(250 * 4096); w.i32(-10); w.u32(12 * 65536); w.u32(60 * 65536);
  w.i32(1); w.i32(2); w.i32(3); w.u8(1); w.reserved(3);
  for (let index = 0; index < 7; index += 1) w.u32(0x2000 + index); w.u16(0); w.reserved(2);
  if (truth) { for (let index = 0; index < 6; index += 1) w.i32(index); for (let index = 0; index < 4; index += 1) w.i32(index); w.u32(1); w.u32(2); }
  return w.finish();
}

export function procedurePayload(activeStep = 7, stepCount = 7): Uint8Array {
  const w = new Writer("PPR1"); w.u32(0x12b54001); w.u16(activeStep); w.u16(stepCount); w.u8(4); w.bool(true); w.reserved(2);
  w.u32(7872); w.u32(7938); w.u16(1); w.reserved(2); w.text("GNSS-loss response"); w.text("Terminal procedure step");
  w.u32(0x4401); w.bool(true); w.reserved(3); return w.finish();
}

export function dispositionPayload(overall = 3): Uint8Array {
  const w = new Writer("PDS1"); w.u8(overall); w.u8(3); w.u8(3); w.u8(2); w.u8(2); w.u8(2); w.u8(1); w.reserved(1); w.u32(0x12b01001); return w.finish();
}

export function globalReplayIndexPayload(): Uint8Array {
  const w = new Writer("PGI1");
  w.u32(0x12c10001); w.u32(0x12c10002); w.u32(0); w.u32(22_014);
  w.u8(1);
  for (let index = 0; index < 6; index += 1) w.u8(1);
  w.reserved(1); w.u16(0); w.reserved(2);
  return w.finish();
}

export function proposalPayload(permissions = 3): Uint8Array {
  const w = new Writer("PAP1"); w.u32(0x47535504); w.u32(0x1010); w.u8(1); w.u8(permissions); w.reserved(2);
  w.u32(6001); w.u32(6002); w.u32(6010); w.u32(6200); w.u32(0x3344); w.u32(0); w.text("Proposal GSU-04"); return w.finish();
}

export function receiptPayload(operation: ActionOperation, sequence: bigint): Uint8Array {
  const w = new Writer("PAR1"); w.u64(sequence); w.u32(0x47535504); w.u32(0x1010); w.u32(0x2000 + operation);
  w.u32(6000 + operation); w.u32(6010); w.u8(2); w.u8(0); w.bool(true); w.u8(operation); w.u32(0x5000 + operation); return w.finish();
}

export function evidenceMetadataPayload(bytes: Uint8Array, chunkLength: number, identity = 0x12b5e001): Uint8Array {
  const output = new Uint8Array(48);
  const view = new DataView(output.buffer);
  output.set(new TextEncoder().encode("PEM1"));
  view.setUint16(4, 1, true); view.setUint16(6, 48, true);
  view.setUint32(8, identity, true); view.setUint32(12, crc32([bytes]), true);
  view.setBigUint64(16, BigInt(bytes.byteLength), true); view.setUint32(24, chunkLength, true);
  view.setUint32(28, Math.ceil(bytes.byteLength / chunkLength), true); output[32] = 1; output[33] = 1;
  return output;
}

export function evidenceChunkPayload(bytes: Uint8Array, chunkLength: number, index: number, identity = 0x12b5e001): Uint8Array {
  const start = index * chunkLength;
  const data = bytes.slice(start, Math.min(bytes.byteLength, start + chunkLength));
  const w = new Writer("PEC1"); w.u32(identity); w.u32(index); w.u32(Math.ceil(bytes.byteLength / chunkLength));
  w.u64(BigInt(start)); w.u32(data.byteLength); w.bytes.push(...data); return w.finish();
}

export function globalPathPayload(
  chunkIndex: number,
  chunkCount: number,
  overrides: Partial<GlobalDisplayPathChunkV1> = {},
): Uint8Array {
  const value: GlobalDisplayPathChunkV1 = {
    pathIdentity: 0x12c2_0001,
    source: 2,
    displayFrame: 2,
    lod: 2,
    flags: 4,
    modelIdentity: 0x12c2_0002,
    estimateIdentity: 0x12c2_0003,
    sourceChecksum: 0x12c2_0004,
    continuityIdentity: 0x12c2_0005,
    chunkIndex,
    chunkCount,
    points: [{
      releaseEpoch: 100 + chunkIndex,
      missionTimeQ16: (100 + chunkIndex) * 2_048,
      segment: 2,
      eventMask: 0,
      anchorIdentity: 0,
      positionQ12Km: [chunkIndex, 0, 0],
    }],
    ...overrides,
  };
  const w = new Writer("PGP1");
  w.u32(value.pathIdentity); w.u8(value.source); w.u8(value.displayFrame); w.u8(value.lod); w.reserved(1);
  w.u16(value.flags); w.u16(value.chunkIndex); w.u16(value.chunkCount); w.u16(value.points.length);
  w.u32(value.modelIdentity); w.u32(value.estimateIdentity); w.u32(value.sourceChecksum); w.u32(value.continuityIdentity);
  for (const point of value.points) {
    w.u32(point.releaseEpoch); w.u32(point.missionTimeQ16); w.u8(point.segment); w.reserved(1);
    w.u16(point.eventMask); w.u32(point.anchorIdentity);
    w.i32(point.positionQ12Km[0]); w.i32(point.positionQ12Km[1]); w.i32(point.positionQ12Km[2]);
  }
  return w.finish();
}
