import { LocalWorkerTransport } from "./localWorker";
import { RemoteWebSocketTransport } from "./remoteWebSocket";
import type { PresentationTransport } from "./types";

export interface Ksa64BrowserRuntimeConfig {
  readonly mode?: "local-worker" | "remote-websocket";
  readonly endpoint?: string;
  readonly browserToken?: string;
  readonly allowedOrigin?: string;
  readonly experience?: "gnss-loss" | "nominal-global";
}

declare global {
  interface Window { readonly __KSA64_PRESENTATION__?: Ksa64BrowserRuntimeConfig; }
}

export function createDefaultPresentationTransport(): PresentationTransport {
  const config = window.__KSA64_PRESENTATION__;
  if (config?.mode === "remote-websocket") {
    if (config.endpoint === undefined || config.browserToken === undefined) {
      throw new Error("remote presentation configuration requires endpoint and browser token");
    }
    return new RemoteWebSocketTransport({ url: new URL(config.endpoint), browserToken: config.browserToken,
      allowedOrigin: config.allowedOrigin ?? window.location.origin });
  }
  return new LocalWorkerTransport(config?.experience ?? "gnss-loss");
}
