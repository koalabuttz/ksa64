import { GLOBAL_DISPLAY_V1_CAPABILITY } from "../protocol/globalDisplay";
import {
  decodeKps1,
  encodeKps1,
  Kps1MessageKind,
  Kps1SequenceValidator,
  type Kps1Frame,
} from "../protocol/kps1";
import {
  decodeHandshakePayload,
  decodePresentationPayload,
  encodeActionIntentPayload,
  encodeCursorsPayload,
  encodeHandshakePayload,
  encodeLifecycleControlPayload,
  encodePaceControlPayload,
  initialPresentationCursors,
  type NumericRole,
  type PresentationCursorsView,
} from "../protocol/presentation";
import {
  BasePresentationTransport,
  type PresentationActionIntent,
  type PresentationConnection,
  type PresentationPaceMode,
  type PresentationRole,
} from "./types";

export interface RemoteWebSocketOptions {
  readonly url: URL;
  /** 64 hexadecimal characters, supplied only as a WebSocket subprotocol. */
  readonly browserToken: string;
  readonly allowedOrigin?: string;
  readonly pollIntervalMillis?: number;
}

const ACTION_CODE = { review: 1, stage: 2, commit: 3, cancel: 4 } as const;

function numericRole(role: PresentationRole): NumericRole {
  switch (role) {
    case "observer": return 1;
    case "guided-operator": return 2;
    case "flight-controller": return 3;
    case "flight-software-engineer": return 4;
    case "sim-director": return 5;
  }
}

function createClientInstance(): bigint {
  const values = new BigUint64Array(1);
  globalThis.crypto.getRandomValues(values);
  return (values[0] ?? 0n) || 1n;
}

export class RemoteWebSocketTransport extends BasePresentationTransport {
  readonly kind = "remote-websocket" as const;
  private socket?: WebSocket;
  private sessionNonce?: bigint;
  private expectedRole?: NumericRole;
  private incoming?: Kps1SequenceValidator;
  private outgoingSequence = 1n;
  private correlation = 1n;
  private actionSequence = 1n;
  private clientInstance?: bigint;
  private pollTimer?: ReturnType<typeof setInterval>;
  private cursors: PresentationCursorsView = initialPresentationCursors();

  constructor(private readonly options: RemoteWebSocketOptions) {
    super();
    if (options.url.protocol !== "ws:" && options.url.protocol !== "wss:") {
      throw new TypeError("presentation endpoint must use ws: or wss:");
    }
    if (options.url.search !== "" || options.url.hash !== "") {
      throw new TypeError("presentation credentials and state are forbidden in endpoint URLs");
    }
    if (!/^[0-9a-f]{64}$/iu.test(options.browserToken)) {
      throw new TypeError("browser token must be exactly 256 bits encoded as hexadecimal");
    }
  }

  async connect(connection: PresentationConnection): Promise<void> {
    if (!["idle", "closed", "stale", "failed"].includes(this.state)) {
      throw new Error(`cannot connect transport from ${this.state}`);
    }
    this.stopPolling();
    const priorSocket = this.socket;
    this.socket = undefined;
    if (priorSocket !== undefined) await this.closeSocket(priorSocket, "presentation reconnecting");
    if (this.options.allowedOrigin !== undefined && window.location.origin !== this.options.allowedOrigin) {
      return Promise.reject(new Error("page origin is not permitted by this transport"));
    }
    this.publish({ type: "state", state: "connecting" });
    this.expectedRole = numericRole(connection.role);
    const expectedReconnectNonce = connection.sessionNonce ?? this.sessionNonce;
    this.sessionNonce = undefined;
    this.clientInstance = connection.clientInstance ?? this.clientInstance ?? createClientInstance();
    this.cursors = {
      snapshots: connection.snapshotCursor ?? this.cursors.snapshots,
      events: connection.eventCursor ?? this.cursors.events,
      timeline: connection.timelineCursor ?? this.cursors.timeline,
      actionReceipts: connection.actionReceiptCursor ?? this.cursors.actionReceipts,
      releaseSamples: connection.releaseSampleCursor ?? this.cursors.releaseSamples,
    };
    this.outgoingSequence = 1n; this.correlation = 1n;

    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const socket = new WebSocket(
        this.options.url,
        `ksa64.presentation.v1.token.${this.options.browserToken.toLowerCase()}`,
      );
      socket.binaryType = "arraybuffer";
      this.socket = socket;

      const fail = (detail: string): void => {
        this.stopPolling();
        this.publish({ type: "state", state: "failed", detail });
        if (!settled) { settled = true; reject(new Error(detail)); }
      };

      socket.addEventListener("open", () => {
        try {
          const payload = encodeHandshakePayload({
            role: this.expectedRole ?? 2,
            clientInstance: this.clientInstance ?? createClientInstance(),
            capabilityMask: GLOBAL_DISPLAY_V1_CAPABILITY,
            cursors: this.cursors,
          });
          socket.send(encodeKps1({ kind: Kps1MessageKind.HandshakeRequest, flags: 0,
            sessionNonce: 0n, sequence: this.outgoingSequence++, correlationId: this.correlation++, payload }));
        } catch (error) {
          fail(error instanceof Error ? error.message : "handshake creation failed");
          socket.close(1002, "invalid handshake");
        }
      }, { once: true });

      socket.addEventListener("message", (event) => {
        if (!(event.data instanceof ArrayBuffer)) {
          fail("non-binary server message"); socket.close(1003, "binary messages required"); return;
        }
        try {
          const frame = decodeKps1(event.data, { expectedSessionNonce: this.sessionNonce });
          if (this.sessionNonce === undefined) {
            if (frame.kind !== Kps1MessageKind.HandshakeResponse || frame.sequence !== 1n) {
              throw new Error("server did not begin with a KPS1 handshake response");
            }
            const handshake = decodeHandshakePayload(frame.payload);
            if (handshake.role !== this.expectedRole) throw new Error("immutable role mismatch");
            if (expectedReconnectNonce !== undefined && frame.sessionNonce !== expectedReconnectNonce) throw new Error("reconnect session nonce mismatch");
            this.sessionNonce = frame.sessionNonce;
            this.cursors = handshake.cursors;
            this.incoming = new Kps1SequenceValidator(frame.sessionNonce);
            this.incoming.accept(frame);
            this.sendClientFrame(
              Kps1MessageKind.LifecycleControl,
              encodeLifecycleControlPayload("running"),
            );
            this.publish({ type: "state", state: "connected" });
            this.startPolling();
            if (!settled) { settled = true; resolve(); }
            return;
          }
          this.incoming?.accept(frame);
          this.observeCursors(frame);
          this.publish({ type: "frame", frame });
        } catch (error) {
          fail(error instanceof Error ? error.message : "invalid KPS1 frame");
          socket.close(1002, "invalid KPS1 frame");
        }
      });
      socket.addEventListener("close", () => {
        if (this.socket !== socket) return;
        this.stopPolling();
        if (this.state !== "failed" && this.state !== "closed") {
          this.publish({ type: "state", state: "stale", detail: "presentation disconnected; authority continues" });
        }
      });
      socket.addEventListener("error", () => fail("WebSocket connection failed"), { once: true });
    });
  }

  async disconnect(): Promise<void> {
    this.stopPolling();
    const socket = this.socket;
    this.socket = undefined;
    if (socket !== undefined) await this.closeSocket(socket, "presentation closed");
    this.publish({ type: "state", state: "closed" });
  }

  async setPace(pace: PresentationPaceMode): Promise<void> {
    if (this.socket?.readyState !== WebSocket.OPEN || this.sessionNonce === undefined) throw new Error("presentation transport is not connected");
    this.sendClientFrame(Kps1MessageKind.PaceControl, encodePaceControlPayload(pace));
  }

  async submit(action: PresentationActionIntent): Promise<void> {
    if (this.socket?.readyState !== WebSocket.OPEN || this.sessionNonce === undefined) {
      throw new Error("presentation transport is not connected");
    }
    const proposalIdentity = action.proposalIdentity ?? Number.parseInt(action.proposalId, 10);
    if (!Number.isSafeInteger(proposalIdentity) || proposalIdentity <= 0) throw new Error("action lacks a numeric proposal identity");
    const payload = encodeActionIntentPayload({
      proposalIdentity,
      expectedLoadIdentity: action.expectedLoadIdentity ?? (action.kind === "review" ? 0 : 0),
      operation: ACTION_CODE[action.kind],
      requestedActivationEpoch: action.requestedActivationEpoch ?? 0,
      clientActionSequence: this.actionSequence++,
    });
    this.sendClientFrame(Kps1MessageKind.ActionIntent, payload);
  }

  private closeSocket(socket: WebSocket, reason: string): Promise<void> {
    if (socket.readyState === WebSocket.CLOSED) return Promise.resolve();
    return new Promise((resolve) => {
      let settled = false;
      const finish = (): void => {
        if (settled) return;
        settled = true;
        globalThis.clearTimeout(timeout);
        resolve();
      };
      const timeout = globalThis.setTimeout(finish, 1_000);
      socket.addEventListener("close", finish, { once: true });
      socket.close(1000, reason);
    });
  }

  private sendClientFrame(kind: Kps1MessageKind, payload: Uint8Array): void {
    if (this.socket?.readyState !== WebSocket.OPEN || this.sessionNonce === undefined) return;
    const frame = encodeKps1({ kind, flags: 0, sessionNonce: this.sessionNonce,
      sequence: this.outgoingSequence++, correlationId: this.correlation++, payload });
    this.socket.send(frame);
  }

  private startPolling(): void {
    this.stopPolling();
    const interval = Math.max(50, this.options.pollIntervalMillis ?? 125);
    this.pollTimer = setInterval(() => this.sendClientFrame(Kps1MessageKind.ReplayControl, encodeCursorsPayload(this.cursors)), interval);
    this.sendClientFrame(Kps1MessageKind.ReplayControl, encodeCursorsPayload(this.cursors));
  }

  private stopPolling(): void {
    if (this.pollTimer !== undefined) clearInterval(this.pollTimer);
    this.pollTimer = undefined;
  }

  private observeCursors(frame: Kps1Frame): void {
    const decoded = decodePresentationPayload(frame, this.expectedRole);
    switch (decoded.kind) {
      case "snapshot": this.cursors = { ...this.cursors, snapshots: decoded.value.publicationSequence }; break;
      case "events": if (decoded.value.length > 0) this.cursors = { ...this.cursors, events: decoded.value.at(-1)!.sequence + 1n }; break;
      case "timeline": this.cursors = { ...this.cursors, timeline: decoded.value.sequence + 1n }; break;
      case "action-receipt": this.cursors = { ...this.cursors, actionReceipts: decoded.value.publicationSequence + 1n }; break;
      case "samples": if (decoded.value.length > 0) this.cursors = { ...this.cursors, releaseSamples: decoded.value.at(-1)!.sequence + 1n }; break;
      default: break;
    }
  }
}
