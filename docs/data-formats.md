# Deterministic scenario and telemetry formats

This document indexes the deterministic data boundaries accepted through Phase 4. Human-authored configuration is separate from compact C64 representations, and canonical evidence carries exact raw integers so host/C64 comparison never depends on decimal formatting.

## Common rules

- Multibyte integers are little-endian.
- Signed fields are two's-complement.
- Phase 1 numeric scales come from `ksa64.numeric.phase1-v1` in `phase0/numeric/FOUNDATION.md`.
- Phase 2 numeric scales and bounds come from `ksa64.numeric.phase2-v1` in `phase2/contract-v1.json`.
- Reserved fields and bits are zero when written. The strict v1 reader rejects nonzero reserved values rather than assigning future meaning to an old version.
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

The rolling checksum starts from FNV-1a offset `2166136261` and incorporates each successful successor state, excluding the unadvanced initial state. Every state contributes seven 32-bit words as little-endian bytes in this exact order: completed step index, mission time, altitude, velocity, acceleration, total mass, and propellant. It never hashes Rust memory layout, padding, status flags, or events.

The frame CRC protects framing and storage; the rolling checksum identifies deterministic state divergence. They serve different purposes.

## Strict stream inspection

The portable reader validates canonical records without allocation. It rejects wrong lengths, magic, versions, declared record sizes, numeric-contract identity, reserved values, status/event masks, and record CRCs. A header may additionally be bound to a validated scenario, which requires exact scenario ID, timestep, and telemetry stride.

The host inspector adds stream semantics: an exact scenario-derived initial frame, strictly increasing emitted steps except a repeated last-valid truth on terminal numeric failure, stride alignment for nonterminal successors, `time = step * timestep`, no steps beyond the scenario limit, numeric fault paired with end-of-run, exactly one terminal position at the end of the file, and the declared final step for successful completion. Trailing bytes and partial frames are errors.

These checks establish framing and semantic coherence. A rolling checksum on an emitted frame still represents all successful successors since launch, including states omitted by telemetry stride; a reader cannot independently reconstruct those hidden states from the stream alone. Exact replay verification therefore remains a separate operation that reruns the scenario or compares against an independently produced oracle.

## Host CSV view

CSV is a presentation/export format, not the canonical replay artifact. It uses ASCII/UTF-8, LF, a comma delimiter, no locale grouping, and exact raw integer columns:

    step,time_q16,altitude_q12,velocity_q24,acceleration_q28,mass_q12,propellant_q12,status_flags,event_flags,state_checksum

Optional interpreted columns may follow, but regression tools compare raw columns. The header is mandatory and column order is versioned with the exporter.

## Golden fixtures

`phase0/reference/generate_numeric_foundation.py` packs the checked-in example scenario and a two-frame telemetry fixture. It records complete hexadecimal encodings, CRCs, lengths, and SHA-256 digests in `phase0/numeric/numeric-v1.json`.

The production Rust telemetry writers match this independent fixture byte for byte on native, rust-mos, and C64 targets. The mission emitter writes the initial state, configured-stride states, an off-stride final state when required, and a terminal fault frame at the last valid truth. Cutoff and depletion flags accumulate until a sink accepts a frame; end-of-run accompanies the final frame. The golden mission produces 257 frames and a 10,312-byte stream whose independently generated CRC-32 is `0xcf56fe65`. A future reader must meet the same rule: matching a structure in memory is not enough; emitted or accepted bytes must match exactly.

## Phase 2 planar formats

Phase 2 is deliberately versioned separately from the vertical laboratory. Its checked source is exact decimal JSON; the host generator emits a fixed 884-byte `KSC2` scenario image containing the planar initial state, bounded stage records, pitch knots, aerodynamic tables, model identities, and a final CRC-32. The portable parser validates counts, reserved fields, ranges, mass relationships, stage topology, guidance, event alignment, and table ordering before constructing truth.

The canonical `KST2` stream uses a 40-byte header and 64-byte frames. The header binds the stream to its numeric, environment, scenario, timestep, stride, and mission identities. Frames carry raw radius, downrange, radial velocity, specific angular momentum, mass, propellant, command and stage state, Mach, dynamic pressure, event bits, rolling exact-state checksum, and record CRC. The nominal stream contains 901 frames and 57,704 bytes; strict host validation owns cadence, time, range, event, and terminal semantics.

`KRP2` is a derived C64 presentation index, not canonical telemetry. Its header binds compact plot/event records to the source `KST2` stream CRC, scenario, terminal checksum, accepted Max-Q and orbit. It embeds the canonical KST2 header and final frame so the portable decoder revalidates source identity on target. Its own CRCs and reviewed SHA-256 protect the cold display path. Physics regression and replay truth remain `KST2`.

The exact Phase 2 layouts, field offsets, masks, golden identities, and generation rules live in `phase2/scenario-v2.schema.json`, `phase2/TELEMETRY.md`, `phase2/REPLAY.md`, and their checked generators. An incompatible meaning requires a new magic/version rather than silently extending `KSC2`, `KST2`, or `KRP2`.

## Phase 3 closed-loop formats

Phase 3 adds three stable transport messages and three artifact families. `SensorFrame` is 56 bytes, `ActuatorCommand` is 16 bytes, and `FlightOutput` is 52 bytes. All are fixed-width little-endian records with CRC-32 and fail-closed validation of sizes, enums, flags, reserved fields, checksums, and sequence relationships.

KSC3 is a 96-byte case configuration bound to the unchanged KSC2 base scenario. It identifies the deterministic sensor seed and selected fault schedule while recording both its own content identity and the KSC2 content identity. Embedded trailing CRC fields are excluded when computing the bound content CRC, avoiding self-referential identity.

KST3 has a 64-byte header and 160-byte frames. Its header binds scenario identity, exact KSC3/KSC2 content CRCs, case, seed, timestep, stride, and mission length. Each frame carries the coherent world truth, projected sensor frame, navigation estimate, flight mode and commands, applied steering feedback, events, alarms, four rolling checksum chains, the embedded sensor-frame CRC, and its own record CRC. The host additionally enforces initial projection, cadence, time, terminal placement, event/mode/engine consistency, and exact scenario/config binding, reporting the first bad frame.

KRP3 is a validated presentation index created only after strict KST3 inspection. It binds the source stream CRC, configuration CRC, terminal step and checksums, carries compact plot/event records with individual CRCs, and ends with its own terminal integrity record. C64 replay reparses every record, enforces order and terminal semantics, and accumulates PETSCII/SID presentation cues. KRP3 is not canonical simulation telemetry and cannot replace KST3 for regression.

Exact layouts, masks, identities, and accepted artifacts are documented in `phase3/CONTRACT.md`, `phase3/TELEMETRY.md`, and `phase3/examples/`.

## Phase 4 campaign, archive, and export formats

Phase 4 preserves KSC2, KSC3, KST3, and KRP3 unchanged and adds six separately versioned families:

- `KSC4` is a fixed 512-byte campaign configuration. It binds the exact Phase 2/3 inputs, master seed, run count, and up to sixteen reviewed distribution records.
- `KSR4` is a fixed 128-byte run summary containing run identity, variation checksum, outcome, cutoff/terminal raw state, load extrema, navigation error, inherited checksum chains, and CRC.
- `KPH4` is a presentation-only compact history with a 64-byte header and eight-byte plot points.
- `KST4` is canonical detailed telemetry for one campaign run. Its 96-byte header binds campaign/run/seed/variation identity; its 160-byte frames preserve KST3 frame semantics.
- `KRA4` is an append-only archive with a 256-byte superblock, independently protected 32-byte record headers, committed payloads, and a completion footer.
- `KXV4` is a numbered export volume with a 64-byte header binding archive identity, selection identity, volume order, logical offset, length, and CRC.

Strict readers reject unknown versions and enums, nonzero reserved data, invalid lengths, CRC failures, mismatched identities, incomplete committed records, corrupt archive chains, and missing, duplicate, reordered, or mixed export volumes. KPH4 and the compact C64 outcome classifier exist for bounded presentation and selection; independent float64 analysis remains authoritative for physical campaign acceptance.

Exact layouts and generation rules live in `phase4/CONTRACT.md`, `phase4/FORMATS.md`, `phase4/CAMPAIGNS.md`, and `phase4/EXPORT.md`.
