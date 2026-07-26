# C64 target catalog and execution policy

The unified target catalog describes accepted C64 products without changing their programs or evidence.

## Current targets

| ID | Placement | Timing | Notes |
|---|---|---|---|
| `c64.firestorm.vertical` | Stock standalone | Long run | Accepted complete mission; about 17.72 PAL CPU minutes |
| `c64.firestorm.spatial-replay` | Stock replay | Replay only | Full spatial world remains a long-run target |
| `c64.firestorm.advanced-flight` | Host world / C64 flight | Externally paced | Finalist bootstrap configures advanced effectors |
| `c64.ksa-g10r.global-flight` | Host world / C64 flight | Externally paced | Finite release and frame-transition evidence |
| `c64.ksa-g10r.global-replay` | Stock replay | Replay only | Passive KPH10/KSR10 browser |
| `c64.ksa-g10r.safehold` | Stock bounded package | Externally paced | Flat SafeholdRecoveryV1 endpoint |
| `c64.ksa-g10r.reference-ops` | Host world / C64 flight | Externally paced | Banked stock-RAM operations stopgap |

None requires an REU. Optional expansion memory may increase retained evidence only where the owning phase explicitly supports it.

## Verify, build, and probe

```powershell
cargo run -p ksa64-host --bin ksa64 -- target show c64.ksa-g10r.reference-ops
cargo run -p ksa64-host --bin ksa64 -- target verify c64.ksa-g10r.reference-ops
cargo run -p ksa64-host --bin ksa64 -- target build c64.ksa-g10r.reference-ops
cargo run -p ksa64-host --bin ksa64 -- target probe c64.ksa-g10r.reference-ops --live
```

`verify` only checks the catalog-bound stored evidence file and never starts an emulator. The owning historical audit remains the deep semantic validator. `build` may invoke the pinned rust-mos toolchain but never VICE. `probe --live` delegates to the owning bounded acceptance script.

Live policy is unchanged:

1. Refuse to start while another VICE process exists.
2. Use one instance at a time with warp disabled.
3. Close it after success or proven failure.
4. Observe the documented cooldown between sequential instances.
5. Never terminate a healthy run merely because it is slow.
6. Never begin a long complete target mission without a fresh projection and explicit confirmation.

Phase 11.5 added no C64 code and did not revise any stock-memory decision. The portable world, 6502-specific rewrite, C64 Ultimate acceleration, physical loader/link, and realtime tracks remain separate roadmap work.
