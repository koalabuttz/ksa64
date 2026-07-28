import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("KSA64 root element is missing");

// The authority session is intentionally mounted once. React development
// StrictMode's synthetic remount would otherwise create and tear down a real
// worker session twice.
createRoot(root).render(<App />);

if ("serviceWorker" in navigator && import.meta.env.PROD) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register("/sw.js", { scope: "/" });
  });
}
