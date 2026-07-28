import { afterEach, describe, expect, it, vi } from "vitest";
import { decodeKps1, encodeKps1, Kps1Flag, Kps1MessageKind } from "../protocol/kps1";
import { decodeHandshakePayload, encodeHandshakePayload, initialPresentationCursors } from "../protocol/presentation";
import { RemoteWebSocketTransport } from "./remoteWebSocket";

class FakeSocket {
  static readonly OPEN = 1;
  readonly sent: ArrayBuffer[] = [];
  readonly listeners = new Map<string, ((event: any) => void)[]>();
  binaryType = "";
  readyState = FakeSocket.OPEN;
  constructor(readonly url: URL, readonly protocols: string | string[]) { sockets.push(this); }
  addEventListener(type: string, listener: (event: any) => void): void {
    const values = this.listeners.get(type) ?? []; values.push(listener); this.listeners.set(type, values);
  }
  send(value: ArrayBuffer | ArrayBufferView): void {
    const bytes = value instanceof ArrayBuffer ? value.slice(0) : value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength);
    this.sent.push(bytes as ArrayBuffer);
  }
  close(): void { this.readyState = 3; this.emit("close", {}); }
  emit(type: string, event: any): void { for (const listener of this.listeners.get(type) ?? []) listener(event); }
}
const sockets: FakeSocket[] = [];
const originalWebSocket = globalThis.WebSocket;
afterEach(() => { globalThis.WebSocket = originalWebSocket; sockets.length = 0; vi.restoreAllMocks(); });

describe("remote WebSocket presentation transport", () => {
  it("forbids tokens or state in endpoint URLs", () => {
    const token = "ab".repeat(32);
    expect(() => new RemoteWebSocketTransport({ url: new URL("ws://127.0.0.1:9000/session?token=oops"), browserToken: token })).toThrow("forbidden");
    expect(() => new RemoteWebSocketTransport({ url: new URL("ws://127.0.0.1:9000/session"), browserToken: "short" })).toThrow("256 bits");
  });

  it("authenticates in the subprotocol, handshakes, polls, and submits typed high-level intent", async () => {
    globalThis.WebSocket = FakeSocket as unknown as typeof WebSocket;
    const token = "cd".repeat(32);
    const transport = new RemoteWebSocketTransport({ url: new URL("ws://127.0.0.1:9000/session"), browserToken: token, pollIntervalMillis: 60_000 });
    const connecting = transport.connect({ role: "guided-operator", clientInstance: 77n });
    const socket = sockets[0]!;
    expect(socket.url.toString()).not.toContain(token);
    expect(socket.protocols).toBe(`ksa64.presentation.v1.token.${token}`);
    socket.emit("open", {});
    const request = decodeKps1(socket.sent[0]!);
    expect(request).toMatchObject({ kind: Kps1MessageKind.HandshakeRequest, sessionNonce: 0n, sequence: 1n });
    const response = encodeKps1({ kind: Kps1MessageKind.HandshakeResponse, flags: Kps1Flag.Response,
      sessionNonce: 99n, sequence: 1n, correlationId: 1n,
      payload: encodeHandshakePayload({ role: 2, clientInstance: 77n, capabilityMask: 0n, cursors: initialPresentationCursors() }) });
    socket.emit("message", { data: response.buffer });
    await connecting;
    expect(transport.state).toBe("connected");
    const lifecycle = decodeKps1(socket.sent[1]!);
    expect(lifecycle.kind).toBe(Kps1MessageKind.LifecycleControl);
    expect(new TextDecoder().decode(lifecycle.payload.subarray(0, 4))).toBe("PLC1");
    expect(lifecycle.payload[12]).toBe(3);
    await transport.setPace("fast");
    const pace = decodeKps1(socket.sent.at(-1)!);
    expect(pace.kind).toBe(Kps1MessageKind.PaceControl);
    expect(pace.payload[12]).toBe(1);
    await transport.submit({ kind: "review", proposalId: "GSU-04", proposalIdentity: 9,
      expectedLoadIdentity: 10, requestedActivationEpoch: 20 });
    const action = decodeKps1(socket.sent.at(-1)!);
    expect(action.kind).toBe(Kps1MessageKind.ActionIntent);
    expect(new TextDecoder().decode(action.payload.subarray(0, 4))).toBe("PAI1");
    await transport.disconnect();

    const reconnecting = transport.connect({ role: "guided-operator", sessionNonce: 99n });
    const second = sockets[1]!; second.emit("open", {});
    const secondRequest = decodeKps1(second.sent[0]!);
    expect(decodeHandshakePayload(secondRequest.payload).clientInstance).toBe(77n);
    const secondResponse = encodeKps1({ kind: Kps1MessageKind.HandshakeResponse, flags: Kps1Flag.Response,
      sessionNonce: 99n, sequence: 1n, correlationId: 1n,
      payload: encodeHandshakePayload({ role: 2, clientInstance: 77n, capabilityMask: 0n, cursors: initialPresentationCursors() }) });
    second.emit("message", { data: secondResponse.buffer });
    await reconnecting; expect(transport.state).toBe("connected");
    await transport.submit({ kind: "stage", proposalId: "GSU-04", proposalIdentity: 9,
      expectedLoadIdentity: 10, requestedActivationEpoch: 20 });
    const resumedAction = decodeKps1(second.sent.at(-1)!);
    expect(resumedAction.kind).toBe(Kps1MessageKind.ActionIntent);
    expect(new DataView(resumedAction.payload.buffer, resumedAction.payload.byteOffset, resumedAction.payload.byteLength)
      .getBigUint64(24, true)).toBe(2n);
    await transport.disconnect();
  });

  it("negotiates GlobalDisplayV1 and issues an explicit resumable range request", async () => {
    globalThis.WebSocket = FakeSocket as unknown as typeof WebSocket;
    const transport = new RemoteWebSocketTransport({
      url: new URL("ws://127.0.0.1:9000/session"),
      browserToken: "ef".repeat(32),
      pollIntervalMillis: 60_000,
    });
    const connecting = transport.connect({ role: "guided-operator", clientInstance: 88n });
    const socket = sockets[0]!;
    socket.emit("open", {});
    const response = encodeKps1({ kind: Kps1MessageKind.HandshakeResponse, flags: Kps1Flag.Response,
      sessionNonce: 100n, sequence: 1n, correlationId: 1n,
      payload: encodeHandshakePayload({ role: 2, clientInstance: 88n, capabilityMask: 1n << 8n,
        cursors: initialPresentationCursors() }) });
    socket.emit("message", { data: response.buffer });
    await connecting;
    const kinds = socket.sent.slice(1).map((bytes) => decodeKps1(bytes).kind);
    expect(kinds).toContain(Kps1MessageKind.GlobalDisplayRangeRequest);
    const rangeFrame = socket.sent.slice(1).map((bytes) => decodeKps1(bytes))
      .find((frame) => frame.kind === Kps1MessageKind.GlobalDisplayRangeRequest)!;
    expect(new TextDecoder().decode(rangeFrame.payload.subarray(0, 4))).toBe("PGR1");
    expect(new DataView(rangeFrame.payload.buffer, rangeFrame.payload.byteOffset, rangeFrame.payload.byteLength)
      .getUint32(12, true)).toBe(0);
    await transport.disconnect();
  });
});
