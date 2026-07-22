# Deterministic scenario and telemetry formats

This document defines the Phase 1 data boundary. Human-authored configuration is separate from the compact C64 representation, and telemetry carries exact raw integers so host/C64 comparison never depends on decimal formatting.

## Common rules

- Multibyte integers are little-endian.
- Signed fields are two's-complement.
- Numeric scales come from `ksa64.numeric.phase1-v1` in `phase0/numeric/FOUNDATION.md`.
- Reserved fields and bits are zero when written and ignored when read.
- Text is UTF-8 with LF line endings.
- CRC-32 uses the IEEE reflected polynomial, initial value `0xffffffff`, and final XOR `0xffffffff`, as produced by Python `binascii.crc32`.
- Stable 32-bit identifiers use FNV-1a over the UTF-8 identifier. A host packer must reject collisions within one build.

Any incompatible layout or meaning requires a version increment. Readers reject unknown versions rather than guessing.

## Scenario source

Humans edit JSON conforming to `phase0/numeric/scenario-v1.schema.json`. Physical decimal values are JSON strings, not binary floating-point numbers. This preserves the exact source spelling and allows one declared rounding rule during packing.

The host-side packer will:

1. Validate the JSON structure and identifiers.
2. Parse decimal strings exactly.
3. Validate field and coupled numeric ranges.
4. Round to raw fixed point using nearest, halves away from zero.
5. Resolve environment and table identifiers.
6. Emit a fixed scenario image and generated Rust constants.

The C64 consumes the packed image or compiled constants. It does not parse JSON.

### Scenario image v1

Magic: ASCII `KSC1`. Total length: 76 bytes.

| Offset | Size | Type | Field |
|---:|---:|---|---|
| 0 | 4 | bytes | Magic `KSC1` |
| 4 | 2 | u16 | Schema version, 1 |
| 6 | 2 | u16 | Record length, 76 |
| 8 | 4 | u32 | Numeric-contract ID |
| 12 | 4 | u32 | Scenario ID |
| 16 | 4 | i32 | Timestep Q16.16 s |
| 20 | 4 | u32 | Physics-step count |
| 24 | 2 | u16 | Telemetry stride in physics steps |
| 26 | 2 | u16 | Flags, zero in v1 |
| 28 | 4 | u32 | Deterministic random seed |
| 32 | 4 | i32 | Initial altitude Q20.12 km |
| 36 | 4 | i32 | Initial velocity Q8.24 km/s |
| 40 | 4 | i32 | Initial total mass Q20.12 t |
| 44 | 4 | i32 | Initial propellant Q20.12 t |
| 48 | 4 | i32 | Dry mass Q20.12 t |
| 52 | 4 | i32 | Thrust Q20.12 MN |
| 56 | 4 | i32 | Mass flow Q16.16 t/s |
| 60 | 4 | i32 | Burn duration Q16.16 s |
| 64 | 4 | i32 | CdA Q16.16 m^2 |
| 68 | 4 | u32 | Environment-table ID |
| 72 | 4 | u32 | CRC-32 of bytes 0 through 71 |

The packed record is accepted only when inert mass (`total mass - propellant`) is at least dry mass, burn duration does not exceed scenario duration, and burn duration is an exact integer multiple of the physics timestep. Step alignment keeps the constant-thrust engine state unambiguous without introducing partial-step event integration.

The fixed record is intentionally narrow. A future variable engine curve, event list, or failure schedule belongs in a new version or separately identified data blocks rather than ambiguous trailing bytes.

## Telemetry binary stream

Binary telemetry is the canonical regression and replay representation. A stream begins with one 32-byte header followed by zero or more 40-byte frames. A truncated header/frame or failed CRC makes the stream invalid.

### Stream header v1

Magic: ASCII `KST1`.

| Offset | Size | Type | Field |
|---:|---:|---|---|
| 0 | 4 | bytes | Magic `KST1` |
| 4 | 2 | u16 | Schema version, 1 |
| 6 | 2 | u16 | Header length, 32 |
| 8 | 2 | u16 | Frame length, 40 |
| 10 | 2 | u16 | Flags, zero in v1 |
| 12 | 4 | u32 | Numeric-contract ID |
| 16 | 4 | u32 | Scenario ID |
| 20 | 4 | i32 | Physics timestep Q16.16 s |
| 24 | 2 | u16 | Telemetry stride in physics steps |
| 26 | 2 | u16 | Reserved, zero |
| 28 | 4 | u32 | CRC-32 of bytes 0 through 27 |

### Frame v1

| Offset | Size | Type | Field |
|---:|---:|---|---|
| 0 | 4 | u32 | Completed physics-step index |
| 4 | 4 | i32 | Mission time Q16.16 s |
| 8 | 4 | i32 | Altitude Q20.12 km |
| 12 | 4 | i32 | Velocity Q8.24 km/s |
| 16 | 4 | i32 | Acceleration Q4.28 km/s^2 |
| 20 | 4 | i32 | Total mass Q20.12 t |
| 24 | 4 | i32 | Propellant Q20.12 t |
| 28 | 2 | u16 | Status flags |
| 30 | 2 | u16 | Event flags since the prior frame |
| 32 | 4 | u32 | Rolling exact-state FNV-1a checksum |
| 36 | 4 | u32 | CRC-32 of bytes 0 through 35 |

Status bit 0 means engine active. Event bit 0 means engine cutoff, bit 1 means propellant depleted, bit 2 means numeric fault, and bit 3 means end of run. All other v1 bits are reserved.

The rolling checksum covers the canonical exact state after every physics step, not just emitted frames. Phase 1 fixes its precise field order alongside the state structure before implementation. The frame CRC protects framing and storage; the rolling checksum identifies the first deterministic state divergence. They serve different purposes.

## Host CSV view

CSV is a presentation/export format, not the canonical replay artifact. It uses ASCII/UTF-8, LF, a comma delimiter, no locale grouping, and exact raw integer columns:

    step,time_q16,altitude_q12,velocity_q24,acceleration_q28,mass_q12,propellant_q12,status_flags,event_flags,state_checksum

Optional interpreted columns may follow, but regression tools compare raw columns. The header is mandatory and column order is versioned with the exporter.

## Golden fixtures

`phase0/reference/generate_numeric_foundation.py` packs the checked-in example scenario and a two-frame telemetry fixture. It records complete hexadecimal encodings, CRCs, lengths, and SHA-256 digests in `phase0/numeric/numeric-v1.json`.

The fixtures verify a future Rust packer/reader independently: matching a structure in memory is not enough; the emitted bytes must match exactly.
