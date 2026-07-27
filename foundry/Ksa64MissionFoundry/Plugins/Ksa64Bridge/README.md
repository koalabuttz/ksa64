# KSA64 Bridge plugin

This Phase 12A runtime plugin is a validated, asynchronous consumer of the
prebuilt Rust `ksa64-viewer-bridge` library. It contains no simulation,
coordinate conversion, interpolation, rendering, or operational UI.

Stage the bridge before generating or building the Unreal project:

```powershell
pwsh -File phase12/build-bridge.ps1
```

The script builds Rust outside UnrealBuildTool and stages exactly one
commit/build-qualified DLL plus an adjacent manifest under
`Binaries/Win64`. Those generated files are intentionally ignored by Git.
UnrealBuildTool only copies an already-staged pair into packaged builds.

At runtime the plugin:

1. requires one strict manifest;
2. validates its schema, clean source commit, ABI, structure sizes, accepted
   catalog identity, safe filename, and target triple;
3. calculates the DLL SHA-256 before loading it;
4. binds the complete required export table;
5. negotiates the runtime ABI and verifies the returned catalog bytes; and
6. exposes nonblocking guided-session start, one-release advancement, and
   role-filtered snapshot polling.

Snapshot polling distinguishes `NO_DATA` from `UNCHANGED`; neither is a
failure, and an unchanged poll never overwrites the caller's last accepted
snapshot. The required export table also includes the no-handle library
diagnostic endpoint so failures before session creation remain inspectable.

The automation tests intentionally exercise incompatible ABI and hash
manifests before loading, then cover catalog identity, lifecycle, one exact
release, guided-role data boundaries, and idempotent shutdown.

## Phase 12B additive surface

The loader remains the frozen Phase 12A ownership boundary, while its compatibility adapter now binds the feature-negotiated Phase 12B operations exports. Operational state, procedures, actions, dispositions, prediction paths, histories, transport status, asynchronous shutdown, and opaque completed evidence remain Rust-owned. The `Ksa64Operations` plugin presents those typed views; `Ksa64Bridge` does not acquire mission or UI authority.

The accepted build preserves ABI major 1 at build identity `0x120B0001`. Clean partial shutdown may terminate the worker with finalization still `InProgress` and no archive; only a real worker/finalizer fault is `Failed`. See [the operations plugin](../Ksa64Operations/README.md) and [the accepted Phase 12B record](../../../../phase12/PHASE12B_COMPLETION.md).
