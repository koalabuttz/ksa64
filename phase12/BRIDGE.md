# Phase 12A Rust-to-Unreal bridge contract

Status: normative 12A interface and containment contract.

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
- Translate panic, invalid state, malformed input, allocation failure, worker
  death, and queue failure into stable result codes and diagnostics.
- No exception or panic crosses the C boundary.
- Do not retain caller pointers after an export returns.
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