# Phase 6: Commodore-in-the-loop

Status: software baseline accepted; physical-link acceptance remains open.

Phase 6 turns the Phase 5 world/flight seam into explicit endpoints joined by deterministic, replayable transports. The exact-paced path preserves Phase 5 results. The separate KSA-6R profile gives one stock C64 a computationally feasible 32 Hz flight-computer workload while a host or another machine owns the simulated world.

## Accepted software baseline

- KLF6 COBS framing, CRC-32, capabilities, explicit epochs, bounded payloads, retry/safeing policy, and deterministic impairment transcripts.
- Fixed KLR6 inertial, aid, command, and status cells with CRC-16 and a four-byte readiness preamble.
- Allocation-free world and flight endpoints. Only the world owns truth; commands become effective in the following epoch.
- A split native exact-paced mission that reproduces the frozen Phase 5 terminal state and checksum chains.
- A KSA-6R scheduler with 32 Hz control, 8 Hz navigation/status, and 1 Hz sliced guidance.
- Transport-neutral memory, ACIA, and Ultimate-UCI state machines, including bounded queues and native fault/backpressure tests.
- Passive Mission Control and an independent delayed/noisy ground estimator that cannot perturb flight.
- A stock physical-flight PRG for SwiftLink 38,400 or Turbo232 57,600 baud, plus a VICE-only mailbox PRG.
- A complete externally paced flight under x64sc at 1x PAL CPU speed: all 12,692 command/status cells matched an independent host shadow flight computer, the terminal state and checksums were exact, and there were zero deadline misses or alarms.

## Full-flight evidence

The accepted KSA-6R run completed 12,692 fast epochs and 3,173 mission steps. The C64 reported navigation checksum `0x82e09168` and final flight checksum `0xacf09b87`. The exact terminal state was:

- position Q12: `[21360371, 4030786, 15731027]`;
- velocity Q24: `[-69442203, 96406364, 65655653]`;
- navigation position Q12: `[21360000, 4031445, 15731484]`;
- navigation velocity Q24: `[-68076267, 95786604, 65320561]`.

The binary-monitor mailbox relay took 1,011.328 wall seconds for 396.625 simulated seconds because monitor access pauses the emulated CPU. This proves exact execution at normal PAL CPU speed, not end-to-end wall-clock realtime transport. The separate CIA timing probe establishes controller compute feasibility; a live ACIA or physical-link run remains the final transport proof.

## Capability and deployment ladder

| Deployment | Status |
|---|---|
| Single-process native oracle | Accepted |
| Split native world/flight over TCP | Accepted |
| Host world plus one VICE C64 flight computer | Accepted through externally paced mailbox relay |
| Stock C64 flight endpoint image | Built and stock-memory bounded |
| SwiftLink/Turbo232 physical link | Driver and endpoint implemented; hardware run pending |
| Ultimate UCI TCP | Allocation-free state machine and native mock accepted; hardware adapter pending |
| User-port exact-paced link | Protocol/bandwidth policy accepted; electrical and target driver work pending |
| Two or three physical C64s | Optional demonstration, not a baseline requirement |
| Self-contained world and flight on one C64 | Separate post-Phase-6 feasibility track |

## Important implementation findings

The pinned rust-mos toolchain exposed two target-only traps during full acceptance. An integer loop written with a 256 upper bound compiled into a nonterminating 16-bit decrement sequence, so mailbox initialization now uses two explicit 128-byte spans. Direct negative `i16` widening inside the checksum also differed from the native compiler; the accepted implementation hashes explicit little-endian bytes and sign bytes. Frozen 1,024-epoch checkpoints and a per-cell shadow flight computer now detect either class of divergence early.

The pinned Windows VICE 3.10 ACIA socket backend successfully transmitted C64 bytes to the host but did not deliver host bytes back to the emulated ACIA in the tested configurations. The physical ACIA endpoint remains valid software, while automated target acceptance uses the binary-monitor mailbox transport. This limitation is documented rather than treated as physical-link evidence.

## Running the system

The supported deployment launcher now runs either the entire KSA-6R stack on the host or places the flight computer in one VICE instance while the host owns the world and passive Mission Control:

```powershell
# Fast all-host mission
powershell -File phase6/run.ps1

# Host world + VICE flight computer + host Mission Control
powershell -File phase6/run.ps1 -Flight vice -MissionControl host -Pace realtime
```

Mission Control records both validated cell streams, accepts 1,587 deterministic delayed/noisy ground fixes, maintains an independent estimate, and compares it with onboard navigation. Disabling Mission Control produces identical terminal and avionics evidence. VICE world and multi-VICE deployments remain explicitly unsupported because no C64 world endpoint has been packaged. See [LAUNCHER.md](LAUNCHER.md) for the complete placement and pacing matrix.

## Live Mission Control presentation

The host presentation layer consumes the same passive `MissionControlUpdate` stream used by the accepted broker. It cannot issue vehicle commands and its recording, rendering, audio, export, pause, or storage failures cannot alter physics or flight-software evidence.

The 120×40 flagship dashboard provides seven operational pages:

| Key | Console |
|---|---|
| F1 | Flight Director overview and GO/NO-GO state |
| F2 | Trajectory vectors, altitude, velocity, range, and trends |
| F3 | Guidance, attitude, gimbal, RCS, and navigation-error history |
| F4 | Onboard navigation, independent ground estimate, and comparison |
| F5 | Stage, mass, propellant, loads, flexibility, and sequencing |
| F6 | KLR6 cells, tracking fixes, checksums, alarms, and event log |
| F7 | Clearly marked omniscient SIM Director state for simulation operations |

The layout remains usable at 80×24 and expands with the terminal. Operational pages display transported telemetry and independent ground products; only F7 exposes simulation truth. Space pauses/resumes, `.` releases one epoch, `[`/`]` select 0.25x through MAX, `U` changes units, `S` changes procedural sound, `B` bookmarks, `F` freezes the view, and `E` exports. Quitting an active mission explicitly offers stop, detach-and-continue-headless, or cancel. A completed mission remains open for postflight review.

Every live run records a host-only, noncanonical KMR6 session by default. Each update has its own CRC-protected binary record; a truncated file replays through the last valid record. The recorder stores KLR6 cells, passive ground products, presentation history, and terminal evidence without replacing KST5 or accepted KLR6 evidence. Sessions replay through `phase6_launch --replay` and export through the `phase6_session` tool.

## Audit

Run the bounded software audit with:

```powershell
powershell -File phase6/complete.ps1
```

It runs native regressions, lints, target builds, three finite PAL timing/endpoint probes, direct and Mission-Control mailbox smoke exchanges, and validation of the checked-in full-flight artifact. It deliberately does not rerun the approximately 17-minute full mission. The runner refuses to start if another x64sc process is already open and every probe closes its VICE instance on success or proven failure.

See [LAUNCHER.md](LAUNCHER.md), [CONTRACT.md](CONTRACT.md), [REALTIME.md](REALTIME.md), [TRANSPORTS.md](TRANSPORTS.md), and [COMPLETION.md](COMPLETION.md).
