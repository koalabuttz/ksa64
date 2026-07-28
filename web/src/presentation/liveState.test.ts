import { describe, expect, it } from "vitest";
import { Kps1MessageKind } from "../protocol/kps1";
import { initialLivePresentationState, reduceTransportEvent } from "./liveState";
import {
  evidenceChunkPayload,
  evidenceMetadataPayload,
  globalPathPayload,
  proposalPayload,
  snapshotPayload,
  testFrame,
} from "../test/presentationFixtures";

describe("live presentation state", () => {
  it("keeps accepted data through disconnect and reconnect while authority continues", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, { type: "state", state: "connected" }, 2);
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(Kps1MessageKind.Snapshot, snapshotPayload(2, false, 7n), 7n) }, 2);
    state = reduceTransportEvent(state, { type: "state", state: "stale", detail: "presentation disconnected; authority continues" }, 2);
    expect(state.snapshot?.publicationSequence).toBe(7n);
    expect(state.connection).toBe("stale");
    state = reduceTransportEvent(state, { type: "state", state: "connected" }, 2);
    expect(state.snapshot?.releaseEpoch).toBe(6000);
    expect(state.connection).toBe("connected");
  });

  it("marks local worker termination incomplete and never implies sealed evidence", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, { type: "incomplete", reason: "worker terminated" }, 2);
    expect(state.connection).toBe("failed");
    expect(state.incompleteReason).toBe("worker terminated");
    expect(state.evidence).toBeUndefined();
  });

  it("fails closed when a frame crosses the immutable role boundary", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(Kps1MessageKind.Snapshot, snapshotPayload(5, true)) }, 2);
    expect(state.connection).toBe("failed");
    expect(state.decodeError).toMatch(/private truth|role/iu);
    expect(state.snapshot).toBeUndefined();
  });

  it("clears an action proposal when the authoritative snapshot closes its gate", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(
      state,
      { type: "frame", frame: testFrame(Kps1MessageKind.ActionProposal, proposalPayload()) },
      2,
    );
    expect(state.proposal).toBeDefined();

    const closedGate = snapshotPayload(2, false, 2n);
    closedGate[28] = (closedGate[28] ?? 0) & ~0x20;
    state = reduceTransportEvent(
      state,
      { type: "frame", frame: testFrame(Kps1MessageKind.Snapshot, closedGate, 2n) },
      2,
    );
    expect(state.proposal).toBeUndefined();
  });

  it("assembles every sealed evidence chunk and verifies the advertised CRC", () => {
    const evidence = Uint8Array.from([1, 2, 3, 4, 5, 6]);
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(
      Kps1MessageKind.EvidenceMetadata, evidenceMetadataPayload(evidence, 4), 1n) }, 2);
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(
      Kps1MessageKind.EvidenceChunk, evidenceChunkPayload(evidence, 4, 0), 2n) }, 2);
    expect(state.sealedEvidence).toBeUndefined();
    expect(state.evidenceAssembly?.receivedLength).toBe(4);
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(
      Kps1MessageKind.EvidenceChunk, evidenceChunkPayload(evidence, 4, 1), 3n) }, 2);
    expect(Array.from(state.sealedEvidence ?? [])).toEqual(Array.from(evidence));
    expect(state.connection).not.toBe("failed");
  });

  it("fails closed when sealed evidence chunks conflict with metadata", () => {
    const evidence = Uint8Array.from([1, 2, 3, 4, 5, 6]);
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(
      Kps1MessageKind.EvidenceMetadata, evidenceMetadataPayload(evidence, 4), 1n) }, 2);
    const corrupt = evidenceChunkPayload(evidence, 4, 0); corrupt[20] = 9;
    state = reduceTransportEvent(state, { type: "frame", frame: testFrame(
      Kps1MessageKind.EvidenceChunk, corrupt, 2n) }, 2);
    expect(state.connection).toBe("failed");
    expect(state.sealedEvidence).toBeUndefined();
  });

  function globalPathFrame(
    chunkIndex: number,
    chunkCount: number,
    sequence: bigint,
    overrides: Parameters<typeof globalPathPayload>[2] = {},
  ) {
    return {
      type: "frame" as const,
      frame: testFrame(
        Kps1MessageKind.GlobalDisplayPathChunk,
        globalPathPayload(chunkIndex, chunkCount, overrides),
        sequence,
      ),
    };
  }

  function globalPathPoints(startRelease: number, count: number) {
    return Array.from({ length: count }, (_, index) => {
      const releaseEpoch = startRelease + index;
      return {
        releaseEpoch,
        missionTimeQ16: releaseEpoch * 2_048,
        segment: 2 as const,
        eventMask: 0,
        anchorIdentity: 0,
        positionQ12Km: [releaseEpoch, 0, 0] as const,
      };
    });
  }

  it("fails closed when a path declares more chunks than the bounded assembler accepts", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(0, 65, 1n), 2);

    expect(state.connection).toBe("failed");
    expect(state.decodeError).toMatch(/bounded pending chunk limit/iu);
    expect(state.globalPaths.size).toBe(0);
  });

  it("fails before retaining global path points beyond the aggregate pending budget", () => {
    let state = initialLivePresentationState();
    for (let index = 0; index < 16; index += 1) {
      state = reduceTransportEvent(state, globalPathFrame(index, 17, BigInt(index + 1), {
        points: globalPathPoints(index * 4_096 + 1, 4_096),
      }), 2);
    }
    expect(state.connection).not.toBe("failed");
    expect(state.globalPaths.size).toBe(0);

    state = reduceTransportEvent(state, globalPathFrame(16, 17, 17n, {
      points: globalPathPoints(16 * 4_096 + 1, 4_096),
    }), 2);

    expect(state.connection).toBe("failed");
    expect(state.decodeError).toMatch(/pending global display paths exceed the retained point limit/iu);
    expect(state.globalPaths.size).toBe(0);
  });

  it("does not expose the first chunk of an incomplete global path generation", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(0, 2, 1n), 2);

    expect(state.globalPaths.size).toBe(0);
    expect(state.pendingGlobalPaths.size).toBe(1);
    expect(state.connection).not.toBe("failed");
  });

  it("publishes a multi-chunk global path atomically when its final chunk arrives", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(1, 2, 1n), 2);
    expect(state.globalPaths.size).toBe(0);

    state = reduceTransportEvent(state, globalPathFrame(0, 2, 2n), 2);
    expect([...state.globalPaths.values()].map((value) => value.chunkIndex)).toEqual([0, 1]);
    expect(state.pendingGlobalPaths.size).toBe(0);
    expect(state.connection).not.toBe("failed");
  });

  it("fails closed instead of coalescing duplicate path chunks within one generation", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(0, 2, 1n), 2);
    state = reduceTransportEvent(state, globalPathFrame(0, 2, 2n), 2);

    expect(state.connection).toBe("failed");
    expect(state.decodeError).toMatch(/duplicate global display path chunk/iu);
    expect(state.globalPaths.size).toBe(0);
  });

  it("rejects cross-chunk point rewinds without replacing a complete path", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(0, 1, 1n, { sourceChecksum: 11 }), 2);

    state = reduceTransportEvent(state, globalPathFrame(0, 2, 2n, { sourceChecksum: 99 }), 2);
    state = reduceTransportEvent(state, globalPathFrame(1, 2, 3n, {
      sourceChecksum: 99,
      points: [{
        releaseEpoch: 100,
        missionTimeQ16: 100 * 2_048,
        segment: 2,
        eventMask: 0,
        anchorIdentity: 0,
        positionQ12Km: [1, 0, 0],
      }],
    }), 2);

    expect(state.connection).toBe("failed");
    expect(state.decodeError).toMatch(/point order changed between chunks/iu);
    expect([...state.globalPaths.values()].map((value) => value.sourceChecksum)).toEqual([11]);
  });

  it("retains a complete path while an incompatible replacement generation is incomplete", () => {
    let state = initialLivePresentationState();
    state = reduceTransportEvent(state, globalPathFrame(0, 1, 1n, { sourceChecksum: 11 }), 2);
    expect([...state.globalPaths.values()].map((value) => value.sourceChecksum)).toEqual([11]);

    state = reduceTransportEvent(state, globalPathFrame(0, 2, 2n, { sourceChecksum: 99 }), 2);
    expect([...state.globalPaths.values()].map((value) => value.sourceChecksum)).toEqual([11]);
    expect(state.pendingGlobalPaths.size).toBe(1);

    state = reduceTransportEvent(state, globalPathFrame(1, 2, 3n, { sourceChecksum: 99 }), 2);
    expect([...state.globalPaths.values()].map((value) => [value.chunkIndex, value.sourceChecksum]))
      .toEqual([[0, 99], [1, 99]]);
    expect(state.pendingGlobalPaths.size).toBe(0);
  });
});
