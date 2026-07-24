# Phase 6 contract

Status: software contract frozen. Physical electrical profiles are not yet frozen.

Phase 6 is additive. Every Phase 5 entry point, KST5 artifact, mission checksum, and generated identity remains frozen. The single-process Phase 5 composition is the regression oracle for exact-paced execution.

## Endpoint authority

The world endpoint alone owns truth, vehicle dynamics, sensor synthesis, mission time, and canonical recording. The flight endpoint receives transported measurements and returns commands; it cannot import simulator truth. Mission Control is observational and has no command authority. The host acceptance broker may run a shadow flight computer, but that shadow validates returned cells and never supplies commands to the world.

## KLF6

KLF6 is a COBS-delimited, CRC-32-protected link frame with a 36-byte decoded header, at most 512 payload bytes, and at most 556 encoded bytes. The header carries session, sequence, acknowledgement, measurement, production, and effective epochs. Unknown record types, required flags, nonzero reserved bytes, identity mismatches, malformed COBS, and bad checksums fail closed.

Exact-paced ordering is sensor N, command N, committed transition N to N+1, then commit acknowledgement. Duplicate sensors return the cached command and duplicate commands cannot advance the world twice. Impairment decisions are deterministic and contribute to the transcript chain.

## KLR6 realtime cells

KSA-6R uses fixed 40-byte inertial, 24-byte command, 64-byte aid, and 48-byte status cells protected by CRC-16-CCITT. A flight endpoint begins a raw KLR6 stream with `[d6 5a 06 00]`. It runs fast control at 32 Hz, navigation and status at 8 Hz, and sliced guidance at 1 Hz.

Each command carries its source epoch and the following effective epoch. Aid appears only on navigation releases; status appears on the same 8 Hz cadence. The terminal inertial cell sets flag bit zero after the world has committed its final mission step. The flight endpoint processes that cell, returns its final command/status evidence, publishes its result, and stops advancing.

The full KLF6 channel remains responsible for general session negotiation, capabilities, reconnect, and replay. KLR6 is a compact, reviewed flight stream, not a replacement for the general link contract.

## Failure policy

Exact-paced links retry twice and then safe. Realtime links hold the preceding continuous command for at most two missed fast epochs, never repeat discrete actions, and latch cutoff/abort on the third miss. Late, wrong-session, or wrong-effective-epoch commands are rejected. Observational traffic sheds before control traffic. A release exceeding the measured deadline budget latches safeing.

## Determinism and evidence

- Draws, physics, sensors, navigation, guidance, commands, and broker decisions remain ordered and deterministic.
- Native TCP and C64 mailbox paths must return identical KLR6 command/status cells.
- The acceptance broker compares every returned cell with a native shadow flight computer and rejects the first mismatch.
- Frozen status checkpoints at 1,024-epoch intervals detect checksum drift early.
- C64 final navigation and flight checksums must match the native oracle exactly.
- Recording, REU storage, Mission Control, and transcript handling cannot alter command ordering or simulation state.

## Hardware baseline

Every C64 endpoint must fit stock memory and execute without an REU. REU capacity may buffer evidence but cannot affect live ordering. User-port serial at 9,600 baud is exact-paced only. SwiftLink/Turbo232 and Ultimate UCI are optional realtime candidates.

The current software contract does not freeze a user-port electrical interface, ACIA cartridge clone, Ultimate firmware requirement, or cable pinout. Those become accepted only after finite tests on actual hardware. The VICE mailbox is an automation adapter and must never be presented as a physical transport.
