# Phase 12B.5 implementation checkpoint

Status: **software implementation, local acceptance, and hosted portable-runtime qualification complete; full phase acceptance pending device and emulator gates**.

Entry commit: `b9f2c79a2603a71cd51c7329fcb0ab763f2f2615`

Date: 2026-07-27

## Outcome

Phase 12B.5 has removed the desktop-heavy host and Win64 bridge from KSA64's portable live-session boundary without changing accepted mission behavior. The portable Rust session, role-filtered presentation protocol, native bridge, loopback and paired transports, compact web product, real browser WebAssembly authority, and bounded Vita client are implemented.

This record does **not** mark the phase fully accepted. The hosted four-platform matrix and WASM worker exactness under the Node harness now pass, but the frozen completion contract also requires a physical Lenovo Duet 11 run, Vita3K execution, and physical Vita acceptance. Those gates cannot be replaced by desktop or hosted tests and remain explicitly pending below.

## Delivered foundation

- `ksa64-session` now owns the accepted exact live mission, procedures, predictions, action transcript, role policy, in-memory KSB11 finalization, and strict KSB11-to-KPS1 replay. Filesystem, reports, wall-clock measurement, campaigns, optimization, processes, sockets, and native worker pools remain in `ksa64-host`; legacy host paths are re-exported.
- `ksa64-presentation` is `no_std + alloc` and owns role-filtered DTOs, the frozen 48-byte KPS1 envelope, typed codecs, cursors, retention gaps, action intents, lifecycle, staleness, and sealed-evidence metadata.
- ABI v1 remains unchanged. The public C header, loader, C/C++ harness, artifact-manifest v2, and Unreal staging support `.dll`, `.so`, and `.dylib` lanes while retaining the frozen accepted Win64 artifact and manifest-v1 reader.
- `ksa64-session-broker` provides one authority worker per session, a loopback-only PWA/WebSocket service, exact-Origin admission, a 256-bit subprotocol token, bounded queues/rates, reconnect cursors, and authority continuation through presentation disconnect.
- Explicit native/Vita LAN mode uses Noise XX with a locally confirmed comparison code and immutable role binding, then Noise IK for authenticated reconnect. It has no discovery, wildcard default, UPnP, NAT traversal, cloud relay, or Internet listener. Unix secret files are created owner-only.
- The React/TypeScript/Vite PWA supplies the compact operations desk, high-level Review/Stage/Commit actions, procedures, trajectory plots, timeline, staleness, multidimensional mission dispositions, accessibility controls, offline shell, WebGPU probe, WebGL2 fallback, and complete 2-D fallback.
- The browser-local lane runs the real KSA64 world, flight package, mission operations, role filter, and KSB11 encoder inside one Rust WebAssembly worker. JavaScript never becomes simulation authority.
- The Vita lane contains a bounded 960x544 SDL2 client, offline KPS1 replay, high-level action controls, resynchronization state, shared Noise pairing/transport, a real VitaSDK socket path, and a reproducible VPK build. Vita does not consume the 64-bit C ABI.
- Native engineering packaging produces target-labelled ZIP/tar archives with generated manifest-v2 structure sizes, SHA-256 sums, and an explicit qualified or unqualified-local status.

## Exact local evidence

The native portable session, strict Rust replay, C ABI harness, and real exported WebAssembly worker all reproduced:

| Field | Accepted value |
|---|---:|
| Terminal release | 21,591 |
| Accepted actions | 4 |
| Simulated duration | 674.71875 s |
| KSB11 bytes | 2,911,464 |
| KSB11 SHA-256 | `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4` |
| Overall disposition | Degraded Success |
| Disposition axes | Primary Achieved / Nominal Vehicle / Completed Procedure / Timely Reference Operator / Degraded Operational Avionics / Complete Evidence |

Local validation passed:

- workspace formatting, warnings-denied Clippy, and the complete native workspace suite;
- the frozen Phase 12B core/bridge audit and complete inactive Phase 10 mission;
- exact native full-session and strict KSB11 replay gates;
- opaque Rust/WASM replay of 69,549 consecutive KPS1 frames, including 21,596 truth-free Observer snapshots and mandatory final evidence metadata;
- no-default portable-session, presentation, and broker builds, including the WASM target;
- independent C KPS1 vectors, C++ misuse/panic/lifecycle harness, and full C-ABI mission;
- 30 all-feature broker library/binary tests covering malformed clients, bounds, disconnect continuation, cursor gaps, Noise XX/IK, revocation, tampering, and reconnect;
- 38 web tests and a self-contained production build with 108 precached routes;
- a real in-app-browser mission in WebGPU, forced WebGL2, and 2-D-only modes, followed by an offline cached-shell run;
- seven all-feature Vita host tests and warnings-denied Clippy; and
- a noncanonical 1,150,950-byte VPK with SHA-256 `e9093c1c791480fec0f26ce5cada39122cf62822549957c0506308235f69cf90`.

Polling cadence, renderer backend, replay polling, and presentation state did not change authority or final evidence in the exercised lanes. Guided Operator surfaces contained no SIM Director truth.

## Frozen bridge metadata audit

The accepted Phase 12B manifest contains a historical header-digest value that does not match any recoverable source revision. The frozen manifest, DLL, accepted header, ABI, and mission evidence remain unchanged. The verifier admits the actual accepted header only through an exact tuple binding the manifest filename and SHA, full source commit, frozen DLL and header hashes, ABI/build identity, and catalog hash; every other mismatch still fails closed. See [FROZEN_BRIDGE_HEADER_AUDIT.md](FROZEN_BRIDGE_HEADER_AUDIT.md).

## Hosted portable-runtime qualification evidence

Commit `aae737c03b8d23e171f77d0b0e95b9dbff22746e` passed both hosted workflows:

- Exact cross-platform acceptance run [30326378656](https://github.com/koalabuttz/ksa64/actions/runs/30326378656): Windows x64, Linux x64, Linux ARM64, and macOS ARM64 all reproduced the accepted mission; the WASM worker ABI reproduced it under the Node harness and preserved the frozen Phase 10 identity.
- Fast portability run [30326378684](https://github.com/koalabuttz/ksa64/actions/runs/30326378684): all seven jobs passed formatting, warnings-denied Clippy, workspace tests, broker security, bridge panic/misuse and full-mission harnesses, web/WASM, Vita host fixtures, qualified packaging, and upload.
- GitHub Actions artifact digests for the uploaded, source-bound engineering archives are: Windows x64 `sha256:16ad1783171503454d4f60416ec160f4ac76e0539cca03ad724f724bc32e427e`, Linux x64 `sha256:0cc89fb636eda366bb524621076c87e38cf5525b85d73d680e95d0a3ec08634a`, Linux ARM64 `sha256:27f09c517b02df1d30546e146914af1a6ec810b51f80b098f556e32cd076449d`, and macOS ARM64 `sha256:4272a8f78c8131652135d7e3c21eb00cdcedad626cbc6391faa839c464d93902`.

## Qualification matrix

| Gate | Status |
|---|---|
| Local Windows x64 native/session/bridge/PWA/WASM | Pass |
| GitHub-hosted Windows x64 | Pass — exact, fast, and qualified archive at `aae737c` |
| GitHub-hosted Linux x64 | Pass — exact, fast, and qualified archive at `aae737c` |
| GitHub-hosted Linux ARM64 | Pass — exact, fast, and qualified archive at `aae737c` |
| GitHub-hosted macOS ARM64 | Pass — exact, fast, and qualified archive at `aae737c` |
| Physical 8 GB Lenovo Duet 11: Crostini native | Pending physical run |
| Physical Duet: hybrid PWA and local WASM | Pending physical run |
| Physical Duet: WebGPU/WebGL2/2-D/offline/suspension | Pending physical run |
| Vita host fixtures and VPK construction | Pass |
| Vita3K smoke/input | Pending emulator environment |
| Physical Vita layout/input/pairing/reconnect/suspend/memory/30 fps | Pending physical run |
| Unreal Win64 frozen behavior | Preserved |
| Unreal Linux x64/macOS ARM64 package evidence | Conditional; qualified engine hosts unavailable |
| Unreal Linux ARM64 | Nonblocking feasibility only |

The physical Duet checklist is [DUET_ACCEPTANCE.md](DUET_ACCEPTANCE.md); Vita build status and the still-open Vita3K/physical-device gates are recorded in [the Vita build-evidence record](../vita/BUILD_EVIDENCE.md). A worker crash, browser suspension, interrupted replay, or incomplete device run must remain incomplete and can never be promoted to accepted evidence.

## Limitations and handoff

- KSB11 replay is strict and Rust-owned; the initial accepted replay profile is the guided GNSS-loss mission, not an arbitrary future-session decoder.
- The Babylon scene is intentionally a backend feasibility probe. Earth-scale coordinates, vehicle pose, frame transitions, entry, and recovery remain Phase 12C.
- The PWA and Vita presentation are engineering-quality feasibility products. Mobile packaging, browser-LAN certificate UX, polished portable clients, and Vita authority placements remain Phase 12C.5.
- Signing, notarization, installers, app stores, accounts, cloud relay, Internet service, and multi-controller arbitration remain deferred.
- C64 formats, programs, VICE policies, long-run rules, and authority boundaries are unchanged.

Phase 12C may use this software boundary for continued development, but Phase 12B.5 is not recorded as fully accepted until the pending physical Duet, Vita3K, and physical Vita gates have their source-bound evidence.
