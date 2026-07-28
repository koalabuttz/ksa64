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

## KST5 spatial mission telemetry

KST5 is the canonical Phase 5 integrated-mission stream. Its 96-byte header binds the numeric/scenario/environment identities, reviewed component signatures, case, seed, 0.125-second timestep, and mission limit. Each 424-byte mission-cadence frame carries committed spatial truth, rigid and flexible state, stage/actuator/load state, the complete CRC-protected spatial sensor and command records, aided navigation, checksum chains, reserved bytes, and a frame CRC.

Frame zero is initial truth. Successor step N contains the sequence N-1 measurement and command that caused it. Strict readers require consecutive steps and time, nested record validity, known masks/enums, zero reserved bytes, an exact rolling observation chain, and exactly one terminal final frame. The checked nominal evidence is 3,134 frames, 1,328,912 bytes, and CRC-32 `0xa9b3b94c`; `phase5/reference/verify_telemetry.py` parses it independently. See `phase5/TELEMETRY.md` for the field contract.
## Phase 5 spatial campaign formats

Gate 10 preserves every Phase 3/4 artifact and adds two fixed-width families for KSA-5A campaigns. Both use little-endian integers, require zero reserved bytes, and reject unknown versions, identifiers, enums, lengths, or CRCs.

### KSC5 campaign configuration

`KSC5` is exactly 704 bytes and can contain at most 24 distribution records.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | Magic `KSC5` |
| 4 | 2 | Version, 5 |
| 6 | 2 | Record length, 704 |
| 8 | 4 | Contract ID `0x050a0000` |
| 12 | 4 | Phase 5 numeric-contract ID |
| 16 | 4 | Phase 5 scenario ID |
| 20 | 4 | Master seed |
| 24 | 4 | Run count |
| 28 | 1 | Active distribution count |
| 29 | 1 | Parameter count |
| 30 | 1 | Distribution capacity, 24 |
| 31 | 89 | Reserved, zero |
| 120 | 4 | CRC-32 of bytes 128 through 703 |
| 124 | 4 | CRC-32 of bytes 0 through 123 |
| 128 | 576 | Twenty-four 24-byte distribution records |

Each distribution record stores parameter, family, correlation group, one reserved zero byte, minimum, baseline, maximum, shape, and a CRC-32 over its first 20 bytes. Unused records are all zero. Run zero bypasses sampling and is the exact frozen nominal mission.

### KSR5 run summary

`KSR5` is exactly 160 bytes.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | Magic `KSR5` |
| 4 | 2 | Version, 5 |
| 6 | 2 | Record length, 160 |
| 8 | 4 | Contract ID `0x050a0001` |
| 12 | 4 | Campaign seed |
| 16 | 4 | Run index |
| 20 | 4 | Derived sensor seed |
| 24 | 4 | Variation checksum |
| 28 | 1 | Mission outcome |
| 29 | 1 | Mission case |
| 30 | 2 | Reserved, zero |
| 32 | 4 | Completed steps |
| 36 | 12 | Terminal ECI position Q20.12 km |
| 48 | 12 | Terminal ECI velocity Q8.24 km/s |
| 60 | 4 | Fixed-point perigee altitude Q20.12 km |
| 64 | 4 | Fixed-point apogee altitude Q20.12 km |
| 68 | 2 | Inclination turn16 |
| 70 | 2 | Event mask |
| 72 | 4 | Maximum dynamic pressure Q16.16 kPa |
| 76 | 4 | Maximum angle-of-attack sine Q16.16 |
| 80 | 4 | Maximum flexible-state magnitude Q8.24 |
| 84 | 4 | Maximum navigation-position error Q20.12 km |
| 88 | 16 | Sensor, navigation, flight, and summary checksums |
| 104 | 52 | Reserved, zero |
| 156 | 4 | CRC-32 of bytes 0 through 155 |

The ordered campaign chain applies FNV-1a to complete KSR5 records in run-index order. Native workers may execute out of order, but they may not merge out of order. The independent float64 analyzer reconstructs variation from KSC5 and uses raw terminal vectors from KSR5; compact fixed-point orbital fields remain selection and presentation aids.

## Phase 5 adaptive-history formats

KPH5 is a presentation-only compact trajectory. Its 80-byte header uses magic `KPH5`, version 5, contract `0x050c0001`, Phase 5 numeric/scenario identities, campaign/run/seed/variation identity, stride, point count, terminal step, payload CRC-32, reserved-zero bytes, and header CRC-32. Its 16-byte points store mission step, three signed quarter-kilometre ECI coordinates, dynamic pressure in one-sixteenth kPa, conservative quarter-kilometre navigation error, events, and alarms.

KRA5 is the append-only Phase 5 archive, with magic `KRA5`, version 5, and contract `0x050c0002`. It retains the KRA4 commit discipline but never accepts a KRA4 superblock. Complete KST5 payloads are embedded unchanged; compact histories remain complete KPH5 streams.

KPH5 is also the direct Phase 5 stock replay tape. Replay adds no new binary format: the portable reader revalidates KPH5 and derives a presentation summary and cue hash in constant memory. The Y–Z projection and SID cues are target presentation outputs and carry no canonical physics authority.


## Phase 6 link formats

Phase 6 preserves every earlier artifact and adds wire records rather than another canonical telemetry stream. KLF6 is a COBS-delimited frame with a 36-byte decoded header, at most 512 payload bytes, CRC-32, session/sequence/acknowledgement identity, and explicit measurement, production, and effective epochs. The maximum encoded frame is 556 bytes. Strict readers reject malformed COBS, bad length or CRC, unknown types or required flags, nonzero reserved fields, identity mismatch, and impossible epoch relationships.

KSA-6R uses fixed CRC-16-CCITT cells: 40-byte inertial, 64-byte aid, 24-byte command, and 48-byte status records. A raw endpoint publishes the four-byte readiness preamble `d6 5a 06 00`. Commands identify their source epoch and following effective epoch; aid and status use the 8 Hz navigation cadence. The terminal inertial flag is set only after the final world commit, allowing the flight endpoint to return final command/status evidence before it stops.

KLF6 transcripts and KLR6 cells are transport evidence, not replacements for KST5. Full layouts, masks, failure rules, and accepted checksums are frozen in `phase6/CONTRACT.md` and `phase6/REALTIME.md`.


### KMR6 Mission Control sessions

KMR6 is a host-only, noncanonical presentation/session container. Its 32-byte header identifies version 1 and carries a header CRC-32. Every following record has a type, bounded little-endian length, compact binary payload, and independent CRC-32. Update records preserve the validated KLR6 cells plus passive ground estimates and host presentation fields; a terminal evidence record marks a complete run. Readers stop at the first truncated, oversized, unknown, or corrupt record and expose all earlier records as a recovered partial session. KMR6 never replaces KST5, KLF6, KLR6, or target acceptance evidence. CSV and JSON are explicitly derived presentation exports.

### Phase 6 visualization derivation

The expanded Mission Control plots introduce no format revision. The planned ascent is the strictly validated frozen run-zero KPH5 artifact; the nominal orbit target comes from its accepted reference record. Live and replayed onboard, ground, and SIM Director inputs already exist in `MissionControlUpdate`/KMR6. Osculating-orbit, ground-track, environment, and residual products are regenerated presentation aids and are never serialized as canonical flight evidence.

KMR6 version 1 remains sufficient because each recorded update already preserves the ordered inputs needed to rebuild every plot. A replay renderer may use only records at or before its selected cursor. Plot glyph mode, terminal size, focused trajectory view, and path emphasis are transient UI preferences and are not stored in KMR6.
## Phase 7 multi-profile formats

Phase 7 preserves all earlier families and adds strict version-7 records under
the hobby numeric-contract identity `0xee0448fa`:

- KVP7 is a fixed 512-byte vertical vehicle pack.
- KMP7 is a fixed 896-byte motor pack with at most 64 sampled thrust knots.
- KMC7 is a fixed 256-byte mission/environment/launch/recovery pack.
- KST7 uses a 96-byte header and 96-byte canonical telemetry frames.
- KSR7 is a fixed 192-byte profile-neutral evaluation summary with a metric
  validity mask.
- KSC7 is a fixed 512-byte keyed uncertainty campaign.
- KCL7 is a bounded ordered candidate-list manifest.
- KPH7 uses a 64-byte header and 16-byte sparse plot points; it is
  noncanonical presentation evidence.
- KRA7 uses a strict 64-byte archive header followed by ordered, independently
  CRC-protected KSR7 records.

Every common header binds kind, length, numeric contract, identity, reserved
zero bytes, and CRC-32. The host compiler consumes JSON/RASP source data; no
human-readable format is accepted by a C64 evaluator.

## Phase 8 spatial formats

Phase 8 preserves every earlier format and introduces a strict version-8 family bound to `HobbySpatialV1`:

- KVP8: 1,024-byte derived spatial vehicle and aerodynamic pack.
- KMP8: 1,024-byte sampled motor and propellant-dependent mass-properties pack.
- KMC8: 512-byte launch, mission, recovery, and environment pack.
- KWP8: 512-byte layered-wind and deterministic-gust pack.
- KST8: 128-byte header followed by 160-byte canonical telemetry frames.
- KSR8: 256-byte evaluation summary with 32 metric slots and spatial validity flags.
- KPH8: 64-byte header followed by 24-byte noncanonical spatial plot points.
- KSC8: 512-byte deterministic campaign configuration.
- KRA8: append-only campaign archive of independently protected summaries.

All multibyte fields are little-endian. Headers bind kind, version, profile/numeric identity, logical identity, payload size, reserved-zero regions, and CRC-32. Parsers reject unknown flags, nonzero reserved bytes, mismatched identities, truncation, trailing bytes where forbidden, and the first corrupt record. KSR7 keeps its original 24 metric slots and rejects Phase 8 profiles or flags; it was not widened in place.

## Phase 8.5 avionics formats

Phase 8.5 is additive to KVP8-KRA8:

- KAP8 is a fixed 512-byte avionics profile pack.
- KAC8 is a fixed 256-byte actuator capability and installation pack.
- KLE8 is a fixed 128-byte avionics-aware evaluation request.
- KLR8 is the local-ENU raw-cell family: 40-byte inertial, 24-byte command, 64-byte aid, and 48-byte status records.
- KAT8 is 256-byte canonical avionics-aware telemetry.
- KAS8 is a fixed 256-byte avionics evaluation summary wrapping the unchanged physical summary.
- KMR8 is a host-only noncanonical Mission Control recording.

KLF6 remains the outer split-endpoint transport. KLR8's distinct contract identity prevents it from being parsed as KLR6 despite the intentionally reused bounded cell lengths. Every canonical format is little-endian, identity-bound, CRC-protected, and reserved-zero strict. KMR8 is passive presentation evidence and cannot replace canonical telemetry.

## Phase 9 optimization formats

Phase 9 adds strict little-endian, CRC-protected optimization records while reusing KAS8/KAT8/KST8/KPH8 for flight evidence:

- KOM9 is a fixed 2,048-byte compiled search manifest with at most 32 variables, eight objectives, and sixteen constraints.
- KDV9 is a fixed 256-byte canonical design vector bound to its KOM9 identity.
- KOE9 is a fixed 512-byte robust candidate aggregate containing exact objective/constraint values, tier, feasibility, violations, and identity.
- KRA9 is a segmented append-only search archive. Each complete generation segment carries design vectors and aggregates plus independently protected KRE9 retained-evidence records containing strict KAS8 cases.
- KPF9 is the bounded ordered finalist package consumed by host and stock-C64 browsers.
- KSN9 contains fixed sensitivity records for baseline/finalist one-quantum derivatives.

KRA9 readers reject truncation, trailing data, bad CRCs, identity or tier mismatch, malformed embedded KDV9/KOE9/KAS8 records, and incomplete generation boundaries. Resume accepts only a byte-exact complete-segment prefix. JSON manifests, JSONL optimizer messages, CSV, HTML, and report JSON are host interfaces or derived presentation—not canonical simulation formats.

## Phase 9.5 advanced-effector formats

Phase 9.5 leaves every Phase 8.5 and Phase 9 record unchanged and adds:

- KPE9: 2,048-byte canard, RCS, tank, supply, failure, and authority installation pack, including four fixed Q24 hinge-load limits.
- KPA9: 512-byte PriorityResidualV1 allocator and compiled mixing pack.
- KLE9: 256-byte advanced-effector evaluation request.
- KLR9: 64-byte fast-sensor, 64-byte command, 64-byte aid, and 80-byte status cells.
- KAT9: 128-byte header followed by 320-byte advanced telemetry frames.
- KAS9: 512-byte advanced evaluation summary.
- KSC9: 512-byte deterministic advanced campaign configuration.
- KAE9: segmented campaign/search archive containing KAS9 evidence.
- KFE9: bounded host/C64 finalist package.
- KFB9: 352-byte, CRC-protected selected-finalist flight bootstrap carried only in the KLF6 Start payload. It binds manifest, study, candidate, vehicle, effector, and allocator identities and carries the bounded flight/allocator configuration required by the separate stock endpoint. KFB9 does not replace KPE9/KPA9 or become physical evidence.
- KMR9: noncanonical host Mission Control recording of passive Phase 9.5 presentation updates and terminal checksum chains. It is replay/presentation data and never enters candidate or evaluator identity.

KLR9 uses CRC-16-CCITT and a distinct sync/version prefix; KLF6 remains its outer transport. The fixed packs use strict version-9 headers, little-endian fields, identity binding, reserved-zero enforcement, and CRC-32. A pulse quantum is exactly 1/256 second (1,024 Q18 units), and a command carries zero through eight quanta for each of twelve jets.

## Phase 10 global formats

Phase 10 preserves every earlier family and adds:

- KEM10: 512-byte Earth/time/gravity/source-policy pack.
- KFT10: bounded 128-byte header plus 48-byte transform knots.
- KAT10: bounded 128-byte header plus 40-byte atmosphere/wind knots.
- KGV10: 2,048-byte global vehicle/propulsion/aero/recovery pack.
- KGM10: 1,024-byte mission/guidance/anchor/transition pack.
- KLR10: 64-byte fast sensor and command cells, 96-byte aid/frame and status cells, and a 192-byte transition cell carried inside KLF6.
- KTT10: 128-byte header followed by 256-byte canonical telemetry frames.
- KSR10: 512-byte global evaluation summary.
- KPH10: 64-byte header followed by 48-byte compact ground-track/altitude points.
- KSC10: 512-byte deterministic campaign configuration.
- KRA10: 128-byte header, embedded KSC10, ordered protected KSR10 records, and a 32-byte footer.
- KMR10: noncanonical host Mission Control recording.

Every canonical record is little-endian, fixed-capacity, identity-bound, CRC-protected, reserved-zero strict, and corruption/truncation rejecting. KLF6 placement, worker count, presentation, and storage never enter physical identity.

## Phase 11 mission-operations formats

Phase 11 leaves KLR10 and every earlier physical record unchanged. It adds
strict operational envelopes around the accepted global flight ABI:

- KFS11 is a fixed 512-byte flight-software package manifest.
- KMP11 is a fixed 2,048-byte compiled mission plan with at most 24 events,
  eight contingency branches, and eight operator decisions.
- KPX11 is a fixed 512-byte segmented-object envelope carrying at most 480
  payload bytes per KLF6 frame.
- KUL11 is a fixed 512-byte staged uplink load.
- KUA11 is a fixed 128-byte commit, cancellation, or acknowledgement record.
- KAL11 uses a 128-byte header followed by fixed 64-byte action records.
- KPD11 is a fixed 256-byte compact prediction summary.
- KPP11 uses a 128-byte header followed by bounded 32-byte prediction points.
- KGO11 and KGE11 are fixed 128-byte ground-observation and ground-estimate
  records.
- KEJ11 is a fixed 64-byte recoverable flight-package journal record.
- KPC11 is a fixed 4,096-byte host procedure pack with a 128-byte header and
  at most 64 fixed 60-byte steps.
- KDR11 is a fixed 512-byte deterministic debrief summary.
- KSD11 is a fixed 256-byte compiled host session-definition pack.
- KSB11 is a segmented host mission-session archive. Each segment uses a
  64-byte header and four-byte CRC trailer; a completed archive ends with a
  strict 44-byte final manifest.

The flight ABI remains profile-specific: KFS11 declares compatibility with
KLR10 rather than replacing its sensor, command, or status cells. Command loads
are staged and validated without changing active state; only a distinct KUA11
commit may schedule activation. Host source JSON, role-filtered presentation,
HTML reports, and operator hints remain noncanonical.

### Banked stock-C64 packaging records

The banked reference-operations endpoint adds two target-private build and
acceptance records. Neither is a mission or simulation format:

- KSB1 is a three-segment stock-RAM linker bundle containing the low helper,
  main/state, and high helper banks plus their load addresses and lengths. Its
  generated packaging manifest binds the raw bundle and every emitted PRG by
  SHA-256. KSB1 is distinct from the KSB11 mission-session archive.
- KOT1 is a bounded exactness transcript of endpoint requests and expected
  replies. It drives the finite native/C64 comparison and does not replace
  KLR10, KAL11, KEJ11, or canonical telemetry.

The packaging tool validates every segment against the accepted memory map
before emitting load-addressed PRGs and a SHA-256 manifest. The VICE probe
rejects any bundle, transcript, bank guard, code-preservation, or response
mismatch.

## Phase 11.5 product metadata

Phase 11.5 introduces no canonical binary format and changes no existing `K*` record.

`ksa64.product-catalog.v1` is deterministic host product metadata describing current experiences, targets, placements, maturity, limitations, and historical provenance. `ksa64.application-outcome.v1` is structured host command output. Neither participates in simulation, flight, campaign, optimizer, or evidence identity, and neither may replace the strict parser owned by an underlying artifact family.

## Phase 12A viewer-bridge metadata

Phase 12A introduces no canonical `K*` format and changes no existing record.
KSB11 remains the unchanged canonical completed mission-session evidence at the
application boundary.

The viewer-bridge ABI manifest, deterministic product-catalog JSON, typed live
session snapshots/events, queue diagnostics, Unreal automation reports, and
packaging/performance audits are noncanonical host integration metadata. They
bind and verify a presentation client but cannot replace KSB11, enter simulation
identity, or acquire physics, avionics, command, or evidence authority.

## Phase 12B presentation metadata

Phase 12B introduces no canonical `K*` format and does not widen or reinterpret KLR10, KTT10, KPH10, KSR10, KUL11, KUA11, KAL11, KDR11, or KSB11.

`FullMissionGnssLossV1`, `OperationalDispositionViewV1`, bridge feature bits, fixed-layout operational/procedure/action/transport/disposition structures, cursored release and timeline views, prediction-path buffers, presentation text, screenshot baselines, and performance reports are noncanonical host presentation metadata.

Existing ABI-v1 symbols and layouts remain byte-for-byte compatible. The Phase 12B bridge advances its build identity to `0x120B0001` and advertises new functions through feature discovery. The original start function continues to own the compressed Phase 11 compatibility session and unchanged KSB11 output.

Unreal receives typed, role-filtered views and Rust-generated proposal identities. It does not parse canonical KSB11 segments, construct KUL11/KUA11 bytes, or derive missing evidence. Multi-axis outcome classifications are views over accepted records; they do not replace owning physical or operational summaries.

The accepted Phase 12B completion JSON, bridge manifest, automation reports, screenshot/semantic manifests, performance samples, and package audits are noncanonical validation metadata. They bind the qualified ABI-v1 build `0x120B0001` and accepted product evidence without becoming mission inputs or canonical records. The frozen evidence and limitations are indexed by `phase12/PHASE12B_COMPLETION.md`; Phase 12C inherits the same ownership rule.


## Phase 12C GlobalDisplay presentation records

Phase 12C introduces no canonical `K*` record and changes no Phase 0–12B.5
format. KTT10, KPH10, KSR10, and KSB11 remain owned by their strict existing
parsers. `GlobalDisplayV1` records are noncanonical, renderer-neutral
presentation products derived and role-filtered in Rust.

KPS1 remains major/minor 1.0 and retains its 48-byte envelope. Capability bit
`KPS1_CAPABILITY_GLOBAL_DISPLAY_V1 = 1 << 8` gates these additive message
kinds:

| Message | Kind | Direction |
|---|---:|---|
| Global display definition | `0x0110` | authority to client |
| Exact sample batch | `0x0111` | authority to client |
| Path chunk | `0x0112` | authority to client |
| Frame/segment transition | `0x0113` | authority to client |
| Replay index | `0x0114` | authority to client |
| Cursor state | `0x0115` | authority to client |
| Exact range request | `0x0210` | client to authority |

Clients that do not negotiate bit 8 never receive or submit these kinds. All
legacy KPS1 vectors, limits, CRC behavior, nonce/sequence rules, cursor gaps,
and correlation rules remain exact. Ordinary records retain the 256-KiB KPS1
payload limit; sample and path queries are additionally bounded by their typed
contracts.

The typed family comprises `GlobalDisplayDefinitionV1`,
`GlobalDisplaySampleV1`, `GlobalDisplaySourcePoseV1`,
`GlobalDisplayPathChunkV1`, `GlobalDisplayTransitionV1`,
`GlobalReplayIndexV1`, `GlobalDisplayCursorStateV1`, and
`GlobalDisplayRangeRequestV1`. They carry fixed-width numeric values and
identities, not canonical record internals. Role filtering removes SIM-truth
source poses and paths before encoding for an unauthorized role.

Path retention uses one shared Rust builder for in-process/WASM and native
bridge clients. It pins sample events and discontinuities plus semantic replay
bookmarks for transitions, declared mission events, procedure actions, faults,
and terminal state. Routine release notifications are not pins. Exact live
sources apply cadence to their zero-based release epochs. The already-sparse
frozen planned source retains its initial point explicitly and applies cadence
to its accepted one-based sequence for subsequent points.

A semantic path record preserves its source/model/estimate identities, source
checksum, continuity identity, anchor, strip, LOD, point count, and unmodified
path flags. Bits 0 through 3 mean stale, incomplete, terminal, and
resynchronization required. Its point checksum starts at FNV-1a offset
`0x811c9dc5`, uses prime `0x01000193`, and processes each point in order as
eight little-endian 32-bit words: release epoch, Q16 mission time, segment
identity, event mask, anchor identity, and signed Q12 X, Y, and Z. This checksum
intentionally binds temporal and event meaning as well as geometry.

Semantic snapshots record the normalized supported view mode. Camera and
display frame are not independent evidence fields: launch/recovery map to the
corresponding local ENU view, Earth-fixed maps to ECEF, inertial/free-orbit map
to GCRF, and chase/inspection map to the authoritative sample frame.

Native ABI-v1 remains unchanged. `ksa64_viewer_global_display_api_v1` returns
an optional, versioned, size-tagged function table for definition, exact sample
ranges, paths, replay indices, and nominal replay startup. Callers must validate
the base bridge manifest and the optional table before use. Old libraries and
manifests remain valid because absence of the optional table means unsupported,
not corrupt.

Browser and Unreal semantic snapshots, screenshots, path-memory reports,
origin-change logs, backend/fallback results, and runtime measurements are
noncanonical validation metadata. The implemented producer and audit schemas
are:

- `ksa64.phase12c.global-display-harness.v1` for the native C++ consumer;
- `ksa64.phase12c.unreal-global-evidence.v1` for packaged Unreal evidence;
- `ksa64.phase12c.browser-evidence.v1` for rendered-browser evidence;
- `ksa64.phase12c.web-source-identity.v1` and
  `ksa64.phase12c.web-distribution-identity.v1` for source and production-build
  binding;
- `ksa64.global-scene-semantic.v1` for renderer-neutral semantic snapshots; and
- `ksa64.phase12c.cross-renderer-evidence.v2` for the strict joined result.

These records are completion-audit inputs only. The v2 cross-renderer record
is generated from and SHA-256-binds the raw native, packaged-Unreal, browser,
semantic, screenshot, bridge, and distribution artifacts. No pending, partial,
or hand-edited record may masquerade as accepted evidence.
