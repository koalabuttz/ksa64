import { describe, expect, it } from "vitest";
import { encodeKps1, Kps1Flag, Kps1MessageKind } from "../protocol/kps1";
import { evidenceMetadataPayload, snapshotPayload } from "../test/presentationFixtures";
import { VerifiedWasmReplayTransport } from "./verifiedWasmReplay";

class FakeReplayWorker {
  onmessage: ((event: MessageEvent<ArrayBuffer | Record<string, unknown>>) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  readonly posted: { readonly value: any; readonly transfer?: Transferable[] }[] = [];
  terminated = false;

  postMessage(value: any, transfer?: Transferable[]): void {
    this.posted.push({ value, transfer });
  }
  terminate(): void { this.terminated = true; }
  emit(value: ArrayBuffer | Record<string, unknown>): void {
    this.onmessage?.({ data: value } as MessageEvent<ArrayBuffer | Record<string, unknown>>);
  }
  crash(): void { this.onerror?.(new Event("error")); }
}

function archive(): Uint8Array {
  return new Uint8Array([0x4b, 0x53, 0x42, 0x31, 1, 2, 3, 4]);
}

function frame(kind: Kps1MessageKind, payload: Uint8Array, sequence: bigint, flags = 0): ArrayBuffer {
  return encodeKps1({ kind, flags, sessionNonce: 99n, sequence, correlationId: 0n, payload }).buffer as ArrayBuffer;
}

function open(role: "observer" | "guided-operator" = "guided-operator", frameCount = "2") {
  const worker = new FakeReplayWorker();
  const transport = new VerifiedWasmReplayTransport(archive(), () => worker as unknown as Worker);
  const events: any[] = [];
  transport.subscribe((event) => events.push(event));
  const connecting = transport.connect({ role });
  worker.emit({ type: "ready", sessionNonce: "99", frameCount });
  return { worker, transport, events, connecting };
}

describe("verified Rust/WASM KSB11 replay transport", () => {
  it("transfers opaque evidence, streams role-filtered KPS1, and requires final metadata", async () => {
    const { worker, transport, events, connecting } = open();
    await connecting;
    expect(transport.state).toBe("connected");
    expect(worker.posted[0]?.value).toMatchObject({ type: "initialize-replay", role: "guided-operator" });
    expect(worker.posted[0]?.value.archive).toBeInstanceOf(ArrayBuffer);
    expect(worker.posted[0]?.transfer).toHaveLength(1);

    const advancing = transport.advance(2);
    expect(worker.posted[1]?.value).toMatchObject({ type: "read-replay", cursor: "0", count: 2 });
    worker.emit(frame(Kps1MessageKind.Snapshot, snapshotPayload(2, false), 1n));
    worker.emit(frame(
      Kps1MessageKind.EvidenceMetadata,
      evidenceMetadataPayload(new Uint8Array([1]), 1),
      2n,
      Kps1Flag.Final,
    ));
    worker.emit({ type: "advanced", requestId: 1, cursor: "2", frameCount: "2", complete: true });
    await expect(advancing).resolves.toBe(2n);
    expect(transport.complete).toBe(true);
    expect(events.filter((event) => event.type === "frame")).toHaveLength(2);
    await expect(transport.submit({ kind: "review", proposalId: "x" })).rejects.toThrow("read-only");
  });

  it("reports corrupt or unsupported evidence as rejected before publishing", async () => {
    const worker = new FakeReplayWorker();
    const transport = new VerifiedWasmReplayTransport(archive(), () => worker as unknown as Worker);
    const events: any[] = [];
    transport.subscribe((event) => events.push(event));
    const connecting = transport.connect({ role: "observer" });
    worker.emit({ type: "rejected", detail: "Rust rejected corrupt KSB11" });
    await expect(connecting).rejects.toThrow("corrupt KSB11");
    expect(transport.state).toBe("failed");
    expect(events.some((event) => event.type === "frame")).toBe(false);
    expect(events.some((event) => event.type === "incomplete")).toBe(false);
  });

  it("fails closed if a replay frame violates immutable role isolation", async () => {
    const { worker, transport, events, connecting } = open("observer");
    await connecting;
    worker.emit(frame(Kps1MessageKind.Snapshot, snapshotPayload(5, true), 1n));
    expect(transport.state).toBe("failed");
    expect(events.some((event) => event.type === "incomplete")).toBe(true);
    expect(events.some((event) => event.type === "frame")).toBe(false);
  });

  it("marks worker loss and terminal streams without final evidence as incomplete", async () => {
    const first = open();
    await first.connecting;
    const pending = first.transport.advance(1);
    first.worker.emit({ type: "incomplete", detail: "worker terminated" });
    await expect(pending).rejects.toThrow("worker terminated");
    expect(first.transport.state).toBe("failed");
    expect(first.events.some((event) => event.type === "incomplete")).toBe(true);

    const second = open("guided-operator", "1");
    await second.connecting;
    const terminal = second.transport.advance(2);
    second.worker.emit(frame(Kps1MessageKind.Snapshot, snapshotPayload(2, false), 1n));
    second.worker.emit({ type: "advanced", requestId: 1, cursor: "1", frameCount: "1", complete: true });
    await expect(terminal).rejects.toThrow("final evidence metadata");
    expect(second.transport.complete).toBe(false);
    expect(second.transport.state).toBe("failed");
  });

  it("bounds uploads and allows only one outstanding stateless batch", async () => {
    expect(() => new VerifiedWasmReplayTransport(new Uint8Array())).toThrow("between 1 byte");
    const { worker, transport, connecting } = open();
    await connecting;
    const first = transport.advance(1);
    await expect(transport.advance(1)).rejects.toThrow("already pending");
    worker.emit({ type: "incomplete", detail: "test cleanup" });
    await expect(first).rejects.toThrow("test cleanup");
  });
});
