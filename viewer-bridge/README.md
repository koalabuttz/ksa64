# KSA64 Phase 12 viewer bridge

This crate is KSA64's only in-process presentation ABI. The Phase 12A entry points remain frozen: they call `Ksa64Application::start_mission` and give one dedicated Rust worker exclusive ownership of the compact accepted `LiveMissionSession`.

Phase 12B adds a strictly additive operations API. A versioned start request can select the complete guided GNSS-loss scenario, which runs the accepted host `FullMissionSession`. Unreal receives role-filtered operational, procedure, disposition, prediction, timeline, release-sample, action-receipt, and transport views. High-level operator actions still pass through the existing Phase 11 review, stage, commit, and cancel boundary. The legacy `ksa64_viewer_start` function and its frozen C++ harness behavior are unchanged.

The ABI uses only fixed-width C fields, explicit ABI and structure sizes, validity masks, opaque handles, caller spans, and Rust-owned buffers released by `ksa64_viewer_free_buffer`. Commands use a bounded nonblocking queue. Roles are immutable for a session, and filtering occurs in Rust before data crosses the ABI. No Unreal-side code receives canonical evidence internals or private SIM truth through an operational role.

Build the production DLL with `cargo build -p ksa64-viewer-bridge --profile viewer`. The panic probe is test-only and is exported only with `--features panic-probe`.

`harness/build.ps1` builds the DLL and independent Windows C++ loader when a Visual Studio C++ environment is active. The wrapper stages a commit-qualified DLL plus a SHA-256/ABI manifest under the ignored `target/viewer` directory. The harness dynamically resolves the legacy function table and compiles all Phase 12B C layout assertions; it never links Rust internals. Rust ABI tests cover the additive operations entry points and require completed bridge-driven KSB11 bytes to equal the direct accepted application path.

The harnesses remain deliberately separate. `harness/build.ps1` runs the frozen Phase 12A lifecycle/misuse oracle unchanged. `harness/build-full.ps1` runs the Phase 12B 21,591-release scripted-operator mission through the additive ABI, exercises Review/Stage/Commit at the accepted windows, validates every role-filtered presentation surface, strictly scans the sealed KSB11 framing and CRC chain, and checks the accepted SHA-256. `harness/build-all.ps1` invokes both explicitly; use it for the combined acceptance gate.

## In-process containment limit

The unwind-enabled profile and both ABI/worker panic boundaries contain ordinary Rust panics and convert them into typed diagnostics. In-process containment cannot guarantee recovery from operating-system out-of-memory termination, access violations, stack overflow, explicit process abort, corrupted foreign pointers, or equivalent non-unwinding faults. If those faults occur in feasibility testing—or the Editor cannot reliably survive bridge replacement or failure—the documented sidecar-process fallback applies.

Snapshot polling is stateful per handle: the first published snapshot returns `KSA64_VIEWER_OK`, and a repeat with no newer publication returns `KSA64_VIEWER_UNCHANGED` without touching canonical mission state. Caller spans are capped at 16 MiB and copied into Rust-owned memory during the export; no caller pointer is retained.

## Phase 12B operational boundary

- `ksa64_viewer_start_v1` chooses the compact legacy fixture or complete GNSS-loss operations session.
- Operational views are public, role-filtered data; F7/SIM truth is represented only for a SIM Director session.
- Action proposals are typed bridge records generated and validated in Rust. Stage, commit, and cancel remain exact Phase 11 operations.
- Polling status, timelines, release samples, receipts, and prediction points is passive and cannot advance the mission.
- `ksa64_viewer_request_shutdown_v1` is asynchronous. `ksa64_viewer_finish_status_v1` and transport status expose progress without blocking the game thread.
- Destroy remains the final ownership boundary and joins the worker only after shutdown has been requested or completion has occurred.
