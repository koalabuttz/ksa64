# Phase 12 web platform decision

Status: accepted for forward planning.

Date: 2026-07-27

## Selected stack

KSA64's native web product uses:

- React with TypeScript for the accessible Mission Control and PWA shell;
- Vite for development and deterministic production bundling, with an explicit
  audited manifest/service-worker layer for PWA packaging;
- Babylon.js through tree-shakeable `@babylonjs/*` ES modules for 3-D;
- Rust compiled for `wasm32-unknown-unknown` for optional local mission authority;
- a dedicated Web Worker for the Rust world, flight package, operations engine,
  role filtering, and evidence construction;
- IndexedDB and explicit file download/import for browser persistence; and
- the Phase 12B.5 typed presentation-session protocol over transferable buffers
  locally or an ordered WebSocket remotely.

Babylon.js is consumed under its Apache-2.0 license and recorded in the
third-party provenance inventory. Dependency versions are pinned in the web
lockfile and recorded in completion evidence. Production builds self-host their JavaScript, WebAssembly, shaders,
and assets rather than depending on a learning CDN.

Babylon is used directly through an imperative scene adapter. React owns DOM
layout and application controls but does not own Babylon's render loop. Babylon
GUI is reserved for labels or interactions located inside the 3-D scene; normal
Mission Control uses semantic HTML, CSS, SVG, and Canvas so keyboard, touch,
scaling, and accessibility remain ordinary browser concerns.

## Why Babylon.js

Babylon supplies the capabilities KSA64 would otherwise have to build around a
lower-level renderer:

- maintained WebGPU and WebGL implementations;
- explicit large-world rendering with high-precision CPU matrices and a
  camera-relative floating origin for GPU work;
- geospatial camera and 3-D Tiles support;
- glTF/GLB loading, PBR materials, picking, LOD, instancing, and scene tools;
- KTX2/Basis compressed textures for constrained and mobile devices; and
- TypeScript definitions, ES modules, and tree-shaking.

The client attempts `WebGPUEngine` only after an explicit capability check and
successful asynchronous initialization. Failure selects a separately tested
WebGL2 `Engine`; it never leaves a partially initialized WebGPU scene in use.
A 2-D-only mode remains valid when neither 3-D backend meets the device gate.

Three.js remains a credible alternative, but its universal WebGPU renderer is
still a moving, explicitly experimental surface and would require more
KSA64-specific large-world and engineering-viewer infrastructure. A raw Rust
`wgpu` renderer would maximize control while making KSA64 maintain an engine,
asset pipeline, materials, picking, and browser integration. CesiumJS remains a
useful reference and potential 3-D Tiles source, but it does not own KSA64 frame
or time semantics. Unreal Pixel Streaming is an optional remote high-fidelity
viewport, not the native web product.

## Authority boundary

Babylon is disposable presentation. It never owns or feeds back:

- physics or numerical integration;
- ENU, ECEF, GCRF, Earth-orientation, or simulation-time authority;
- atmosphere, gravity, guidance, navigation, control, or recovery;
- authoritative event detection or mission outcome classification;
- procedures, role permissions, action validation, or evidence; or
- a browser-side interpretation of canonical K records.

Babylon's physics integrations are not used. Rust converts authoritative global
state into role-filtered, camera-relative display samples plus explicit frame,
source, validity, and event metadata. Babylon may interpolate those samples for
smooth presentation. Exact events snap to their accepted release sample, and
no interpolated value can re-enter the mission.

## Execution modes

The same application supports three primary modes:

| Mode | Authority | Browser responsibility |
|---|---|---|
| Remote live | Native Windows/Linux/macOS, Crostini, or another KSA64 endpoint | Mission Control, 3-D, actions, and bounded replay views |
| Local WebAssembly | Dedicated Rust Web Worker in the PWA | Mission Control and 3-D on the main thread |
| Replay/debrief | Rust browser evidence adapter over an accepted archive | Passive inspection and presentation |

Optional Unreal Pixel Streaming is a fourth presentation mode. Streamed input
may manipulate cameras and presentation only; operational actions still use the
ordinary KSA64 action broker.

## Fully local browser placement

A browser may own the complete mission without Crostini, Android, or a remote
host:

```text
Browser main thread
  React Mission Control + Babylon.js viewer
                    |
       bounded transferable messages
                    |
Dedicated Web Worker
  Rust/WASM browser session
  world + truth-blind flight package
  operations + actions + role filtering
  canonical in-memory KSB11 construction
                    |
        IndexedDB / explicit download
```

The initial implementation is single-threaded inside one authoritative worker.
World and flight retain their serialized sensor/command boundaries, but ordinary
JavaScript scheduling cannot interleave them. The main thread never advances a
physical step. `SharedArrayBuffer`, WASM threads, and OffscreenCanvas are
optional measured optimizations, not baseline dependencies.

The existing native host crate is not compiled wholesale. Phase 12B.5 extracts
or feature-gates a browser-safe session/evidence layer containing the accepted
world, flight package, `FullMissionSession`, role filtering, action handling,
and in-memory KSB11 encoder. Filesystem, terminal, process, native-thread,
report, target, campaign, and optimizer facilities remain in native host code.
A thin `wasm-bindgen` adapter owns browser messages and storage requests; the
native C ABI remains frozen for desktop consumers.

Local WebAssembly becomes accepted only when an identical definition and action
transcript reproduce the native catalog identity, release order, event order,
physical/navigation/command checksums, outcome axes, terminal release, exact
2,911,464-byte KSB11 archive, and accepted SHA-256. Renderer backend, refresh
rate, polling, window size, and presentation quality may not alter those bytes.

## Browser lifecycle and failure policy

A service worker caches and updates the installable application; it never owns
a running mission. The simulation worker owns the session and advances only by
accepted releases. Browser throttling may delay wall-clock completion but cannot
change simulated time or results.

Page close, process termination, operating-system suspension, worker failure,
and memory pressure cannot be mistaken for continuous execution or a completed
archive. Initially they leave an explicitly incomplete session. Deterministic
resume may later reconstruct from a validated checkpoint and ordered action
transcript. Background realtime authority is unsupported until lifecycle tests
prove an honest policy.

The WebAssembly target initially uses panic-abort. A panic terminates the
mission worker, yields no completed archive, and is reported by the shell as a
contained session failure. The UI remains alive and may offer a restart or
validated replay; it may not invent recovery state.

Remote sessions follow a different disconnect policy: the native authority
continues unless an accepted operator action explicitly pauses it. Reconnection
uses bounded sequence identities and role binding rather than silently
rewinding or skipping data.

## Duet reference tier

The 8 GB Lenovo Chromebook Duet 11 is the first ChromeOS acceptance device.
The web product must provide:

- complete responsive 2-D Mission Control;
- remote operation of a Crostini or other native authority;
- local replay and debrief;
- one complete local-WASM GNSS-loss mission after parity is established;
- a modest globe, trajectory paths, vehicle marker, and exact event display;
- WebGPU, forced-WebGL2, and 2-D-only test modes; and
- measured memory, thermal, storage, startup, and sustained-frame behavior.

The initial 3-D tier targets 30 frames per second with bounded dynamic internal
resolution, compressed textures, restrained post-processing and shadows,
limited Earth detail, and explicit LOD. Rendering may degrade without changing
the world, flight package, actions, or evidence. Large campaigns,
optimization, authoring compilation, production-scale terrain, and high-end
Unreal-equivalent effects remain host-oriented.

## Phase ownership

- Phase 12B.5 extracts the browser-safe session boundary, freezes the web
  protocol, establishes the PWA shell, proves remote/replay operation, and runs
  the first native/WASM exactness and bounded local-world feasibility gates.
- Phase 12C builds the complete renderer-neutral global display model and its
  Babylon and Unreal implementations.
- Phase 12C.5 productizes the polished PWA, accepts complete local-WASM
  operation, and qualifies portable browser/mobile workflows.
- Phase 12E owns production assets, quality tiers, distribution, and sustained
  performance evidence.

## References

- Babylon.js engine capabilities: <https://www.babylonjs.com/specifications/>
- Babylon.js source and Apache-2.0 license: <https://github.com/BabylonJS/Babylon.js/>
- Babylon.js WebGPU support: <https://doc.babylonjs.com/setup/support/webGPU/>
- Babylon.js large-world engine options: <https://doc.babylonjs.com/typedoc/interfaces/BABYLON.EngineOptions>
- Babylon.js geospatial features: <https://doc.babylonjs.com/features/featuresDeepDive/geospatial/>
- Babylon.js KTX2 compression: <https://doc.babylonjs.com/features/featuresDeepDive/materials/using/ktx2Compression>
- Babylon.js ES-module scaffolding: <https://doc.babylonjs.com/setup/createBabylonJS/>
- Rust browser WebAssembly target: <https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html>
- Browser Web Workers: <https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers>
- Chrome WebGPU diagnostics: <https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips>
