import { describe, expect, it } from "vitest";
import { Kps1MessageKind } from "../protocol/kps1";
import { initialLivePresentationState, reduceTransportEvent } from "./liveState";
import {
  evidenceChunkPayload,
  evidenceMetadataPayload,
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
});
