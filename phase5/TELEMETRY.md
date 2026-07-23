# Phase 5 spatial telemetry

Gate 9 freezes `KST5`, the canonical integrated-mission observation stream. It is additive: KST3, KST4, and every Phase 3/4 entry point remain byte-for-byte unchanged.

## Records

A stream begins with one 96-byte header followed by 424-byte frames. All integers are little-endian. The header binds the Phase 5 numeric, scenario, environment, KSA-5A vehicle, avionics, guidance, case, seed, timestep, and maximum mission-step identities. Its CRC-32 covers bytes 0-91; reserved bytes must remain zero.

Each frame contains:

- committed 3-D truth position, velocity, acceleration, quaternion, body rates, flexible modes, mass, inertia, gimbal state, atmosphere/load data, stage state, events, alarms, and flight mode;
- the complete 128-byte spatial sensor record, including its own CRC;
- the aided navigation state and aid flags;
- the complete 32-byte actuator command, including its own CRC;
- sensor, navigation, flight, and rolling observation checksums;
- a frame CRC-32 over bytes 0-419.

Frames are emitted at every 0.125-second mission boundary. Frame zero is the non-advancing initial state. A committed successor at step N carries sensor, navigation, and command sequence N-1 because those values caused the transition. The final frame alone carries the terminal flag. The event-record flag is equivalent to a nonzero event mask.

## Validation

The portable no-std codec rejects wrong lengths, magic, versions, identities, cadence, timestep, reserved bytes, flag masks, enums, nested-record damage, sequence disagreement, and CRC failure. The host inspector additionally requires consecutive steps, exact mission time, a single final terminal frame, and a valid FNV-1a observation chain.

The canonical nominal stream has 3,134 frames and 1,328,912 bytes. Its CRC-32 is `0xa9b3b94c`, SHA-256 is frozen in `telemetry-reference-v1.json`, and terminal observation checksum is `0x5b7b2419`. An independent Python parser reconstructs these values directly from bytes without using the Rust decoder.

The finite rust-mos codec probe is 16,778 bytes in the size-optimized profile and signature `0x07bc3e16`. It serializes and parses one header and one initial frame; it does not start a target mission.