# Phase 6 software completion record

Status: accepted software baseline. Live physical-link acceptance remains open and is not implied by this record.

KSA64 now has explicit world, flight, broker, and Mission Control boundaries; strict exact-paced and realtime wire contracts; deterministic impairment/replay behavior; a C64-oriented realtime flight profile; stock target endpoints; and complete host-plus-C64 execution evidence.

## Exit criteria

| Criterion | Evidence | Result |
|---|---|---:|
| Split execution preserves the accepted model | The allocation-free exact world and flight endpoints reproduce the frozen Phase 5 3,133-step terminal state and sensor/navigation/flight checksums. | Pass |
| Links are strict and replayable | KLF6 and KLR6 reject bad length, identity, flags, reserved bytes, epochs, COBS, CRC-32, and CRC-16. Deterministic corruption, loss, duplication, delay, and disconnect schedules freeze transcript behavior. | Pass |
| Flight software remains truth-isolated | Only the world imports vehicle truth. Flight consumes KLR6 cells and returns next-epoch commands. Mission Control is passive. | Pass |
| One stock C64 can execute the realtime flight workload | Three PAL CIA measurements put ordinary, navigation/status, and guidance releases below the 24,631-cycle conservative slot budget. | Pass |
| Complete target execution is exact | A stock C64 endpoint under 1x PAL x64sc processed all 12,692 epochs. Every command/status cell matched a native shadow flight computer and terminal evidence was exact. | Pass |
| Failure behavior is bounded | Stale cells, deadlines, malformed cells, transport faults, late commands, and disconnects fail closed. VICE harnesses close on success or proven failure and prevent concurrent audit instances. | Pass |
| Physical deployment is accessible | One C64 plus a host is the baseline topology; two- and three-C64 arrangements are optional. All endpoint images fit stock memory. | Pass for packaging |
| Live physical transport is measured | ACIA and Ultimate transport software exists, but no complete physical-link mission has been run. | Open |

## Exact-paced evidence

The zero-impairment split Phase 5 mission remains exact:

| Evidence | Value |
|---|---:|
| Steps | 3,133 |
| Terminal position Q12 | `[21468577, 3871182, 15698368]` |
| Terminal velocity Q24 | `[-66327286, 89767125, 68337641]` |
| Sensor checksum | `0x67d05c4a` |
| Navigation checksum | `0xb2938ca6` |
| Flight checksum | `0xf27c8d9a` |

## KSA-6R full-flight evidence

| Evidence | Value |
|---|---:|
| Fast epochs | 12,692 |
| Mission steps | 3,173 |
| Simulated mission duration | 396.625 s |
| Externally paced wall duration | 1,011.328 s |
| Terminal position Q12 | `[21360371, 4030786, 15731027]` |
| Terminal velocity Q24 | `[-69442203, 96406364, 65655653]` |
| Navigation position Q12 | `[21360000, 4031445, 15731484]` |
| Navigation velocity Q24 | `[-68076267, 95786604, 65320561]` |
| Navigation checksum | `0x82e09168` |
| Final flight checksum | `0xacf09b87` |
| Deadline misses / alarms | 0 / 0 |

The binary-monitor relay deliberately pauses x64sc while transferring mailbox data, so wall time is not a live-link performance result. The run proves full target execution at normal PAL CPU speed and exact cross-target behavior. Compute feasibility is established separately by CIA timing.

## Timing and memory

| Artifact/release | Accepted result |
|---|---:|
| Ordinary fast release | 12,339 cycles |
| Navigation/status release | 23,656 cycles |
| Guidance release | 14,914 cycles |
| Conservative release budget | 24,631 cycles |
| Projected controller CPU demand | 196.436 s, about 49.5% average |
| Physical ACIA endpoint | 17,554 bytes, ends `$4C91` |
| VICE mailbox endpoint | 15,324 bytes, ends `$43DB` |
| Endpoint probe | 16,491 bytes, ends `$486A` |
| Timing probe | 14,924 bytes, ends `$424B` |

No endpoint requires an REU.

## Target-only findings

Full acceptance exposed two rust-mos behaviors that short probes had not reached:

1. a loop with an upper bound of 256 compiled into a nonterminating sequence; two explicit 128-byte spans replaced it;
2. direct negative `i16` widening produced a different checksum chain from native; byte-explicit sign extension replaced it.

The full broker now compares every command and status cell against a native shadow flight computer, and frozen checkpoints at every 1,024 epochs fail early on checksum drift.

## Transport limitation

The pinned Windows VICE 3.10 socket-backed ACIA sent bytes from C64 to host but did not deliver host bytes to the C64 in the tested configurations. Automated completion therefore uses the VICE-only mailbox adapter. The stock SwiftLink/Turbo232 endpoint is built and tested at the codec/driver level but requires a later finite run on actual compatible hardware. Ultimate UCI and user-port hardware also remain pending.

## Completion audit

The bounded runner is:

```powershell
powershell -File phase6/complete.ps1
```

It validates formatting, compilation, lints, the entire native regression suite, stock target packaging, refreshed finite timing and endpoint probes, one direct mailbox exchange, an eight-epoch host/VICE/host-Mission-Control exchange, and the checked-in full-flight artifact. It does not silently rerun the full 17-minute target mission.


## Operational console addendum

The host-native Mission Control TUI is additive to the accepted Phase 6 software baseline. It changes no KLR6 cell, checksum, target image, scheduler, memory budget, or command path. The acceptance test records one complete 12,692-epoch native mission, reloads exact terminal evidence, exports CSV/JSON, recovers a deliberately truncated footer, and renders all seven pages at 120×40 plus the Flight Director page at 80×24. A bounded eight-epoch host/VICE smoke remains shadow-exact after launcher integration and the relay closes its single VICE instance.

Live pacing, pause, single-step, display freeze, bookmarks, sound, and safe detach are operator/presentation functions. Pacing changes only wall-clock release time. Stop is explicit; detach continues headlessly. KMR6 is noncanonical and cannot substitute for the frozen full-flight artifact or physical-link acceptance.
