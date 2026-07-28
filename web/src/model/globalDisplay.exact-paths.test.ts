import { describe, expect, it } from "vitest";
import { buildExactGlobalDisplay, type ExactGlobalDisplayInput } from "./globalDisplay";
import {
  type GlobalDisplayDefinitionV1,
  type GlobalDisplayPathChunkV1,
} from "../protocol/globalDisplay";

const definition: GlobalDisplayDefinitionV1 = {
  displayIdentity: 1,
  earthIdentity: 2,
  transformIdentity: 3,
  missionIdentity: 4,
  epochUnixDay: 19_723,
  epochTaiMinusUtc: 37,
  semiMajorQ12Km: 26_124_849,
  semiMinorQ12Km: 26_036_734,
  inverseFlatteningQ20: 313_883_719,
  launchAnchor: { identity: 5, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [1, 2, 3] },
  recoveryAnchor: { identity: 6, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [4, 5, 6] },
  availableSourceMask: 2,
  availableFrameMask: 7,
  cameraDomainMask: 0xff,
};

function chunk(chunkIndex: number, overrides: Partial<GlobalDisplayPathChunkV1> = {}): GlobalDisplayPathChunkV1 {
  return {
    pathIdentity: 10,
    source: 2,
    displayFrame: 2,
    lod: 2,
    flags: 4,
    modelIdentity: 11,
    estimateIdentity: 12,
    sourceChecksum: 13,
    continuityIdentity: 14,
    chunkIndex,
    chunkCount: 2,
    points: [{ releaseEpoch: chunkIndex, missionTimeQ16: chunkIndex * 2_048, segment: 2,
      eventMask: chunkIndex, anchorIdentity: 0, positionQ12Km: [chunkIndex, 0, 0] }],
    ...overrides,
  };
}

function build(chunks: readonly GlobalDisplayPathChunkV1[]) {
  const input: ExactGlobalDisplayInput = {
    definition,
    samples: [],
    paths: new Map(chunks.map((value, index) => [String(index), value])),
    transitions: [],
  };
  return buildExactGlobalDisplay(input);
}

describe("exact GlobalDisplayV1 path reconstruction", () => {
  it("assembles exactly one identical-metadata chunk at every index", () => {
    const path = build([chunk(1), chunk(0)]).paths[0];
    expect(path).toMatchObject({ flags: 4, stale: false, incomplete: false, terminal: true });
    expect(path?.points.map((point) => point.releaseEpoch)).toEqual([0, 1]);
  });

  it("rejects flags or other path metadata changing between chunks", () => {
    expect(() => build([chunk(0), chunk(1, { flags: 1 })])).toThrow(/metadata changed/);
    expect(() => build([chunk(0), chunk(1, { sourceChecksum: 99 })])).toThrow(/metadata changed/);
    expect(() => build([chunk(0), chunk(1, { chunkCount: 3 })])).toThrow(/metadata changed/);
  });

  it("rejects release or mission-time rewind across chunk boundaries", () => {
    expect(() => build([chunk(0), chunk(1, { points: [{ releaseEpoch: 0, missionTimeQ16: 2_048,
      segment: 2, eventMask: 0, anchorIdentity: 0, positionQ12Km: [1, 0, 0] }] })])).toThrow(/point order/);
    expect(() => build([chunk(0), chunk(1, { points: [{ releaseEpoch: 1, missionTimeQ16: 0,
      segment: 2, eventMask: 0, anchorIdentity: 0, positionQ12Km: [1, 0, 0] }] })])).toThrow(/point order/);
  });

  it("rejects duplicate, missing, and extra multi-chunk records", () => {
    expect(() => build([chunk(0), chunk(0)])).toThrow(/duplicate or invalid chunk index/);
    expect(() => build([chunk(0)])).toThrow(/missing or extra chunk/);
    expect(() => build([chunk(0), chunk(1), chunk(2)])).toThrow(/missing or extra chunk/);
  });
});
