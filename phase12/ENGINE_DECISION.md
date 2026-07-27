# Phase 12 engine and authority decision

Date: 2026-07-26

Status: accepted for Phase 12A.

## Decision

Use the current Unreal Engine 5.8 Epic Games Launcher build on native Windows
11 for Phase 12. Pin the exact installed engine identity and supported Windows
compiler/SDK combination before accepting any build. Start with a blank C++
desktop project under `foundry/Ksa64MissionFoundry`.

Connect Unreal in process to a versioned Rust `cdylib` through a narrow C ABI.
The bridge calls `Ksa64Application` directly, owns one `LiveMissionSession` on
a dedicated Rust worker per handle, and publishes immutable role-filtered
snapshots and events. Keep an out-of-process sidecar as the documented fallback
if panic containment, DLL replacement, or editor survival cannot be made
adequate; do not implement the sidecar in 12A.

Use Epic's experimental Unreal MCP only for supervised, bounded editor
development. Keep it loopback-only and optional. MCP, Python, the editor, and
Codex are not runtime dependencies of the packaged product.

## Why Unreal 5.8

- It is the current UE5 release selected for this spike and is available as a
  Launcher build, avoiding a source-engine maintenance burden.
- Epic's 5.8 documentation recommends Windows 11, Visual Studio 2026 for
  general development, DirectX 12, and Shader Model 6 for the relevant modern
  rendering feature set.
- UE 5.8 includes Epic's official experimental Unreal MCP integration, which
  can inspect and make bounded editor changes without treating binary
  `.uasset` contents as guessable text.
- Unreal supplies a mature packaged Windows runtime, C++/automation surface,
  editor, asset pipeline, large-world rendering, and later presentation tools.
  None of those capabilities needs to become simulation authority.

The engine choice is a feasibility decision, not an irreversible claim that
all Phase 12 acceptance gates will pass. Record the full build identity; an
engine upgrade requires a clean branch, full rebuild, automation, cook,
package, and asset-diff review.

## Authority separation

Rust remains sole authority for:

- product/catalog identity and capability discovery;
- live-session lifecycle and release ordering;
- physical state, avionics, role filtering, procedures, predictions, and
  operator action validation;
- canonical telemetry, events, checksums, and KSB11 construction.

Unreal owns:

- presentation clocks, cameras, layout, rendering, interpolation, and
  role-specific views;
- later editor workflows that invoke accepted Rust compilers and services;
- noncanonical diagnostics, screenshots, and packaged performance evidence.

Unreal must not run substitute Chaos flight physics, derive events from visual
state, expose SIM Director truth to another role, or make display cadence part
of evidence identity.

## Deliberate changes to the supplied Unreal guide

The external `KSA64_Unreal_Codex_Windows_Guide.md` is useful research and setup
input, but it is not the project contract. Phase 12 changes its rollout in the
following ways:

1. The first accepted bridge target is a **live** GNSS-loss operations session,
   not a replay presented as live. This follows the later accepted Phase 11.5
   `LiveMissionSession` boundary.
2. GNSS-loss and complete global replay are separate evidence slices.
   GNSS-loss proves the bidirectional application boundary, actions, roles, and
   KSB11 finalization in 12B. The complete Phase 10 mission proves
   ENU/ECEF/GCRF presentation, large-world continuity, entry, and recovery in
   12C.
3. Phase 12A proves state and containment with no scene, Earth, NASA asset,
   interpolation, or visual-performance requirement. Visual ambition begins
   only after the bridge is accepted.
4. The DLL contract is hardened beyond a representative C ABI: opaque handles,
   explicit ABI and structure sizes, fixed-width fields, validity masks,
   bounded queues, explicit buffer ownership, commit-qualified filenames,
   hash verification, unwind containment, and malformed-input tests are
   required.
5. MCP and Python are developer tooling only. Shipping behavior belongs in
   runtime C++/Slate/UMG modules and must work in a packaged build.
6. Launcher hotfix identity, LFS capacity, Unreal licensing/redistribution, and
   third-party asset rights are explicit acceptance records rather than
   assumptions.

## Failure and fallback decision

Start in process because it is the smallest integration and retains a simple
typed boundary. Fail 12A or switch to a sidecar design before 12B if any of
these remain true after focused remediation:

- a Rust failure can unwind across the ABI or terminate the harness/editor;
- normal bridge iteration requires unsafe replacement of a loaded DLL;
- malformed input can corrupt session ownership or subsequent calls;
- Unreal must block on simulation work to use the API;
- role-restricted data crosses the bridge and is merely hidden in the UI.

At 32 Hz, process isolation is expected to be affordable, but a sidecar needs a
separate protocol, lifecycle, and acceptance decision.

## Sources

- Epic, [Unreal Engine 5.8 is now available](https://www.unrealengine.com/news/unreal-engine-5-8-is-now-available).
- Epic, [Unreal Engine 5.8 release notes](https://dev.epicgames.com/documentation/unreal-engine/unreal-engine-5-8-release-notes).
- Epic, [hardware and software specifications](https://dev.epicgames.com/documentation/unreal-engine/hardware-and-software-specifications-for-unreal-engine).
- Epic, [Unreal MCP in Unreal Editor](https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor).