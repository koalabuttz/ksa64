# Phase 12A completion record

Status: complete and accepted on 2026-07-27.

Phase 12A proves that an Unreal Engine presentation process can consume KSA64
through a versioned, role-filtered, nonblocking bridge while Rust remains the
only simulation, commanding, and evidence authority. It adds no renderer,
alternate physics, authoring surface, or canonical `K*` format.

## Accepted outcome

- The Phase 0 through Phase 11.5 audit remains exact from the Phase 12A
  compatibility baseline `8208d53`.
- The validated Phase 12A source is commit
  `e98df4921c03ddbebba6b9f0d2f4d5fe306e48bc`.
- The sanitized workstation/toolchain lock has SHA-256
  `1a50c4d7bed2634cac04596fe62761b523a3376e0c6a029180fbe34b75fb58fa`.
- The Phase 8 Firestorm and Phase 10 KSA-G10R source-data portability repairs
  preserve the already accepted byte streams exactly. They change repository
  text treatment only; no accepted pack, identity, model, or result changed.
- The accepted `ksa64.product-catalog.v1` remains 13 experiences and 21,068
  bytes with SHA-256
  `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`.
- A complete guided GNSS-loss session through the C ABI produces the unchanged
  22,369-byte KSB11 bundle, identity `0x6d4122a0`, and SHA-256
  `38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8`.
- The bridge is ABI version 1, build identity `0x120a0001`, and is loaded by a
  commit-qualified filename after manifest and SHA-256 verification.
- One Rust worker owns each live-session handle. Unreal-facing calls enqueue or
  poll bounded queues and never execute simulation work on the game thread.
- Guided-operator snapshots are filtered in Rust and expose no SIM Director
  truth fields.
- A native C++ harness and the minimal Unreal runtime plugin use the same
  public header and dynamically loaded function table.
- A packaged Development build loads the bridge without Unreal Editor, MCP,
  Python, or editor-only plugin binaries.
- The optional Unreal MCP connection was proven on loopback for one disposable
  editor mutation; normal build, test, package, and runtime paths do not depend
  on it.

## Bridge evidence

The accepted clean bridge artifact is:

- DLL:
  `ksa64_viewer_bridge-e98df4921c03-120a0001.dll`
- size: 614,912 bytes
- DLL SHA-256:
  `d1605c4aa9a8b407d8e35ee76d965e404c1e7efcc357d8bd0704b73ade43272d`
- manifest SHA-256:
  `391e59246a07be5639b2d488249e82bcd9b1b4c1c451a18d56b1d579593de316`
- C header SHA-256:
  `ae863a6ce4535280d6d224687052f716581f849ac17fcb39510779657b2d1c86`

The frozen public structure sizes are 132 bytes for ABI information, 24 bytes
for spans, 32 bytes for owned buffers, 24 bytes for events, and 184 bytes for
snapshots. Command and event queues are bounded at 32 and 256 entries;
advancement is bounded to 64 releases per call; caller spans are bounded to
16 MiB.

Rust layout, lifecycle, ownership, action, panic-containment, role-filtering,
KSB11 parity, and queue-pressure gates passed. The final production harness
observed a maximum enqueue time of 0 microseconds and a 4,096-poll total of 798
microseconds, with no individual poll above timer resolution. The earlier
panic-enabled build observed a 3-microsecond maximum enqueue and a 765
microsecond polling total. These are diagnostic host measurements, not
realtime guarantees.

## Unreal evidence

The empty UE 5.8 C++ shell remains deliberately free of scene actors,
coordinate conversion, replay interpolation, operational UI, vehicle art, and
NASA assets.

- The prior clean Editor build completed in 587.396 seconds.
- The final incremental Editor build completed in 99.03 seconds using
  file-redirected UnrealBuildTool output.
- Unreal automation passed 2 tests with 0 failures. The report is 1,484 bytes,
  SHA-256
  `eeaa21ed3b440b79c54f1f25c631ca971a9c3129f79bfd214a9f8751203131af`.
  The run completed in 30.766 seconds; the tests themselves took 0.0743478
  seconds.
- The final package contains 54 files and 1,031,144,201 bytes.
- The packaged game executable SHA-256 is
  `29e21f606091beafd7d821266f26ecf28c506aaeae8d215784bc851d396ede44`.
- The packaged bridge manifest matches the accepted manifest above.
- The packaged headless smoke process exited 0. Its log SHA-256 is
  `3b19648bfe62e7e2f4b2b22e2803c40d960aa7f3516c89841586c4b2d3af2189`.
- The package audit SHA-256 is
  `899cdd3b98acd528727f90b12ed7f2cf25bed2df2832e0420835c91d43232972`.
- No MCP, Toolset Registry, AllToolsets, or Python editor plugin binary was
  packaged.

Captured-console output could deadlock UnrealBuildTool on this workstation;
the reliable gate redirects stdout and stderr to files. Headless automation
uses Unreal's memory-cache fallback when its normal cache service is
sandboxed. Packaging uses PowerShell 7 and `-SkipZenStore`, because the
Launcher build's BaseGame configuration otherwise attempted an unavailable
Zen path. These are recorded toolchain behaviors, not product dependencies.

## MCP feasibility

The optional Unreal MCP plugin version 1.0 exposed 52 toolsets at
`127.0.0.1:8000/mcp`. It inspected `/Engine/Maps/Entry`, created and verified
one disposable actor, removed it, and verified its absence. No tracked map or
asset survived the test.

MCP remains experimental, loopback-only, supervised development tooling. It is
not invoked by [complete.ps1](complete.ps1), the build, automation, packaging,
or the packaged application.

## Source and storage hygiene

- No `.uasset` or `.umap` file is tracked.
- Unreal binary-asset LFS rules are active even though Phase 12A introduces no
  production binary asset.
- `Binaries`, `Intermediate`, `Saved`, DDC, cooked output, and local captures
  remain generated and ignored.
- The clean short-path checkout had no Unreal process running at completion.

Observed local disk use was:

| Area | Bytes | Files |
|---|---:|---:|
| UE 5.8 installation | 32,192,981,609 | 249,068 |
| Derived Data Cache | 477,899,230 | 4,911 |
| Project `Binaries` | 801,560,156 | generated |
| Project `Intermediate` | 5,705,263,043 | generated |
| Project `Saved` | 1,640,682,611 | generated |
| Packaged Development build | 1,031,144,201 | 54 |

## Validation and repeatability

Machine-readable evidence is in
[completion-audit.json](completion-audit.json). The repeatable non-live audit
is [complete.ps1](complete.ps1). Its default path runs frozen/native/bridge
checks only. Unreal build, headless automation, and packaging require separate
explicit switches. The script never starts Unreal Editor, MCP, or packaging
implicitly.

The engine and authority decision is [ENGINE_DECISION.md](ENGINE_DECISION.md),
the ABI contract is [BRIDGE.md](BRIDGE.md), the workstation lock is
[toolchain-lock.toml](toolchain-lock.toml), and the next implementation
boundary is [PHASE12B_HANDOFF.md](PHASE12B_HANDOFF.md).

## Remaining boundaries

- Phase 12B owns the live GNSS-loss presentation, procedure workflow, operator
  forms, smooth/exact display modes, and action UX.
- Phase 12C owns global coordinate/display domains, complete Phase 10 replay,
  Earth rendering, vehicle pose, entry, recovery, and packaged performance.
- Phase 12D owns vehicle/mission authoring and GUI/headless compiler parity.
- Phase 12E owns production art, NASA-derived visual assets, effects, quality
  tiers, and visual-performance acceptance.
- The sidecar fallback was not needed. It remains available if future
  in-process containment, DLL replacement, or editor-failure behavior becomes
  inadequate.
- This phase makes no real-vehicle accuracy, launch approval, certification,
  regulatory, or safety-authority claim.
