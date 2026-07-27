# Phase 12A: Unreal toolchain and live-bridge feasibility

Status: accepted implementation contract; completion evidence pending.

## Purpose

Prove that a pinned Unreal Engine 5.8 Launcher build on native Windows can
consume the accepted KSA64 application and live GNSS-loss session safely,
deterministically, and without creating another simulator or evidence format.

Baseline commit `8208d53` must exist on `origin/main` before the Phase 12
short-path checkout is created. All Phase 0–11.5 artifacts and the 13-entry
accepted catalog remain frozen.

## Locked implementation

### 1. Publish and freeze

- Verify local and `origin/main` both resolve to `8208d53`; never force-push.
- Run the Phase 11.5 frozen completion audit before bridge changes.
- Record the audit result and catalog bytes/hash in the 12A completion record.

### 2. Native Windows toolchain

- Use native Windows 11, not WSL, for Unreal, MSVC, the bridge, and packaging.
- Install the current UE 5.8 Launcher patch under `E:` and place its
  Derived Data Cache on `E:`.
- Use Visual Studio 2026 with Game Development with C++, Desktop C++, Unreal
  integration, profiling tools, AddressSanitizer, the Epic-supported MSVC
  toolset, and Windows SDK.
- Update Git for Windows and Git LFS to current stable releases.
- Populate the noncanonical lock manifest described in `TOOLCHAIN.md`.
- Epic authentication, EULA acceptance, and installer prompts are human steps;
  automation verifies the resulting installations and records no credentials.

### 3. Checkout and source policy

- Create `C:\dev\KSA64` from the verified remote and use it as the sole Phase
  12 writer while Unreal is open.
- Enable Git LFS before adding Unreal binary content and enforce `ASSETS.md`.
- Ignore Unreal/IDE/cook/cache/performance output; track project configuration,
  source, scripts, plugins, and intentional content.
- Add Foundry-specific agent instructions preserving Rust authority, role
  filtering, MCP limits, asset provenance, and verification.

### 4. Empty Unreal project

- Create `foundry/Ksa64MissionFoundry` as a Blank C++ desktop project with
  Starter Content and hardware ray tracing disabled.
- Use DX12 and SM6. Lumen, Nanite, Niagara, CommonUI, and any quality tier are
  not 12A acceptance requirements.
- Enable only Unreal MCP and its Toolset Registry/AllToolsets dependencies plus
  Python editor scripting for the development experiment.
- Accept the clean Editor target before bridge integration.

### 5. Rust bridge

- Add a `viewer-bridge` workspace crate producing native tests and an MSVC
  `cdylib`; it depends directly on the accepted application facade.
- Use a dedicated unwind-enabled viewer profile. Catch panic at every exported
  entrypoint and worker boundary; never permit an unwind to cross C.
- Implement the ownership, layout, role, threading, and diagnostics rules in
  `BRIDGE.md`.
- Expose ABI/build information, deterministic catalog JSON, live-session
  lifecycle/pacing/stepping, truth-blind snapshots/events, existing atomic
  action payloads, and exact completed KSB11 retrieval.
- Use a commit-qualified DLL filename and an adjacent SHA-256/ABI manifest.
  UnrealBuildTool stages a prebuilt artifact and never launches Cargo.

### 6. Independent native harness

- Load the same header and dynamic function table used by Unreal.
- Test ABI/size/null rejection, catalog identity, lifecycle, nonblocking
  polling, action submission, completion, buffer ownership, diagnostics, and
  clean teardown.
- Drive the complete guided GNSS-loss session through the C ABI and compare its
  KSB11 bytes with the accepted Rust path.
- Run a test-only contained panic probe; the process must survive and receive a
  typed bridge error.

### 7. Minimal Unreal runtime plugin

- Load and hash-validate the bridge, enumerate the product catalog, open the
  guided GNSS-loss session, advance one release, and read one role-filtered
  snapshot.
- Add automation for ABI/hash mismatch, catalog identity, lifecycle, one-step
  advancement, role-data absence, and clean shutdown.
- Do not add simulation, coordinates, interpolation, scene actors, or
  operations UI.
- Stage the DLL into a packaged Development build and prove startup without
  the editor, MCP, or Python.

### 8. MCP feasibility

- Keep the server on loopback; do not expose the unauthenticated endpoint.
- Prove one read-only actor/toolset inspection.
- Make, verify, and remove one disposable editor mutation; confirm no
  unintended tracked asset remains.
- Record engine/plugin identity and limitations. MCP calls are serialized and
  are never issued concurrently.
- All normal build, automation, cook, package, and runtime gates pass with MCP
  disabled.

### 9. Completion audit

- Run Rust formatting, Clippy with warnings denied, full native tests, every
  frozen Phase 0–11.5 audit, bridge layout/ownership/panic tests, the C++
  harness, Unreal automation, clean Editor build, cook, and packaged smoke.
- Verify no generated Unreal directory or non-LFS governed binary is tracked.
- Record build times, bridge size/hash, catalog hash/count, queue bounds,
  polling latency, packaged startup, disk use, and all tool versions.
- Produce a Phase 12A completion record and Phase 12B handoff only after every
  required gate passes.

## Acceptance

Phase 12A is complete only when:

- every Phase 0–11.5 artifact remains unchanged and the accepted catalog still
  has 13 experiences with its frozen identity;
- a clean short-path checkout builds the empty UE 5.8 Editor target with the
  recorded toolchain;
- the harness and Unreal plugin reach `Ksa64Application` only through the
  versioned bridge;
- a complete guided GNSS-loss ABI run yields byte-identical KSB11 evidence;
- invalid ABI, size, hash, identifiers, payloads, and lifecycle operations fail
  before session state changes;
- an intentional panic is contained without crossing the ABI or killing the
  harness;
- guided-operator data contains no SIM Director truth;
- polling, presentation pacing, and queue pressure cannot change release order,
  actions, evidence, or final identity;
- the packaged smoke target loads without Unreal Editor, MCP, or Python; and
- MCP proves one bounded disposable editor change while remaining optional.

## Explicitly deferred

Phase 12A adds no renderer, scene graph, coordinate conversion, interpolation,
authoring UI, NASA asset, new K-format, physics change, or alternate simulator.
Phase 12B owns live operations presentation; 12C owns the complete global
viewer; 12D owns authoring/compiler parity; 12E owns production visual assets
and performance tiers.