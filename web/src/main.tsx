import { createRoot } from "react-dom/client";
import { App } from "./App";
import { installPhase12cBrowserEvidenceHarness } from "./evidence/phase12cBrowserEvidence";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("KSA64 root element is missing");

const query = new URLSearchParams(window.location.search);
const evidenceMode = query.get("phase12c-evidence") === "1";
const requestedExperience = query.get("experience");
const evidenceExperience = evidenceMode &&
  (requestedExperience === "gnss-loss" || requestedExperience === "nominal-global")
  ? requestedExperience
  : undefined;

if (evidenceExperience !== undefined) {
  const configured = window.__KSA64_PRESENTATION__;
  if (configured?.mode === "remote-websocket") {
    throw new Error("Phase 12C rendered-browser evidence requires the local verified Rust/WASM transport");
  }
  Object.defineProperty(window, "__KSA64_PRESENTATION__", {
    configurable: true,
    value: { ...configured, mode: "local-worker", experience: evidenceExperience },
  });
}

// The authority session is intentionally mounted once. React development
// StrictMode's synthetic remount would otherwise create and tear down a real
// worker session twice.
createRoot(root).render(<App experience={evidenceExperience} />);

if (evidenceMode) installPhase12cBrowserEvidenceHarness();

if ("serviceWorker" in navigator && import.meta.env.PROD) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register("/sw.js", { scope: "/" });
  });
}
