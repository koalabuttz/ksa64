# Phase 6 contract

Status: implementation in progress.

Phase 6 is additive. Every Phase 5 entry point, KST5 artifact, mission checksum, and generated identity remains frozen. The single-process Phase 5 composition is the regression oracle for the exact paced profile.

## Endpoint authority

The world endpoint alone owns truth, vehicle dynamics, sensor synthesis, mission time, and canonical recording. The flight endpoint receives measurements and returns commands; it cannot import simulator truth. Mission Control is observational and has no command authority.

## KLF6

KLF6 is a COBS-delimited, CRC-32-protected link frame with a 36-byte decoded header, at most 512 payload bytes, and at most 556 encoded bytes. The header carries session, sequence, acknowledgement, measurement, production, and effective epochs. Unknown record types, required flags, nonzero reserved bytes, identity mismatches, malformed COBS, and bad checksums fail closed.

Exact paced ordering is sensor N, command N, committed transition N to N+1, then commit acknowledgement. Duplicate sensors return the cached command and duplicate commands cannot advance the world twice.

## KLR6 realtime cells

The separately identified KSA-6R profile uses fixed 40-byte inertial, 24-byte command, 64-byte aid, and 48-byte status cells protected by CRC-16-CCITT. It runs fast control at 32 Hz, navigation and sequencing at 8 Hz, and sliced guidance at 1 Hz. The full KLF6 channel remains responsible for session control and capability negotiation.

## Failure policy

Paced links retry twice and then safe. Realtime links hold the preceding continuous command for at most two missed fast epochs, never repeat discrete actions, and latch cutoff/abort on the third miss. Late commands are rejected. Observational traffic sheds before control traffic.

## Hardware baseline

Stock C64 endpoints are required. REU storage may retain evidence but cannot affect live ordering. User-port serial is a 9,600-baud paced transport. The full realtime VICE bridge uses a Turbo232-compatible emulated ACIA at 57,600 baud. Ultimate UCI TCP is optional hardware with required native mock acceptance.
