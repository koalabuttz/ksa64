# KSA64 Phase 12A viewer bridge

This crate is the only in-process presentation ABI. It calls `Ksa64Application::start_mission` and gives one dedicated Rust worker exclusive ownership of the resulting `LiveMissionSession`.

The ABI is versioned and uses only fixed-width C fields, opaque session handles, caller spans, and Rust-owned buffers released by `ksa64_viewer_free_buffer`. Commands use a bounded nonblocking queue. Snapshots are truth-blind application snapshots selected by the immutable session role. Uplink actions remain the accepted KUL11/KUA11 wire records. MCP, Python, Unreal Editor code, coordinate conversion, interpolation, and rendering are deliberately absent.

Build the production DLL with `cargo build -p ksa64-viewer-bridge --profile viewer`. The panic probe is test-only and is exported only with `--features panic-probe`.

`harness/build.ps1` builds the DLL and the independent Windows C++ loader when a Visual Studio C++ environment is active. The wrapper stages a commit-qualified DLL plus a SHA-256/ABI manifest under the ignored `target/viewer` directory. The harness dynamically resolves the function table; it never links Rust internals. The Rust ABI test separately requires the completed bridge-driven KSB11 bytes to equal the direct accepted application path.
## In-process containment limit

The unwind-enabled profile and both ABI/worker panic boundaries contain ordinary Rust panics and convert them into typed diagnostics. In-process containment cannot guarantee recovery from operating-system out-of-memory termination, access violations, stack overflow, explicit process abort, corrupted foreign pointers, or equivalent non-unwinding faults. If those faults occur in feasibility testing—or the Editor cannot reliably survive bridge replacement or failure—Phase 12A stops the in-process path and triggers the documented sidecar-process fallback.

Snapshot polling is stateful per handle: the first published snapshot returns `KSA64_VIEWER_OK`, and a repeat with no newer publication returns `KSA64_VIEWER_UNCHANGED` without touching canonical mission state. Caller spans are capped at 16 MiB and copied into Rust-owned memory during the export; no caller pointer is retained.