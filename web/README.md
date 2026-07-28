# KSA64 Mission Control Web

This directory contains the portable Mission Control PWA and the Phase 12C
global mission viewer. The UI is only a role-filtered presentation client:
Rust remains the authority for the world, flight software, operations, frames,
procedures, actions, dispositions, and evidence.

## Run locally

```text
npm install
npm test
npm run build:wasm
npm run build
npm run dev
```

The default application starts a dedicated Web Worker and loads the portable
Rust session authority from `/wasm/ksa64-session.wasm`. Each worker session
uses a fresh cryptographically generated nonzero KPS1 session nonce. Worker
failure, suspension, or termination is shown as incomplete and can never be
presented as sealed evidence.

The pace selector changes authority advancement: **Real time** schedules one 32 Hz
release every 31.25 ms for interactive operator work, while **Fast** advances in
bounded batches for observation and scripted runs. Fast mode can pass short
operator windows before a person can react, so use Real time for manual actions.

The production build has no CDN or runtime package dependency. Its manually
maintained service worker caches same-origin presentation assets while excluding
the reserved `/session/` native-authority endpoint and the per-launch
`/runtime-config.js` credential response. The broker marks that response
`no-store`, so launch tokens never enter the offline cache.

## Global mission viewer

The viewer consumes the additive, negotiated `GLOBAL_DISPLAY_V1` KPS1 stream.
Rust supplies fixed-point ECEF, GCRF, and applicable ENU poses, source validity,
frame transitions, event bookmarks, path levels, role filtering, and terminal
disposition. TypeScript does not perform accepted physical frame conversion.
The older 2-D operations stream can still produce a visibly labelled schematic
compatibility view, but it is never accepted GlobalDisplayV1 or renderer-parity
evidence.

The default hybrid mission-director layout combines the procedural WGS 84 view
with the existing operations desk. Engineering split and cinematic layouts,
automatic and manually selected cameras, exact-release replay controls, event
jumps, source-labelled paths, a screen-space locator, and a true-scale vehicle
inset are also available. SIM truth is structurally absent for ordinary roles;
a director stream may expose it, but it remains hidden until explicitly enabled
and carries a persistent label.

Babylon uses WebGPU when available, WebGL2 as the next choice, and a complete
2-D Canvas fallback. Context loss changes only the passive presentation backend.
The absolute Rust pose, selected release, action ordering, and evidence identity
are unaffected by renderer, camera, layout, polling rate, or local origin.

## Native broker mode

A native launcher may select the loopback WebSocket transport before React
starts:

```js
window.__KSA64_PRESENTATION__ = {
  mode: "remote-websocket",
  endpoint: "ws://127.0.0.1:8765/session",
  browserToken: "<64 lowercase hexadecimal characters>",
  allowedOrigin: window.location.origin,
};
```

The 256-bit launch token is sent only through the WebSocket subprotocol. The
transport rejects endpoint query strings and fragments so credentials cannot be
placed in URLs, history, or ordinary access logs. It performs the typed KPS1
handshake, preserves reconnect cursors, validates the immutable role and stream
sequence, and polls retained publications without pausing native authority.

## Replay and tests

`App` accepts any `PresentationTransport`. `VerifiedWasmReplayTransport` transfers
opaque KSB11 bytes to a dedicated worker; Rust validates the sealed archive,
strictly decodes its action transcript, re-executes the mission byte-for-byte,
and only then serves bounded role-filtered KPS1 batches. JavaScript never parses
canonical KSB11. Corruption publishes no frames, worker loss is explicit, and
end-of-stream is accepted only after final evidence metadata. The lower-level
`ReplayTransport` remains useful for already-produced KPS1 fixtures. Replay is
read-only. The nominal path in `src/model/missionReference.ts` is a labeled
presentation-only planning reference. Static data in `src/model/demo.ts`
is available only through the explicitly labeled demonstration fallback after
a live connection failure; it is never described as live or sealed evidence.

## Boundaries

- `src/protocol` implements the strict, noncanonical KPS1 envelope and typed DTO codecs.
- `src/presentation` reduces typed publications into reconnect-safe live UI state.
- `src/transport` provides local Worker, native WebSocket, and replay adapters.
- `src/workers/session.worker.ts` owns one local Rust/WASM authority worker; `replay.worker.ts` owns strict opaque-evidence replay.
- `scripts/build-local-wasm.mjs` explicitly builds and copies the raw module; Vite never invokes Cargo implicitly.
- `src/render` presents the procedural global scene through WebGPU, WebGL2, or the complete 2-D Canvas fallback.
- Babylon physics is not imported or enabled.

Only high-level Review, Stage, Commit, and Cancel proposal intents cross the
presentation boundary. Rust constructs and validates KUL11/KUA11 records through
the accepted atomic command path. Direct effector commands remain impossible.
Mission outcome remains multidimensional: objective, vehicle, procedure,
operator, avionics, and evidence dispositions stay separately visible.
