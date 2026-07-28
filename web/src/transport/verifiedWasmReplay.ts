import { decodeKps1, Kps1Flag, Kps1MessageKind, Kps1SequenceValidator } from "../protocol/kps1";
import { decodePresentationPayload, type NumericRole } from "../protocol/presentation";
import {
  BasePresentationTransport,
  type PresentationActionIntent,
  type PresentationConnection,
  type PresentationRole,
} from "./types";

const MAX_KSB11_BYTES = 16 * 1024 * 1024;
const MAX_REPLAY_BATCH = 256;

interface ReplayWorkerMessage {
  readonly type: string;
  readonly detail?: string;
  readonly sessionNonce?: string;
  readonly frameCount?: string;
  readonly cursor?: string;
  readonly complete?: boolean;
  readonly requestId?: number;
}

interface PendingAdvance {
  readonly requested: number;
  readonly resolve: (cursor: bigint) => void;
  readonly reject: (error: Error) => void;
}

export type ReplayWorkerFactory = () => Worker;

function numericRole(role: PresentationRole): NumericRole {
  switch (role) {
    case "observer": return 1;
    case "guided-operator": return 2;
    case "flight-controller": return 3;
    case "flight-software-engineer": return 4;
    case "sim-director": return 5;
  }
}

/**
 * Opens opaque KSB11 bytes only inside the Rust/WASM replay reader. JavaScript
 * receives strict, role-filtered KPS1 frames after complete archive validation
 * and byte-identical deterministic re-execution.
 */
export class VerifiedWasmReplayTransport extends BasePresentationTransport {
  readonly kind = "replay" as const;
  private readonly archive: Uint8Array;
  private readonly workerFactory: ReplayWorkerFactory;
  private worker?: Worker;
  private expectedRole?: NumericRole;
  private sessionNonce?: bigint;
  private sequence?: Kps1SequenceValidator;
  private replayCursor = 0n;
  private replayFrameCount = 0n;
  private nextRequestId = 1;
  private readonly pending = new Map<number, PendingAdvance>();
  private sawFinalEvidence = false;
  private pendingFrameCount = 0;

  constructor(archive: ArrayBuffer | Uint8Array, workerFactory?: ReplayWorkerFactory) {
    super();
    const bytes = archive instanceof Uint8Array
      ? archive.slice()
      : new Uint8Array(archive.slice(0));
    if (bytes.byteLength === 0 || bytes.byteLength > MAX_KSB11_BYTES) {
      throw new RangeError("KSB11 replay must be between 1 byte and 16 MiB");
    }
    this.archive = bytes;
    this.workerFactory = workerFactory ?? (() => new Worker(
      new URL("../workers/replay.worker.ts", import.meta.url),
      { type: "module", name: "ksa64-replay" },
    ));
  }

  connect(connection: PresentationConnection): Promise<void> {
    if (this.state !== "idle" && this.state !== "closed") {
      return Promise.reject(new Error(`cannot open replay from ${this.state}`));
    }
    this.publish({ type: "state", state: "connecting" });
    this.expectedRole = numericRole(connection.role);
    this.sessionNonce = undefined;
    this.sequence = undefined;
    this.replayCursor = 0n;
    this.replayFrameCount = 0n;
    this.sawFinalEvidence = false;
    this.pendingFrameCount = 0;
    const worker = this.workerFactory();
    this.worker = worker;

    return new Promise((resolve, reject) => {
      let settled = false;
      const fail = (detail: string, incomplete: boolean): void => {
        if (incomplete) this.publish({ type: "incomplete", reason: detail });
        this.publish({ type: "state", state: "failed", detail });
        this.rejectPending(detail);
        worker.terminate();
        if (this.worker === worker) this.worker = undefined;
        if (!settled) {
          settled = true;
          reject(new Error(detail));
        }
      };
      worker.onmessage = (event: MessageEvent<ArrayBuffer | ReplayWorkerMessage>) => {
        if (event.data instanceof ArrayBuffer) {
          try {
            if (this.pending.size !== 1) throw new Error("replay worker sent an unsolicited frame");
            const frame = decodeKps1(event.data, { expectedSessionNonce: this.sessionNonce });
            this.sequence?.accept(frame);
            decodePresentationPayload(frame, this.expectedRole);
            if (frame.kind === Kps1MessageKind.EvidenceMetadata &&
                (frame.flags & Kps1Flag.Final) !== 0) {
              this.sawFinalEvidence = true;
            }
            this.pendingFrameCount += 1;
            this.publish({ type: "frame", frame });
          } catch (error) {
            fail(error instanceof Error ? error.message : "invalid role-filtered replay frame", true);
          }
          return;
        }
        if (event.data.type === "ready") {
          try {
            if (typeof event.data.sessionNonce !== "string" ||
                typeof event.data.frameCount !== "string" ||
                !/^[0-9]+$/u.test(event.data.sessionNonce) ||
                !/^[0-9]+$/u.test(event.data.frameCount)) {
              throw new Error("Rust replay returned invalid metadata");
            }
            const nonce = BigInt(event.data.sessionNonce);
            const frameCount = BigInt(event.data.frameCount);
            if (nonce === 0n || frameCount === 0n) throw new Error("Rust replay returned empty metadata");
            this.sessionNonce = nonce;
            this.replayFrameCount = frameCount;
            this.sequence = new Kps1SequenceValidator(nonce);
            this.publish({ type: "state", state: "connected" });
            if (!settled) {
              settled = true;
              resolve();
            }
          } catch (error) {
            fail(error instanceof Error ? error.message : "invalid Rust replay metadata", true);
          }
        } else if (event.data.type === "advanced") {
          try {
            this.finishAdvance(event.data);
          } catch (error) {
            fail(error instanceof Error ? error.message : "invalid replay advancement", true);
          }
        } else if (event.data.type === "rejected") {
          fail(event.data.detail ?? "Rust rejected the KSB11 replay", false);
        } else if (event.data.type === "incomplete") {
          fail(event.data.detail ?? "replay worker stopped before playback completed", true);
        }
      };
      worker.onerror = () => fail("replay worker failed", true);
      const upload = this.archive.slice().buffer as ArrayBuffer;
      worker.postMessage(
        { type: "initialize-replay", archive: upload, role: connection.role },
        [upload],
      );
    });
  }

  async disconnect(): Promise<void> {
    if (this.worker !== undefined) {
      this.worker.postMessage({ type: "close" });
      this.worker.terminate();
    }
    this.worker = undefined;
    this.rejectPending("replay closed");
    this.publish({ type: "state", state: "closed" });
  }

  async submit(_action: PresentationActionIntent): Promise<void> {
    throw new Error("verified KSB11 replay is read-only");
  }

  advance(count = 1): Promise<bigint> {
    if (this.state !== "connected" || this.worker === undefined) {
      return Promise.reject(new Error("replay is not open"));
    }
    if (!Number.isInteger(count) || count < 1 || count > MAX_REPLAY_BATCH) {
      return Promise.reject(new RangeError(`replay batch must be between 1 and ${MAX_REPLAY_BATCH}`));
    }
    if (this.replayCursor === this.replayFrameCount) return Promise.resolve(this.replayCursor);
    if (this.pending.size !== 0) return Promise.reject(new Error("a replay batch is already pending"));
    const requestId = this.nextRequestId;
    this.nextRequestId += 1;
    return new Promise((resolve, reject) => {
      this.pendingFrameCount = 0;
      this.pending.set(requestId, { requested: count, resolve, reject });
      try {
        this.worker?.postMessage({
          type: "read-replay",
          requestId,
          cursor: this.replayCursor.toString(),
          count,
        });
      } catch (error) {
        this.pending.delete(requestId);
        this.pendingFrameCount = 0;
        reject(error instanceof Error ? error : new Error("could not request replay batch"));
      }
    });
  }

  get cursor(): bigint { return this.replayCursor; }
  get frameCount(): bigint { return this.replayFrameCount; }
  get complete(): boolean {
    return this.replayFrameCount !== 0n &&
      this.replayCursor === this.replayFrameCount &&
      this.sawFinalEvidence;
  }

  private finishAdvance(message: ReplayWorkerMessage): void {
    if (!Number.isSafeInteger(message.requestId) || message.requestId === undefined) {
      throw new Error("replay worker returned an invalid request identity");
    }
    const pending = this.pending.get(message.requestId);
    if (pending === undefined || typeof message.cursor !== "string" ||
        typeof message.frameCount !== "string" || !/^[0-9]+$/u.test(message.cursor) ||
        !/^[0-9]+$/u.test(message.frameCount)) {
      throw new Error("replay worker returned an unsolicited advancement");
    }
    const cursor = BigInt(message.cursor);
    const frameCount = BigInt(message.frameCount);
    if (frameCount !== this.replayFrameCount || cursor < this.replayCursor ||
        cursor > frameCount || cursor - this.replayCursor > BigInt(pending.requested) ||
        cursor - this.replayCursor !== BigInt(this.pendingFrameCount) ||
        message.complete !== (cursor === frameCount)) {
      throw new Error("replay worker returned inconsistent advancement state");
    }
    if (message.complete && !this.sawFinalEvidence) {
      throw new Error("replay ended without final evidence metadata");
    }
    this.pending.delete(message.requestId);
    this.replayCursor = cursor;
    this.pendingFrameCount = 0;
    pending.resolve(cursor);
  }

  private rejectPending(detail: string): void {
    for (const pending of this.pending.values()) pending.reject(new Error(detail));
    this.pending.clear();
    this.pendingFrameCount = 0;
  }
}
