# KST3 telemetry and KRP3 replay

KST3 is the canonical Phase 3 evidence stream. It is allocation-free at the
simulation boundary and contains a 64-byte header followed by fixed 160-byte
little-endian records. The default cadence is every eight physics steps (1 Hz),
plus discrete world/mode transitions and one terminal record.

The header binds the stream to:

- telemetry contract `0x03000001`
- exact KSA-2A scenario identity and content CRC
- exact KSC3 content CRC, mission case, and seed
- Q16 timestep, mission length, record length, and cadence

Each record carries:

- truth radius, downrange, velocities/angular momentum, acceleration, mass,
  propellant, stage, pitch, Mach, dynamic pressure, and events
- imperfect sensor measurements, validity, onboard time, and sensor-frame CRC
- navigation state, flight mode, alarms, and requested actuator/engine commands
- applied steering feedback
- separate rolling truth, sensor, navigation, and flight-software checksums
- record flags, reserved bytes, and a record CRC

The host inspector validates framing, header/config identity, all CRCs and
reserved bytes, monotonic step/time, cadence, event/terminal semantics, sensor
projection, and engine/stage consistency. Errors include the first divergent
record index.

KRP3 is a compact 24-byte-per-record replay view used by the C64 presentation
layer. It contains altitude, downrange, applied pitch, mode, stage, events, and
alarms. There is intentionally no unchecked KRP3 generator: the public derivation
entry point first performs full KST3 inspection and binds the replay header to
the source stream CRC.

`phase3/reference/verify_missions.py` independently parses every frozen KST3
record using Python's binary and CRC implementations, recomputes float64 orbital
elements and loads, checks navigation error, and propagates the post-cutoff coast
with an independent float64 two-body integrator. The generated
`mission-reference-v1.json` must report every acceptance item as true.

## Frozen sizes

| Case | KST3 records | KST3 bytes | KRP3 bytes |
|---|---:|---:|---:|
| nominal | 906 | 145,024 | 21,776 |
| altimeter dropout | 906 | 145,024 | 21,776 |
| GPS outage | 906 | 145,024 | 21,776 |
| steering stuck | 590 | 94,464 | 14,192 |