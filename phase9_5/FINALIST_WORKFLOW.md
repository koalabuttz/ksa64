# Phase 9.5 Mission Control and finalist workflow

Phase 9.5 keeps optimization on the host and gives the C64 two bounded roles:

1. Browse accepted KFE9 finalist evidence in a stock presentation image.
2. Execute the genuine flight and PriorityResidualV1 kernels for a selected finalist while the host owns the authoritative world.

Neither role requires an REU. REU capacity changes retained history only.

## Live host Mission Control

Run host world plus host flight with the passive seven-page console:

```powershell
cargo run -p ksa64-host --bin phase9_5_launch -- --display tui --pace realtime
```

For bounded evidence or automation:

```powershell
cargo run -p ksa64-host --bin phase9_5_launch -- --display summary --pace fast --max-releases 64 --record target/phase9_5/session.kmr9
```

F1–F6 consume public flight/link telemetry. F7 alone exposes simulator truth. Rendering and KMR9 recording are passive sinks; the 64-release invariance test produces the same terminal checksums with and without them.

## Inspect and retain finalists

Inspect the first mixed finalist, show the stock retention plan, and repeat its accepted 64-case evaluation:

```powershell
cargo run -p ksa64-host --bin phase9_5_finalists -- --package phase9_5/evidence/workbench/mixed-nsga2.kfe9 --index 0 --reu-kib 0 --rerun
```

Use `--reu-kib 128` or a larger detected tier to preview proportional history retention. Use `--subset 0,2,4 --output target/finalists.kfe9` to create a strict smaller package. Candidate ordering and evaluation do not change.

The stock browser directly validates KFE9 framing, CRCs, identities, design vectors, robust aggregates, and nominal KAS9 evidence. Its pages are Status, Pareto, Design, Objectives, Constraints, Evidence, and Integrity.

## Selected finalist on a C64

The selected-finalist path is deliberately additive:

```text
KFE9 candidate
    -> host materializes validated KPE9/KPA9-bound packs
    -> strict 352-byte KFB9 Start bootstrap
    -> stock C64 flight + allocator endpoint
    -> exact KLR9 commands/status
    -> host shadow comparison
    -> host world advances
```

KFB9 carries only the bounded flight and allocation configuration. It binds manifest, study, candidate, vehicle, effector, and allocator identities. It does not replace the design packs or become a second physical model.

Build the endpoint with the pinned rust-mos image:

```powershell
powershell -File tools/toolchains/rust-mos.ps1 cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase9-5-finalist-flight-endpoint-c64
cargo build -p ksa64-host --release --bin phase9_5_finalist_bridge
```

Run a finite VICE proof for a selected package:

```powershell
python phase9_5/reference/vice_finalist_split.py --vice .toolchains/vice/3.10/GTK3VICE-3.10-win64/bin/x64sc.exe --prg target/mos-c64-none/c64/ksa64-phase9-5-finalist-flight-endpoint-c64 --broker target/release/phase9_5_finalist_bridge.exe --package phase9_5/evidence/workbench/mixed-nsga2.kfe9 --index 0 --max-releases 8
```

The accepted Gate 11 evidence runs canard, RCS, and mixed finalists sequentially. Each probe uses one VICE instance, disables warp, closes the emulator after success or proven failure, and verifies every returned cell against the host shadow.

## Accepted stock sizes

| Image | Bytes | End address | REU |
|---|---:|---:|---:|
| KFE9 finalist browser | 29,010 | `$7951` | No |
| KFB9 configurable flight endpoint | 39,963 | `$A41A` | No |

The configurable endpoint leaves 7,142 bytes before `$C000`. It is externally paced: simulated 32 Hz epochs and successor-command semantics are exact, but the host waits for each C64 answer and therefore makes no wall-clock realtime claim.

## Open hardware boundary

VICE proves stock target execution and exact placement behavior. A physical user-port, ACIA, or C64 Ultimate Ethernet transport remains the Phase 6 hardware-acceptance boundary. The same KLF6/KLR9/KFB9 contracts are intended to survive that transport change.
