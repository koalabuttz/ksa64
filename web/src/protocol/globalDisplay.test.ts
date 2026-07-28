import { describe, expect, it } from "vitest";
import {
  decodeGlobalDisplayDefinitionPayload,
  decodeGlobalDisplayPathPayload,
  decodeGlobalDisplaySamplesPayload,
  decodeGlobalDisplayTransitionPayload,
  decodeGlobalReplayIndexPayload,
  GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK,
  GLOBAL_DISPLAY_SOURCE_SIM_TRUTH,
} from "./globalDisplay";

class Writer {
  readonly bytes: number[] = new Array(12).fill(0);
  constructor(magic: string) {
    this.bytes.splice(0, 4, ...new TextEncoder().encode(magic));
    this.set16(4, 1);
    this.set16(6, 12);
  }
  u8(value: number): void { this.bytes.push(value & 0xff); }
  u16(value: number): void { this.bytes.push(value & 0xff, (value >>> 8) & 0xff); }
  i16(value: number): void { this.u16(value & 0xffff); }
  u32(value: number): void {
    this.bytes.push(value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff);
  }
  i32(value: number): void { this.u32(value >>> 0); }
  u64(value: bigint): void {
    for (let shift = 0n; shift < 64n; shift += 8n) this.u8(Number((value >> shift) & 0xffn));
  }
  zero(count: number): void { for (let index = 0; index < count; index += 1) this.u8(0); }
  i32s(values: readonly number[]): void { for (const value of values) this.i32(value); }
  finish(): Uint8Array { this.set32(8, this.bytes.length); return Uint8Array.from(this.bytes); }
  private set16(offset: number, value: number): void {
    this.bytes[offset] = value & 0xff;
    this.bytes[offset + 1] = (value >>> 8) & 0xff;
  }
  private set32(offset: number, value: number): void {
    for (let index = 0; index < 4; index += 1) this.bytes[offset + index] = (value >>> (index * 8)) & 0xff;
  }
}

function definitionPayload(sourceMask = GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK): Uint8Array {
  const writer = new Writer("PGD1");
  writer.u32(0x12c00001); writer.u32(1); writer.u32(2); writer.u32(3);
  writer.i32(19_723); writer.i16(37); writer.zero(2);
  writer.i32(26_124_165); writer.i32(26_036_734); writer.i32(313_883_719);
  writer.u32(4); writer.i32s([5, 6, 7]);
  writer.u32(8); writer.i32s([9, 10, 11]);
  writer.u32(sourceMask); writer.u8(7); writer.zero(1); writer.u16(0xff);
  return writer.finish();
}

function resolvedPose(writer: Writer, position: readonly number[]): void {
  writer.i32s(position); writer.i32s([4, 5, 6]); writer.i32s([1 << 30, 0, 0, 0]);
}

function sourcePose(writer: Writer, source: number): void {
  writer.u8(source); writer.u8(2); writer.zero(2);
  writer.u32((1 << 10) - 1); writer.u32(0x100 + source); writer.u32(0x200 + source);
  writer.u32(0x300 + source); writer.u32(0);
  for (let index = 0; index < 5; index += 1) resolvedPose(writer, [100 + index, 200, 300]);
  writer.i32s([7, 8, 9]);
}

function samplesPayload(sources: readonly number[] = [2, 3]): Uint8Array {
  const writer = new Writer("PGS1");
  writer.u16(1); writer.zero(2); writer.u64(5n); writer.u32(32); writer.u32(65_536);
  writer.u8(2); writer.u8(2); writer.u8(1); writer.u8(1); writer.u16(4); writer.u16(sources.length);
  writer.u32(0); writer.u32(0x5555); writer.i32s([1, 2, 3]); writer.i32(4);
  writer.i32(5); writer.i32(6); writer.i32(7); writer.i32(8); writer.i32(9);
  writer.i16(10); writer.i16(-11); for (let index = 0; index < 12; index += 1) writer.u8(1);
  writer.u8(12); writer.u8(13); writer.u16(14);
  for (const source of sources) sourcePose(writer, source);
  return writer.finish();
}

function pathPayload(): Uint8Array {
  const writer = new Writer("PGP1");
  writer.u32(1); writer.u8(3); writer.u8(2); writer.u8(2); writer.zero(1);
  writer.u16(4); writer.u16(0); writer.u16(1); writer.u16(2);
  writer.u32(2); writer.u32(3); writer.u32(4); writer.u32(5);
  writer.u32(32); writer.u32(65_536); writer.u8(2); writer.zero(1); writer.u16(0); writer.i32s([1, 2, 3]);
  writer.u32(64); writer.u32(131_072); writer.u8(2); writer.zero(1); writer.u16(1); writer.i32s([4, 5, 6]);
  return writer.finish();
}

describe("Rust-compatible GlobalDisplayV1 payloads", () => {
  it("decodes the frozen definition and rejects nonzero reserved bytes", () => {
    expect(decodeGlobalDisplayDefinitionPayload(definitionPayload(), 2)).toMatchObject({
      displayIdentity: 0x12c00001,
      epochTaiMinusUtc: 37,
      availableSourceMask: GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK,
      recoveryAnchor: { identity: 8, geodeticQ28Q12: [9, 10, 11] },
    });
    const corrupt = definitionPayload();
    corrupt[34] = 1;
    expect(() => decodeGlobalDisplayDefinitionPayload(corrupt, 2)).toThrow(/reserved/u);
  });

  it("decodes resolved source poses and rejects SIM truth for operational roles", () => {
    const values = decodeGlobalDisplaySamplesPayload(samplesPayload(), 2);
    expect(values[0]).toMatchObject({
      sequence: 5n,
      releaseEpoch: 32,
      activeFrame: 2,
      segment: 2,
      sources: [{ source: 2, ecef: { positionQ12Km: [101, 200, 300] } }, { source: 3 }],
    });
    const truth = samplesPayload([2, 4]);
    expect(decodeGlobalDisplaySamplesPayload(truth, 5)[0]?.sources.at(-1)?.source).toBe(4);
    expect(() => decodeGlobalDisplaySamplesPayload(truth, 2)).toThrow(/private truth/u);
    expect(() => decodeGlobalDisplayDefinitionPayload(
      definitionPayload(GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK | GLOBAL_DISPLAY_SOURCE_SIM_TRUTH), 2,
    )).toThrow(/definition/u);
  });

  it("decodes bounded path chunks and rejects non-monotonic points", () => {
    const value = decodeGlobalDisplayPathPayload(pathPayload(), 2);
    expect(value).toMatchObject({ pathIdentity: 1, source: 3, lod: 2, flags: 4, chunkCount: 1 });
    expect(value.points).toHaveLength(2);
    const corrupt = pathPayload();
    // Second release begins at byte 68 in this two-point fixture.
    new DataView(corrupt.buffer).setUint32(68, 31, true);
    expect(() => decodeGlobalDisplayPathPayload(corrupt, 2)).toThrow();
  });

  it("decodes exact transition continuity and replay index records", () => {
    const transition = new Writer("PGT1");
    transition.u32(29); transition.u32(59_392); transition.u8(1); transition.u8(2);
    transition.u8(1); transition.u8(2); transition.u8(1); transition.zero(3);
    transition.u32(1); transition.u32(2); transition.u32(3);
    transition.i32s([0, 0, 0]); transition.i32s([0, 0, 0]); transition.i32(1);
    transition.i32s([0, 0, 0]); transition.u32(4);
    expect(decodeGlobalDisplayTransitionPayload(transition.finish())).toMatchObject({
      releaseEpoch: 29, fromFrame: 1, toFrame: 2, checksum: 4,
    });

    const index = new Writer("PGI1");
    index.u32(1); index.u32(2); index.u32(0); index.u32(100); index.u8(3);
    for (const value of [1, 2, 3, 4, 1, 1]) index.u8(value);
    index.zero(1); index.u16(1); index.zero(2);
    index.u32(29); index.u32(59_392); index.u8(2); index.zero(3);
    index.u32(3); index.u32(4); index.u32(5);
    expect(decodeGlobalReplayIndexPayload(index.finish())).toMatchObject({
      firstRelease: 0, lastRelease: 100, terminalDisposition: 3,
      entries: [{ releaseEpoch: 29, kind: 2, eventIdentity: 4 }],
    });
  });
});
