import type { Kps1Frame } from "../protocol/kps1";

export type PresentationRole =
  | "observer"
  | "guided-operator"
  | "flight-controller"
  | "flight-software-engineer"
  | "sim-director";

export type PresentationTransportState =
  | "idle"
  | "connecting"
  | "connected"
  | "stale"
  | "resync-required"
  | "closed"
  | "failed";

export type PresentationActionKind = "review" | "stage" | "commit" | "cancel";
export type PresentationPaceMode = "fast" | "realtime";

export interface PresentationActionIntent {
  readonly kind: PresentationActionKind;
  readonly proposalId: string;
  readonly proposalIdentity?: number;
  readonly expectedLoadIdentity?: number;
  readonly requestedActivationEpoch?: number;
}

export interface PresentationConnection {
  readonly role: PresentationRole;
  readonly sessionNonce?: bigint;
  readonly snapshotCursor?: bigint;
  readonly eventCursor?: bigint;
  readonly timelineCursor?: bigint;
  readonly actionReceiptCursor?: bigint;
  readonly releaseSampleCursor?: bigint;
  readonly clientInstance?: bigint;
}

export type PresentationTransportEvent =
  | { readonly type: "state"; readonly state: PresentationTransportState; readonly detail?: string }
  | { readonly type: "frame"; readonly frame: Kps1Frame }
  | { readonly type: "incomplete"; readonly reason: string };

export type PresentationTransportListener = (event: PresentationTransportEvent) => void;

export interface PresentationTransport {
  readonly kind: "remote-websocket" | "local-worker" | "replay";
  readonly state: PresentationTransportState;
  connect(connection: PresentationConnection): Promise<void>;
  disconnect(): Promise<void>;
  submit(action: PresentationActionIntent): Promise<void>;
  setPace?(pace: PresentationPaceMode): Promise<void>;
  subscribe(listener: PresentationTransportListener): () => void;
}

export abstract class BasePresentationTransport implements PresentationTransport {
  abstract readonly kind: PresentationTransport["kind"];
  protected currentState: PresentationTransportState = "idle";
  protected readonly listeners = new Set<PresentationTransportListener>();

  get state(): PresentationTransportState {
    return this.currentState;
  }

  abstract connect(connection: PresentationConnection): Promise<void>;
  abstract disconnect(): Promise<void>;
  abstract submit(action: PresentationActionIntent): Promise<void>;

  subscribe(listener: PresentationTransportListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  protected publish(event: PresentationTransportEvent): void {
    if (event.type === "state") this.currentState = event.state;
    for (const listener of this.listeners) listener(event);
  }
}
