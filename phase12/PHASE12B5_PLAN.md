# Phase 12B.5 implementation contract

Status: implementation and hosted portable-runtime qualification complete; full acceptance pending device and emulator qualification.

Entry commit: `b9f2c79a2603a71cd51c7329fcb0ab763f2f2615`

Date: 2026-07-27

## Objective

Phase 12B.5 removes Windows and Unreal assumptions from KSA64's accepted live
session boundary. It establishes one portable, role-filtered foundation for
native desktop applications, the browser, the Lenovo Duet 11, PlayStation
Vita, and later mobile clients.

The phase does not add or alter physics, flight software, optimization,
mission outcomes, product-catalog identities, canonical formats, C64 programs,
or accepted Phase 12B evidence. The accepted ABI-v1 Win64 layouts and archived
binary remain frozen.

## Frozen entry evidence

The implementation must preserve:

- the 13-entry product catalog and SHA-256
  `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`;
- the accepted four-action GNSS-loss completion at release 21,591 and
  674.71875 simulated seconds;
- the exact 2,911,464-byte KSB11 and SHA-256
  `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`;
- all disposition axes, event/action order, and physical, navigation, command,
  and status checksum chains;
- the Phase 12B ABI-v1 function and structure layouts, bridge build identity
  `0x120B0001`, and archived accepted Win64 DLL; and
- every Phase 0-12B regression artifact and existing C64/VICE policy.

The source commit may advance and produce new commit-qualified bridge hashes.
It may not rewrite any archived acceptance record.

## Portable session and presentation crates

Add two workspace crates without rewriting accepted mission behavior:

### `ksa64-session`

- Own `FullMissionSession`, lifecycle, exact release advancement, procedures,
  predictions, action transcripts, role policy, and in-memory KSB11
  finalization.
- Use only `core`, `alloc`, and browser-compatible `std`.
- Exclude filesystem, terminal, sockets, processes, wall-clock authority,
  native threads, reports, campaigns, optimization, and target launching.
- Preserve operation order, fixture lifetime, action timing, and exact output.
- Re-export moved APIs from their existing `ksa64_host` paths for source
  compatibility.

### `ksa64-presentation`

- Build as `no_std + alloc`.
- Own role-filtered operational DTOs and strict codecs for snapshots,
  procedures, dispositions, action proposals and receipts, timeline events,
  release samples, paths, queue state, lifecycle, integrity, and staleness.
- Expose a transport-neutral `PresentationSession`,
  `PresentationActionIntent`, and independent cursor/batch interfaces.
- Apply role filtering before any C, network, JavaScript, SDL2, or Unreal
  boundary.
- Never expose `FullMissionSnapshot`, private truth, or canonical K-record
  internals to a client.

Filesystem persistence, CLI/TUI, reports, campaigns, optimizers, and platform
launching remain in `ksa64-host`.

## KPS1 presentation protocol

Freeze the noncanonical `ksa64.presentation-session.v1` protocol with this
48-byte little-endian envelope:

```text
magic[4]            = "KPS1"
major               = u16, 1
minor               = u16, 0
header_length       = u16, 48
message_kind        = u16
flags               = u32
session_nonce       = u64
sequence            = u64
correlation_id      = u64
payload_length      = u32
crc32               = u32
```

CRC-32 covers the header excluding the CRC field followed by the payload.
Correlation zero identifies an unsolicited publication.

Ordinary payloads are bounded to 256 KiB. Sealed evidence uses 64 KiB chunks
and a 16 MiB object ceiling. Unknown required flags, nonzero reserved fields,
stale session nonces, invalid sequences, truncation, corruption, and oversized
records fail before state changes.

Snapshots and release samples may coalesce or be dropped as explicitly lossy
presentation history. Events, timeline records, and action receipts may not.
Independent reconnect cursors report `ResyncRequired` when retained history no
longer covers the requested cursor.

Typed messages cover handshake, lifecycle and pacing, snapshots, procedures,
paths, actions, events, transport state, replay, diagnostics, and opaque sealed
evidence metadata. Clients submit only proposal identities such as Review,
Stage, Commit, and Cancel. Rust constructs and validates KUL11/KUA11 through
the accepted stage-validate-commit authority. Direct effector commands remain
impossible.

KSB11 remains opaque to clients. Role-filtered replay is emitted through a
Rust-owned reader.

## Native bridge portability

- Preserve every ABI-v1 symbol, field, structure size, and behavior.
- Restrict Windows conditionals in the public C header to calling-convention
  and export macros.
- Support `.dll`, `.so`, and `.dylib` through one portable loader/harness
  abstraction.
- Add manifest v2 fields for generic library filename, target triple, OS,
  architecture, source commit, profile, SHA-256, catalog identity, ABI/build
  identity, and structure sizes.
- Continue accepting the frozen Win64 manifest-v1 layout.
- Retain independent misuse, panic, lifecycle, queue, and full-mission
  harnesses on every qualified native target.
- Link `ksa64-presentation` statically on Vita; do not force the 64-bit C ABI
  onto that target.

## Broker and paired transport

Add `ksa64-session-broker` with one authority worker per live session.

The PWA service binds loopback by default, admits binary WebSocket messages
only, requires a per-launch 256-bit token in the WebSocket subprotocol, and
enforces an exact Origin allowlist plus bounded connections, rates, queues,
and outstanding commands. Role is immutable and one controlling client is
allowed per session. Browser disconnect never pauses or stops native
authority. Reconnect requires the session nonce and independent stream
cursors.

LAN mode is explicit and binds only a user-selected interface. First pairing
uses Noise `XX_25519_ChaChaPoly_BLAKE2s`, a locally confirmed comparison code
derived from the handshake hash, stored peer public keys, and immutable role
assignment. Returning peers use Noise IK. KPS1 frames are length-prefixed
inside the encrypted transport. Pairing timeout, rate limiting, revocation,
and fail-closed malformed-peer behavior are required. Discovery, UPnP, NAT
traversal, wildcard binding, cloud relay, and Internet listening are excluded.

ChromeOS uses Crostini localhost forwarding for the browser lane. Productized
browser-LAN certificate UX remains Phase 12C.5.

## Web and WebAssembly product

Create an npm-lockfile-pinned React/TypeScript/Vite PWA:

- semantic HTML/CSS with SVG/Canvas engineering plots;
- direct tree-shaken `@babylonjs/*` use without a React renderer wrapper;
- an explicit audited manifest and service worker;
- self-hosted JavaScript, WebAssembly, shaders, and assets;
- one `PresentationTransport` implemented by WebSocket, local Worker, and
  replay adapters; and
- transferable `ArrayBuffer` messages between the main thread and worker.

The compact desk exposes mission/frame/package status, planned/onboard/ground
2-D paths, procedures, Review-Stage-Validate-Commit actions, navigation
residuals, events, connection/staleness, independent disposition axes, and
evidence integrity. It supports keyboard, touch, scaling, reduced motion, high
contrast, and explicit degraded/contingency success.

Babylon proves explicit WebGPU initialization, separately tested WebGL2
fallback, and a complete 2-D-only mode with a small presentation-only scene.
Babylon physics is disabled. Complete Earth, frame, and vehicle 3-D remains
Phase 12C.

Local authority runs the accepted world, flight package, operations engine,
role filtering, and KSB11 encoder in one single-threaded Rust WebAssembly
worker. The main thread never advances physics. Baseline acceptance does not
use WASM threads, `SharedArrayBuffer`, `OffscreenCanvas`, or browser physics.
A panic, tab close, discard, suspension, or worker termination leaves an
explicitly incomplete session and can never fabricate a sealed archive.

## Native, Duet, Vita, and Unreal lanes

Pin Rust 1.93 for native and WebAssembly work while leaving rust-mos and Vita
toolchains explicitly isolated.

Native authority and evidence are qualified on:

- Windows x86-64;
- Linux x86-64;
- Linux ARM64; and
- macOS ARM64.

Engineering archives contain the CLI/session tools, platform bridge where
applicable, manifest, hashes, and provenance. macOS output remains unsigned in
this phase.

The physical 8 GB Lenovo Chromebook Duet 11 must run the ARM64 Crostini
CLI/TUI/session tools, produce the exact accepted KSB11, and record startup,
throughput, peak RSS, archive-write cost, storage, and worker limits. It must
also run Crostini authority with the ChromeOS PWA and the complete local-WASM
mission in WebGPU, forced-WebGL2, and 2-D-only modes. Local WASM need not be
realtime; the presentation must remain responsive and the result exact.

The Vita client uses a pinned nightly Rust target, VitaSDK, `cargo-vita`, and
SDL2. Its 960x544 VPK provides compact status, navigation, procedure,
trajectory, timeline, integrity, high-level actions, offline role-filtered
replay, and paired encrypted live Mission Control. Target 30 fps, bounded
history, and a 64 MiB client working set. Vita3K supplies repeatable smoke and
input evidence; physical hardware owns controls, pairing, network
loss/reconnect, suspend/resume, memory, and timing acceptance. Vita flight or
world authority remains Phase 12C.5.

Generalize Unreal bridge staging and loading to Win64, Linux x86-64, and macOS
ARM64 while preserving the accepted Win64 path. Linux/macOS Editor and package
evidence is required only where a qualified Unreal host exists. Its absence
does not block native Rust or PWA acceptance. Linux ARM64 Unreal remains
nonblocking feasibility work.

## Implementation gates

1. **Freeze and toolchains**
   - Record this contract and the entry commit.
   - Freeze Phase 12B identities, layouts, hashes, and outputs.
   - Add Rust 1.93 toolchain metadata and fast/exact CI workflows.
   - Pass the complete Phase 0-12B compatibility audit.

2. **Portable session extraction**
   - Move behavior without rewriting it and retain host re-exports.
   - Pass exact native session and KSB11 parity.

3. **Presentation contract**
   - Implement DTOs, KPS1, cursors, action intents, and independent
     Rust/TypeScript/C vectors.
   - Pass corruption, overflow, stale-session, cursor-gap, and role-isolation
     gates.

4. **Native bridge portability**
   - Add portable headers, loaders, harnesses, and manifest v2.
   - Pass the frozen Windows regression and Linux/macOS harnesses.

5. **Native platform matrix**
   - Qualify CLI, TUI, session, evidence, and bridge output on all four native
     targets.
   - Pass full cross-platform evidence identity.

6. **Broker and paired transport**
   - Implement loopback browser service, reconnect, Noise pairing, role
     binding, revocation, and malformed-client handling.
   - Pass authority-continuation and security gates.

7. **Compact PWA**
   - Implement remote/replay Mission Control, procedures, actions, plots,
     offline shell, accessibility, and renderer fallback.
   - Pass browser and render-invariance tests.

8. **Local WebAssembly authority**
   - Implement the Worker adapter and complete scripted mission.
   - Pass native/WASM KSB11 equality and failure containment.

9. **Duet acceptance**
   - Run native Crostini, hybrid browser, and local-WASM modes on the physical
     device and record lifecycle/performance evidence.

10. **Vita feasibility client**
    - Build the desktop SDL fixture harness, VPK, Vita3K automation, and
      physical-device paired-LAN evidence.

11. **Unreal portability and completion**
    - Remove source-level Win64 restrictions without changing frozen behavior.
    - Run available platform packaging evidence.
    - Complete audits, provenance, documentation, and the Phase 12C handoff.

Commit after each gate. Do not commit generated build directories, credentials,
peer keys, local performance captures, or unsigned external dependencies.

## Continuous integration policy

Fast pull-request and push checks use Rust 1.93 on Windows x86-64, Linux
x86-64, Linux ARM64, and macOS ARM64. They run formatting, warnings-denied
Clippy, workspace tests, bridge library tests, and—once its lockfile
exists—the web build and tests.

The complete four-action GNSS-loss exactness test is intentionally separate. It
runs on `main`, on manual dispatch, and for phase completion. It must run on
all four native targets and later add WebAssembly when the Worker runner is
implemented. Worker count, runner scheduling, presentation, and artifact
packaging remain outside experiment identity.

Unreal, Duet, Vita, C64, and VICE gates require their qualified hosts or
physical devices and are not emulated by ordinary hosted CI.

## Completion criteria

Phase 12B.5 is complete only when:

- every Phase 0-12B artifact remains unchanged;
- all native targets and WebAssembly reproduce release 21,591, four accepted
  actions, every disposition axis and checksum chain, and the exact accepted
  KSB11 bytes and hash;
- the physical Duet reproduces native ARM64 and local-WASM evidence;
- polling, rendering, reconnect, dropped snapshots, and placement cannot alter
  authority;
- unauthorized roles receive no private truth;
- corrupt, stale, unauthenticated, mismatched, or overflowing traffic fails
  closed;
- native authority continues through presentation disconnect;
- browser failure cannot masquerade as session completion;
- the PWA remains functional in WebGPU, WebGL2, and 2-D-only modes;
- the physical Vita supplies deterministic replay and paired, encrypted,
  role-bound Mission Control; and
- existing C64 boundaries and VICE policies remain unchanged.

Mission success remains multidimensional. Nominal, degraded, contingency,
vehicle, procedure, operator, avionics, and evidence outcomes remain
independently visible.

## Deferred work

- Complete global Earth and trajectory 3-D in Unreal and Babylon: Phase 12C.
- Polished web, Vita, Android, iOS, browser-LAN certificate UX, and Vita
  authority placements: Phase 12C.5.
- Mission Foundry authoring: Phase 12D.
- Production assets, signing, installers, stores, quality tiers, and packaged
  performance: Phase 12E.
- Internet-facing services, accounts, cloud relay, discovery, multi-controller
  arbitration, and public deployment.
