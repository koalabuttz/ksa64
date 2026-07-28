import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { Kps1MessageKind, type Kps1Frame } from "./protocol/kps1";
import { BasePresentationTransport, type PresentationActionIntent, type PresentationConnection } from "./transport";
import { proposalPayload, receiptPayload, snapshotPayload, testFrame } from "./test/presentationFixtures";

vi.mock("./components/GlobalMissionViewer", () => ({
  GlobalMissionViewer: () => <section aria-label="Global mission director">Global viewer</section>,
}));

class TestTransport extends BasePresentationTransport {
  readonly kind = "replay" as const;
  readonly actions: PresentationActionIntent[] = [];
  readonly paces: string[] = [];
  private receiptSequence = 1n;
  constructor(private readonly fail = false) { super(); }
  async connect(_connection: PresentationConnection): Promise<void> {
    if (this.fail) { this.publish({ type: "state", state: "failed", detail: "test authority unavailable" }); throw new Error("test authority unavailable"); }
    this.publish({ type: "state", state: "connected" });
  }
  async disconnect(): Promise<void> { this.publish({ type: "state", state: "closed" }); }
  async submit(action: PresentationActionIntent): Promise<void> {
    this.actions.push(action);
    const operation = ({ review: 1, stage: 2, commit: 3, cancel: 4 } as const)[action.kind];
    this.emit(testFrame(Kps1MessageKind.ActionReceipt, receiptPayload(operation, this.receiptSequence), this.receiptSequence));
    this.receiptSequence += 1n;
    if (action.kind === "stage") this.emit(testFrame(Kps1MessageKind.ActionProposal, proposalPayload(1 | 4 | 8), 50n));
  }
  async setPace(pace: "fast" | "realtime"): Promise<void> { this.paces.push(pace); }
  emit(frame: Kps1Frame): void { this.publish({ type: "frame", frame }); }
  incomplete(reason: string): void { this.publish({ type: "incomplete", reason }); }
}

describe("live compact Mission Control desk", () => {
  it("starts from a role-filtered live state without claiming fixture authority", async () => {
    const transport = new TestTransport(); render(<App transport={transport} />);
    expect(await screen.findByText("Role-filtered replay")).toBeInTheDocument();
    expect(screen.getByText("Role-filtered · no private truth")).toBeInTheDocument();
    expect(screen.getByText("Evidence accumulating")).toBeInTheDocument();
    expect(screen.queryByText("Contingency success")).not.toBeInTheDocument();
    act(() => transport.emit(testFrame(Kps1MessageKind.Snapshot, snapshotPayload(2))));
    expect(await screen.findByText("6,000 / 21,591")).toBeInTheDocument();
  });

  it("submits Review, Stage, then Commit through the high-level transport", async () => {
    const transport = new TestTransport(); render(<App transport={transport} />);
    act(() => transport.emit(testFrame(Kps1MessageKind.ActionProposal, proposalPayload())));
    const review = await screen.findByRole("button", { name: "review" });
    const stage = screen.getByRole("button", { name: "stage" });
    const commit = screen.getByRole("button", { name: "commit" });
    expect(review).toBeEnabled(); expect(stage).toBeDisabled(); expect(commit).toBeDisabled();
    fireEvent.click(review);
    await waitFor(() => expect(stage).toBeEnabled());
    expect(transport.actions[0]).toMatchObject({ kind: "review", proposalIdentity: 0x47535504 });
    fireEvent.click(stage);
    await waitFor(() => expect(commit).toBeEnabled());
    fireEvent.click(commit);
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("commit accepted"));
    expect(transport.actions.map((value) => value.kind)).toEqual(["review", "stage", "commit"]);
  });

  it("labels the static fixture explicitly and only opens it after a live failure", async () => {
    render(<App transport={new TestTransport(true)} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Live presentation unavailable");
    expect(screen.queryByText("Contingency success")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open labeled demonstration fixture" }));
    expect(screen.getByText("Demonstration fixture")).toBeInTheDocument();
    expect(screen.getByText("Static test data · no authority")).toBeInTheDocument();
    expect(screen.getByText("DEMONSTRATION FIXTURE · no live authority")).toBeInTheDocument();
    expect(screen.getAllByText("Contingency success").length).toBeGreaterThan(0);
  });

  it("offers a high-contrast presentation toggle", () => {
    render(<App transport={new TestTransport()} />);
    const toggle = screen.getByRole("button", { name: "High contrast" }); fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-pressed", "true");
  });

  it("routes pacing through the authority transport", async () => {
    const transport = new TestTransport(); render(<App transport={transport} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Session pace" }), { target: { value: "fast" } });
    await waitFor(() => expect(transport.paces).toEqual(["fast"]));
  });
});
