import { describe, expect, it } from "vitest";
import { Kps1MessageKind } from "./kps1";
import { decodePresentationPayload, decodeHandshakePayload, encodeCursorsPayload,
  encodeHandshakePayload, initialPresentationCursors } from "./presentation";
import { dispositionPayload, procedurePayload, snapshotPayload, testFrame } from "../test/presentationFixtures";

describe("typed KPS1 presentation payloads", () => {
  it("decodes the multidimensional disposition instead of collapsing it to pass/fail", () => {
    const decoded = decodePresentationPayload(testFrame(Kps1MessageKind.Disposition, dispositionPayload()), 2);
    expect(decoded.kind).toBe("disposition");
    if (decoded.kind === "disposition") expect(decoded.value).toMatchObject({ overall: 3, objective: 3, vehicle: 3, procedure: 2, avionics: 2, evidence: 1 });
  });

  it("enforces the immutable role and blocks truth for a guided operator", () => {
    const guided = testFrame(Kps1MessageKind.Snapshot, snapshotPayload(2, false));
    expect(decodePresentationPayload(guided, 2).kind).toBe("snapshot");
    expect(() => decodePresentationPayload(guided, 3)).toThrow("immutable role mismatch");
    const leaked = testFrame(Kps1MessageKind.Snapshot, snapshotPayload(2, true));
    expect(() => decodePresentationPayload(leaked, 2)).toThrow("private truth crossed");
    const simDirector = testFrame(Kps1MessageKind.Snapshot, snapshotPayload(5, true));
    expect(decodePresentationPayload(simDirector, 5).kind).toBe("snapshot");
  });

  it("rejects corruption and reserved bytes before publishing values", () => {
    const payload = dispositionPayload().slice(); payload[19] = 1;
    expect(() => decodePresentationPayload(testFrame(Kps1MessageKind.Disposition, payload), 2)).toThrow("reserved");
    const truncated = snapshotPayload(2).slice(0, -1);
    expect(() => decodePresentationPayload(testFrame(Kps1MessageKind.Snapshot, truncated), 2)).toThrow("length mismatch");
  });

  it("accepts the one-based terminal procedure step", () => {
    const decoded = decodePresentationPayload(testFrame(Kps1MessageKind.Procedure, procedurePayload(7, 7)), 2);
    expect(decoded.kind).toBe("procedure");
    if (decoded.kind === "procedure") expect(decoded.value).toMatchObject({ activeStep: 7, stepCount: 7 });
    expect(() => decodePresentationPayload(testFrame(Kps1MessageKind.Procedure, procedurePayload(0, 7)), 2)).toThrow("invalid procedure");
    expect(() => decodePresentationPayload(testFrame(Kps1MessageKind.Procedure, procedurePayload(8, 7)), 2)).toThrow("invalid procedure");
  });

  it("round-trips handshake cursors without putting credentials in payloads", () => {
    const cursors = { ...initialPresentationCursors(), events: 9n, releaseSamples: 41n };
    const payload = encodeHandshakePayload({ role: 2, clientInstance: 77n, capabilityMask: 0n, cursors });
    expect(decodeHandshakePayload(payload)).toEqual({ role: 2, clientInstance: 77n, capabilityMask: 0n, cursors });
    const replay = encodeCursorsPayload(cursors);
    expect(new TextDecoder().decode(replay.subarray(0, 4))).toBe("KCR1");
    expect(new DataView(replay.buffer).getBigUint64(40, true)).toBe(41n);
  });
});
