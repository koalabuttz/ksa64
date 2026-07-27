# Phase 12B.5 handoff — cross-platform runtime and presentation foundation

Status: ready to plan. Phase 12B is complete and accepted.

Date: 2026-07-27

## Why this phase exists

Phase 12A and Phase 12B deliberately qualified KSA64's first graphical product
on one pinned native-Windows Unreal Engine 5.8 workstation. That was a useful
feasibility and acceptance boundary, not a permanent restriction on the
product.

Phase 12B.5 moves portability ahead of the global viewer and Mission Foundry so
Windows DLL, D3D12, filesystem, packaging, and process assumptions do not
spread into every later presentation. It adds no physics, flight software,
mission outcome, optimizer, canonical format, or alternate simulator.

The accepted Phase 12B evidence, ABI-v1 layouts, Win64 binaries, hashes, and
2,911,464-byte KSB11 remain frozen.

## Priority platforms

The portable authority lane targets:

- Windows x86-64;
- Linux x86-64;
- Linux ARM64;
- macOS ARM64;
- PlayStation Vita through Rust's VitaSDK-backed Tier-3 target; and
- the existing rust-mos C64 targets through their already accepted boundaries.

The full desktop presentation lane targets:

- Windows/D3D12;
- Linux/Vulkan; and
- macOS/Metal.

Linux ARM64 Unreal remains a measured feasibility lane rather than a universal
acceptance blocker. Native Rust world, flight, evidence, CLI, and TUI support
is required there regardless of Unreal feasibility.

## Shared presentation-session boundary

All graphical and constrained clients consume one typed, versioned,
transport-neutral presentation-session contract above `Ksa64Application` and
`LiveMissionSession`.

The contract may expose:

- role-filtered live snapshots;
- labelled planned, onboard-estimated, ground-estimated, and permitted truth
  paths;
- exact events, transitions, procedures, and action receipts;
- lifecycle, pacing, queue, and integrity state;
- high-level operator proposals through the existing
  stage-validate-commit boundary; and
- bounded, role-appropriate replay products.

It may not expose private truth to an unauthorized role, accept direct
effector commands, parse or reinterpret KSB11 in a presentation client, invent
events from rendering, or run a second mission loop.

In-process dynamic libraries, a sidecar, browser connections, mobile clients,
the Vita, and future hardware links are placements over this same authority
boundary. Placement, renderer, polling rate, interpolation, and client
disconnection remain outside mission identity.

Loopback is the default network exposure. Any LAN listener requires a reviewed
pairing, authentication, role-binding, origin, and transport-security policy.
A disconnected client cannot silently pause or stop a remote authoritative
mission.

## Lenovo Chromebook Duet 11 reference lane

The reference Chromebook is the 8 GB Lenovo Chromebook Duet 11 with an ARM64
MediaTek Kompanio 838.

Phase 12B.5 should prove:

1. the native ARM64 Linux KSA64 application in Debian Crostini;
2. world, flight-package, CLI/TUI, evidence, and bounded campaign behavior;
3. byte-identical accepted session evidence against the native reference;
4. measured mission throughput, memory, storage, and worker-count limits; and
5. a local hybrid topology in which Crostini owns the Rust mission while the
   ChromeOS browser presents role-filtered operations.

Crostini-hosted Unreal is experimental only. It cannot block the required ARM64
Rust package or browser lane.

## Web priority lane

The web product is not an Unreal HTML export. Its accepted stack is frozen in
[WEB_PLATFORM.md](WEB_PLATFORM.md): React and TypeScript for the accessible
Mission Control/PWA shell, Vite for builds, an explicit audited PWA manifest and
service-worker layer, Babylon.js for 3-D, and Rust WebAssembly in a dedicated Web Worker for optional local mission
authority.

Babylon is selected for its maintained WebGPU and WebGL paths, explicit
large-world rendering, geospatial support, glTF pipeline, compressed textures,
LOD, and browser tooling. The application uses tree-shakeable
`@babylonjs/*` modules and pins exact versions. It explicitly attempts WebGPU,
falls back to a separately tested WebGL2 engine, and retains a complete 2-D-only
mode. Babylon physics is disabled: Rust remains the only world, frame, time,
event, action, role, and evidence authority.

Mission Control uses semantic DOM/CSS/SVG/Canvas. Babylon owns only its 3-D
canvas and scene-local labels. Rust supplies role-filtered, camera-relative
display samples and exact event metadata; Babylon may interpolate those samples
for presentation but may never feed a result back to the mission.

Supported operating modes are introduced in this order:

1. live remote client connected to a native or Crostini Rust host;
2. local role-filtered replay and debrief;
3. browser-safe Rust session extraction plus native/WASM exactness fixtures;
4. one complete local-WASM mission in an authoritative Web Worker;
5. complete Babylon global viewing through the Phase 12C display model; and
6. optional Pixel Streaming from a desktop Unreal host.

The fully local topology contains the real fixed-point world, truth-blind flight
package, operations engine, role filtering, and in-memory KSB11 construction:

```text
Browser main thread
  React Mission Control + Babylon.js 3-D
                  |
       transferable typed messages
                  |
Dedicated Web Worker
  Rust/WASM FullMissionSession
  world + flight + operations + evidence
                  |
       IndexedDB / explicit download
```

The current desktop `host` and `viewer-bridge` crates are not compiled
wholesale. Native filesystem, terminal, process, thread, report, campaign,
optimizer, and C-ABI code stays native; a browser-safe session/evidence layer
and thin `wasm-bindgen` adapter are extracted without changing the accepted
simulator.

Local WebAssembly is accepted only after the same definition and action
transcript produce the native catalog identity, release/event order, checksum
chains, outcome axes, terminal release, exact 2,911,464-byte KSB11, and SHA-256.
A service worker may cache the PWA but never owns the running world. Browser
close, suspension, or worker failure leaves an explicitly incomplete session;
it cannot masquerade as uninterrupted execution.

On the Duet, the preferred hybrid topology remains:

```text
Debian Crostini
  Rust world + flight + LiveMissionSession
                  |
       role-filtered local session link
                  |
ChromeOS Chrome
  PWA Mission Control + Babylon WebGPU/WebGL2 3-D
```

After exact WASM parity, the same installed PWA may instead run completely
locally with no Crostini or Android dependency. The reference tier targets full
2-D operations and a restrained 30-fps globe/trajectory view with dynamic
resolution, LOD, compressed textures, and reduced effects. Actual WebGPU,
forced-WebGL2, and 2-D-only behavior are all acceptance evidence.

## PlayStation Vita and SDL2 priority lane

The Vita receives a purpose-built SDL2 client rather than Unreal. Rust’s
`armv7-sony-vita-newlibeabihf` target uses VitaSDK, supports `no_std` plus
`alloc` and partial `std`, and produces statically linked applications;
dynamic linking is unavailable.

Its first accepted role is:

- compact Mission Control;
- telemetry and procedure pages;
- trajectory and event replay;
- high-level operator actions;
- clear connection, staleness, and authority state; and
- deterministic comparison against host-side presentation fixtures.

The reference display target is 960x544 with bounded memory and retained
history. The first live placement keeps the host world and flight software
authoritative. Follow-on gates measure:

1. host world plus Vita flight computer;
2. selected Vita world plus host flight computer; and
3. an all-in-one portable mission with SDL2 Mission Control.

Each placement must reuse the portable Rust implementation, match the accepted
sensor/command/checksum chains, and separate flight-computer release timing
from world throughput. Vita3K is useful automation evidence, but physical
hardware owns input and timing claims.

SDL2 is a client technology, not the common renderer. Unreal, web, and SDL2
share Rust-owned view contracts and fixtures while retaining platform-specific
layouts and drawing code.

## Android and iOS direction

Android ARM64 is the first native mobile package, using the 8 GB Duet as a
reference ChromeOS/Android device. iOS/iPadOS follows after a pinned
Mac/Xcode/signing lane exists.

Initial mobile responsibilities are:

- role-filtered live Mission Control;
- touch-friendly procedures and high-level actions;
- replay, trajectory inspection, and debrief; and
- reduced-quality 3-D where measured hardware permits.

The initial world and flight computer remain remote. A local native flight
package or bounded world may be enabled separately after proving exact
evidence, foreground/background lifecycle, thermal behavior, memory bounds,
and safe suspension. A mobile operating system may never silently suspend an
authoritative session and later pretend that it ran continuously.

## Phase 12B.5 planning expectations

The implementation plan should include:

1. platform-neutral bridge filenames, loading, hashing, staging, and manifests;
2. native Rust build/test matrices for Windows x64, Linux x64, Linux ARM64,
   macOS ARM64, and bounded Vita compile/vector probes;
3. exact cross-platform catalog, lifecycle, action, and KSB11 fixtures;
4. a typed presentation-session sidecar or broker with explicit security and
   disconnect semantics;
5. Linux and macOS 2-D operations-desk packaging where build hardware exists;
6. the Duet ARM64 package and local Crostini-to-Chrome acceptance slice;
7. minimal web/PWA and Vita/SDL2 feasibility clients;
8. passive presentation and renderer invariance;
9. deterministic replay and corruption/truncation rejection; and
10. preservation of every Phase 0–12B artifact.

Phase 12B.5 establishes portability and client contracts. Phase 12C owns the
complete renderer-neutral global display model and full Unreal/web 3-D
viewers. Phase 12C.5 owns polished web, Vita, Android, and iOS operations
clients. Phase 12D remains desktop Mission Foundry authoring, and Phase 12E
owns production assets, quality tiers, distribution, and performance.
