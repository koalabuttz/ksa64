import type { Kps1Frame } from "../protocol/kps1";
import {
  BasePresentationTransport,
  type PresentationActionIntent,
  type PresentationConnection,
} from "./types";

export class ReplayTransport extends BasePresentationTransport {
  readonly kind = "replay" as const;
  private cursor = 0;

  constructor(private readonly frames: readonly Kps1Frame[]) {
    super();
  }

  async connect(_connection: PresentationConnection): Promise<void> {
    this.cursor = 0;
    this.publish({ type: "state", state: "connected" });
  }

  async disconnect(): Promise<void> {
    this.publish({ type: "state", state: "closed" });
  }

  async submit(_action: PresentationActionIntent): Promise<void> {
    throw new Error("replay transport is read-only");
  }

  advance(count = 1): number {
    if (this.state !== "connected") throw new Error("replay is not open");
    if (!Number.isSafeInteger(count) || count < 0) throw new RangeError("count must be nonnegative");
    const end = Math.min(this.cursor + count, this.frames.length);
    for (; this.cursor < end; this.cursor += 1) {
      const frame = this.frames[this.cursor];
      if (frame !== undefined) this.publish({ type: "frame", frame });
    }
    return this.cursor;
  }
}
