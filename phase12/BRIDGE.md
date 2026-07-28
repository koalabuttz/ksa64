# Phase 12 Rust-to-Unreal bridge contract

The Phase 12A base contract is frozen. Phase 12B extends it additively; no ABI-v1 entry point or layout is reinterpreted.

Status: normative 12A interface and containment contract.

## Phase 12B.5 portability amendment

ABI v1 remains byte-for-byte compatible with the accepted Win64 build. New builds use the same C surface on Windows x64, Linux x64/ARM64, and macOS ARM64; only the export/calling-convention macros and native library suffix vary. The portable harness loads `.dll`, `.so`, or `.dylib` through a platform adapter and validates a manifest-v2 record containing the generic filename, target triple, OS, architecture, source commit, profile, SHA-256, catalog identity, build identity, and all public structure sizes. The archived Win64 manifest-v1 layout remains accepted.

The Win64 DLL and D3D12 statements below describe the frozen 12A/12B acceptance lane; they do not restrict later ABI-v1 libraries to Windows. Vita links `ksa64-presentation` statically and does not consume this 64-bit bridge.

## Boundary

The bridge is a versioned Windows MSVC Rust `cdylib` that calls
`Ksa64Application` directly. Unreal and the native C++ harness use the same C
header and dynamically loaded function table. The CLI and phase executables are
not integration APIs.

The exact exported symbol list is generated/reviewed with the implementation,
but every export uses a `ksa64_viewer_` prefix and the semantic groups below.
No public signature exposes Rust enums, `bool`, `usize`, references, trait
objects, layouts, or canonical evidence internals.

## ABI and layouts

- ABI version starts at `1`; incompatible changes increment it.
- Every public input/output structure begins with `abi_version` and
  `struct_size`, both fixed-width integers.
- Use fixed-width integers and IEEE-754 fields only where the owning accepted
  Rust API already defines their meaning.
- Optional/role-dependent fields use explicit validity masks, never sentinel
  numbers.
- Text/input bytes use caller-owned pointer-plus-length spans valid only for
  the duration of the call.
- Variable output uses Rust-owned immutable buffers with a matching bridge free
  function. A pointer is freed exactly once by the same loaded DLL.
- Opaque handles are never dereferenced, retained, or fabricated by the caller.
- Unknown ABI versions, undersized structures, null required pointers,
  overflowed lengths, invalid UTF-8/JSON, stale handles, and double frees return
  typed errors before changing session state.

## Required capability groups

1. **Library identity:** ABI version, structure sizes, source commit, Rust build
   identity, catalog hash/count, DLL hash-manifest identity, and feature flags.
2. **Catalog:** deterministic current catalog JSON allocated by Rust.
3. **Session:** create the accepted `ksa-g10r.operations` / `gnss-loss`
   experience for one immutable role; query lifecycle; destroy/abort cleanly.
4. **Control:** pause, resume, set passive pacing, enqueue one exact release,
   and enqueue bounded advancement.
5. **Observation:** nonblocking latest-snapshot poll and ordered event drain,
   with explicit empty/unchanged/overflow status.
6. **Actions:** stage, commit, and cancel the existing typed Phase 11 action
   payloads. Submission is serialized and returns accepted/rejected identity.
7. **Evidence:** retrieve the exact completed KSB11 bytes only after successful
   completion.
8. **Diagnostics:** retrieve typed result code and per-handle/library diagnostic
   text without changing simulation state.
9. **Test-only fault:** a nonshipping contained panic probe proving process
   survival and typed failure.

Catalog JSON, snapshots, and diagnostic text are noncanonical application data.
Retrieved KSB11 bytes remain canonical evidence and are not decoded or rebuilt
by Unreal.

## Thread and queue model

Each live handle owns one Rust worker and the entire `LiveMissionSession`.
Callers enqueue commands or poll immutable published data; they never invoke
simulation work on the Unreal game thread.

- Command, snapshot, and event queues have fixed documented bounds.
- Action and lifecycle commands are never silently dropped or reordered.
- A full command queue returns `busy` without accepting the command.
- Snapshot publication may coalesce intermediate presentation snapshots, but
  retained release/event ordering and final evidence may not change.
- Event overflow is a typed fail-closed condition; the caller must not pretend
  it observed a complete stream.
- Destroy requests worker shutdown and joins outside Unreal's hot frame path.
- Poll frequency, rendering cadence, pause display, or queue pressure cannot
  affect accepted release order or final identity.

## Role separation

Role selection is immutable at session creation. Rust constructs the
role-filtered snapshot/event representation before any data crosses the ABI.
The guided-operator process does not receive SIM Director truth fields,
truth-only events, or raw backing objects. Hiding a field in UMG is not
role filtering.

Tests compare the actual bytes/validity masks available to each role and fail
on forbidden data presence.

## Failure containment

- Build the bridge with a dedicated unwind-enabled viewer profile even though
  the existing workspace release profile may abort.
- Wrap every export and worker entry in panic containment.
- Translate recoverable panic, invalid state, malformed input, fallible-allocation
  errors, worker death, and queue failure into stable result codes and diagnostics.
- No exception or Rust panic crosses the C boundary. Process-level allocator OOM,
  native access violations, and explicit aborts are not unwindable guarantees; any
  observed case stops in-process acceptance and triggers the documented sidecar
  decision boundary.
- Copy bounded caller spans during the export and do not retain caller pointers
  after it returns.
- A failed validation call leaves session state unchanged.
- After worker failure, only diagnostics and destruction remain valid.

The DLL filename includes the source commit/build identity. The adjacent
manifest records filename, SHA-256, ABI, structure sizes, target triple,
catalog identity, and build command. The plugin validates it before loading.
UnrealBuildTool stages an already-built DLL and does not invoke Cargo.

If the harness or editor cannot survive contained faults reliably, 12A stops
and the sidecar fallback receives a separate design decision.

## Harness and Unreal acceptance

Both consumers must prove:

- exact identity/layout negotiation;
- null, size, version, hash, UTF-8/JSON, identifier, lifecycle, and stale-handle
  rejection;
- deterministic catalog JSON and frozen 13-entry identity;
- nonblocking stepping/polling and bounded-queue behavior;
- action sequencing and clean shutdown;
- buffer allocation/free ownership including misuse probes;
- guided-role truth absence;
- contained test panic;
- byte-identical complete guided GNSS-loss KSB11 output.

The Unreal plugin additionally proves the prebuilt DLL is staged into and
loaded by a packaged Development build with MCP and Python disabled.

## Accepted Phase 12B operations extension

Phase 12B qualifies the additive API at ABI major 1 and build identity `0x120B0001`. `ksa64_viewer_start_v1` selects the complete GNSS-loss operations session without changing the original `ksa64_viewer_start` fixture. Feature discovery exposes operational, procedure, disposition, action-receipt, prediction-path, timeline, release-sample, transport, and finalization views only when supported.

Polling and draining are passive. High-level actions use Rust-generated proposals and the exact Phase 11 stage/commit/cancel boundary. KSB11 crosses only as an opaque Rust-owned buffer after successful finalization; Unreal never parses its canonical segments.

Shutdown is asynchronous and distinct from evidence finalization. A requested clean stop of a partial session may terminate the worker while finalization remains `InProgress` with no archive. It is not an evidence failure. `Failed` requires an actual worker/finalizer error, and `Completed` requires a Rust-sealed archive.

The accepted build is `ksa64_viewer_bridge-423c116cf586-120b0001.dll` with SHA-256 `da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565`. Both C++ harnesses, 17/17 Unreal operations tests, the standalone full mission, exact KSB11 finalization, and the D3D12 presentation gate pass. See [PHASE12B_COMPLETION.md](PHASE12B_COMPLETION.md) and [PHASE12C_HANDOFF.md](PHASE12C_HANDOFF.md).

The frozen manifest's historical header-digest discrepancy and the exact, fail-closed compatibility treatment are recorded in [FROZEN_BRIDGE_HEADER_AUDIT.md](FROZEN_BRIDGE_HEADER_AUDIT.md). The manifest, DLL, accepted source header, and canonical evidence remain unchanged.


## Accepted Phase 12C optional global-display extension

Phase 12C preserves every ABI-v1 export and structure. New native consumers
discover the separately size-tagged `GlobalDisplayApiV1` function table; an
older bridge may omit it and remain valid. The table provides bounded
definition, exact-sample range, path, replay-index, and nominal-replay calls
over Rust-owned, role-filtered `GlobalDisplayV1` products. Path consumers must
preserve source/model/estimate and continuity identities plus the raw
stale/incomplete/terminal/resynchronization flags; semantic checksums bind
release, time, segment, event, anchor, and XYZ. It does not expose canonical
K-record internals or permit a renderer to own frames, events, mission
outcomes, or actions. Camera and display-frame selections are normalized to one
supported shared view mode before renderer semantics are compared. The shared
Rust path builder applies exact-source cadence to live release epochs, preserves
the frozen planned source's explicit initial point and one-based sparse
sequence, pins semantic replay bookmarks, and excludes routine release ticks.

The packaged Unreal global-viewer plugin reuses the operations plugin's loaded
bridge and creates no second worker or authority. Its evidence binds the clean
source commit, commit-qualified bridge and manifest, packaged executable,
semantic captures, screenshots, and package inventory. Guided Operator data
contains no SIM truth; SIM Director truth starts hidden and stays explicitly
labelled when enabled.

For the UE 5.8 Launcher build, project modules compile with
`RayTracingMode=Inline` solely so custom scene-proxy vtables match the
precompiled Renderer ABI. Runtime ray tracing remains disabled with
`r.RayTracing=False`; no bridge or global-display capability depends on
ray-tracing hardware.

Phase 12C accepted this additive extension without changing any ABI-v1 symbol,
field, structure, or behavior. The accepted source commit is
`64d72f2a4ee0848bf7ff73c345fcd1cf56579ba1`; its commit-qualified bridge is
`ksa64_viewer_bridge_64d72f2a4ee0.dll` with SHA-256
`b8c5f1b3890fa94b0a182a39bf25a017741a061974b7424addeda56c9c998c85`.
The adjacent manifest has SHA-256
`e505095789d94189791963b82bbc4947c588479380b5210c50dfd6c59ca49777`.

The strict joined `ksa64.phase12c.cross-renderer-evidence.v2` record has
SHA-256
`c869a5dbc341ea6b5272e901882fe803dd2e15f1ab49cbeff48788527c01e50e`.
It binds the native bridge harness, the packaged Unreal evidence, and the
rendered-browser evidence; compares all nine reviewed nominal milestones and
six guided action/fault milestones; and requires complete source/path
products, event/discontinuity masks, continuity identities, raw path-state
flags, normalized view mode, role filtering, and terminal disposition.

The accepted bridge polling thresholds apply only to availability and exact
sample-range service calls: 8,500 ns p99 and 364,600 ns p99 respectively.
Path retrieval is measured separately and is not covered by the sub-millisecond
polling claim. The packaged Unreal viewer reused the operations plugin's single
loaded bridge and recorded 305,300 ns p99 display-publication service time;
this is not total GPU frame latency.
