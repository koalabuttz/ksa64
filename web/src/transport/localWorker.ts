import { decodeKps1, Kps1SequenceValidator } from "../protocol/kps1";
import {
  BasePresentationTransport,
  type PresentationActionIntent,
  type PresentationConnection,
  type PresentationPaceMode,
} from "./types";

interface LocalWorkerMessage {
  readonly type: string;
  readonly detail?: string;
  readonly sessionNonce?: string;
}

export class LocalWorkerTransport extends BasePresentationTransport {
  readonly kind = "local-worker" as const;
  private worker?: Worker;
  private sessionNonce?: bigint;
  private sequence?: Kps1SequenceValidator;

  connect(connection: PresentationConnection): Promise<void> {
    if (this.state !== "idle" && this.state !== "closed") {
      return Promise.reject(new Error(`cannot connect transport from ${this.state}`));
    }
    this.publish({ type: "state", state: "connecting" });
    // Local WASM authority owns a fixed session nonce. The caller-supplied nonce
    // is for remote/replay transports and must not be mistaken for local authority.
    this.sessionNonce = undefined;
    this.sequence = undefined;
    const worker = new Worker(new URL("../workers/session.worker.ts", import.meta.url), {
      type: "module",
      name: "ksa64-session",
    });
    this.worker = worker;

    return new Promise((resolve, reject) => {
      let settled = false;
      const fail = (detail: string, incomplete: boolean): void => {
        if (incomplete) this.publish({ type: "incomplete", reason: detail });
        this.publish({ type: "state", state: "failed", detail });
        if (!settled) {
          settled = true;
          reject(new Error(detail));
        }
      };
      worker.onmessage = (event: MessageEvent<ArrayBuffer | LocalWorkerMessage>) => {
        if (event.data instanceof ArrayBuffer) {
          try {
            const frame = decodeKps1(event.data, { expectedSessionNonce: this.sessionNonce });
            this.sequence?.accept(frame);
            this.publish({ type: "frame", frame });
          } catch (error) {
            fail(error instanceof Error ? error.message : "invalid worker frame", false);
            void this.disconnect();
          }
          return;
        }
        if (event.data.type === "ready") {
          const nonce = event.data.sessionNonce;
          if (typeof nonce !== "string" || !/^[0-9]+$/u.test(nonce)) {
            fail("local authority returned an invalid session nonce", true);
            void this.disconnect();
            return;
          }
          this.sessionNonce = BigInt(nonce);
          this.sequence = new Kps1SequenceValidator(this.sessionNonce);
          this.publish({ type: "state", state: "connected" });
          if (!settled) {
            settled = true;
            resolve();
          }
        } else if (event.data.type === "incomplete") {
          fail(event.data.detail ?? "local authority stopped before finalization", true);
        } else if (event.data.type === "error") {
          this.publish({ type: "state", state: "stale", detail: event.data.detail ?? "local action was rejected" });
        }
      };
      worker.onerror = () => fail("local authority worker failed", true);
      worker.postMessage({ type: "initialize", connection });
    });
  }

  async disconnect(): Promise<void> {
    if (this.worker !== undefined) {
      this.worker.postMessage({ type: "close" });
      this.worker.terminate();
    }
    this.worker = undefined;
    this.publish({ type: "state", state: "closed" });
  }

  async setPace(pace: PresentationPaceMode): Promise<void> {
    if (this.state !== "connected" || this.worker === undefined) throw new Error("local worker is not connected");
    this.worker.postMessage({ type: "pace", pace });
  }

  async submit(action: PresentationActionIntent): Promise<void> {
    if (this.state !== "connected" || this.worker === undefined) {
      throw new Error("local worker is not connected");
    }
    this.worker.postMessage({ type: "action", action });
  }
}
