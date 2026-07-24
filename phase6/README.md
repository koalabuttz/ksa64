# Phase 6: Commodore-in-the-loop

Status: implementation in progress.

Phase 6 turns the existing world/flight seam into explicit endpoints connected by deterministic, replayable transports. The exact paced path preserves Phase 5 results while enabling native, VICE, hybrid, and physical deployments. A distinct KSA-6R profile adds a C64-oriented realtime controller without weakening the accepted accuracy-first simulator.

## Accepted so far

- Strict KLF6 COBS framing, CRC-32, capability records, explicit epochs, and 512-byte payload bound.
- Compact KLR6 inertial, aid, command, and status cells with CRC-16.
- Allocation-free exact world and flight endpoints.
- Deterministic broker decisions and transcript checksum chain.
- A complete split native nominal mission whose terminal state, sensor checksum, navigation checksum, and flight checksum exactly reproduce Phase 5.
- An additive 32 Hz vehicle seam where four held-command ticks exactly reproduce one frozen Phase 5 step.
- The KSA-6R virtual scheduler and flight-computer skeleton with deterministic 32/8/1 Hz releases, next-epoch commands, stale-link safeing, and deadline safeing.
- A transport-neutral nonblocking byte interface, bounded queues and incremental frame pumps, a register-neutral SwiftLink/Turbo232 ACIA driver, and an Ultimate UCI TCP state machine with a deterministic native mock.

## Planned acceptance ladder

1. Frozen contracts and Phase 5 compatibility.
2. Link codec, exact split, impairment, and replay.
3. Stock endpoint packaging and paced user-port VICE exchange.
4. Native socket, VICE ACIA, and Ultimate UCI transports.
5. 32/8/1 Hz KSA-6R scheduler and controller.
6. Native and full 1x PAL realtime missions.
7. Passive Mission Control and independent ground tracking.
8. Deployment matrix, self-contained feasibility report, and completion audit.

See [CONTRACT.md](CONTRACT.md) for the frozen wire and authority rules. Long target runs retain the project rule: project first, ask when the estimate exceeds 30 minutes, and never cancel a run unless David explicitly requests it.
