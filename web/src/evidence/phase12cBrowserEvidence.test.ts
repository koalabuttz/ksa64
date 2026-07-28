import { cleanup, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { installPhase12cBrowserEvidenceHarness } from "./phase12cBrowserEvidence";

afterEach(() => {
  cleanup();
  document.getElementById("phase12c-evidence-controls")?.remove();
  delete window.__KSA64_PHASE12C_EVIDENCE__;
  window.history.replaceState({}, "", "/");
});

describe("Phase 12C browser evidence controls", () => {
  it("exposes a bounded visible nominal capture lane only when explicitly requested", () => {
    window.history.replaceState({}, "", "/?phase12c-evidence=1&evidence-controls=1&experience=nominal-global");
    installPhase12cBrowserEvidenceHarness();
    expect(screen.getByRole("region", { name: "Phase 12C evidence controls" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run nominal" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Run guided" })).toBeDisabled();
    const output = screen.getByRole("textbox", { name: "Canonical Phase 12C evidence JSON" });
    expect(output).toHaveAttribute("readonly");
    expect(output).toHaveValue("");
  });

  it("does not add controls without the explicit evidence-controls query", () => {
    window.history.replaceState({}, "", "/?phase12c-evidence=1&experience=nominal-global");
    installPhase12cBrowserEvidenceHarness();
    expect(screen.queryByRole("region", { name: "Phase 12C evidence controls" })).not.toBeInTheDocument();
  });
});
