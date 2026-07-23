# KSA64 host telemetry tools

The host crate is a `std` adapter around the portable `ksa64-core`; it does not contain a second simulator. It captures records through the same `TelemetrySink` boundary used by C64 targets, then inspects the canonical binary stream with the core decoder.

## Capture and inspect

From the project root:

    cargo run -p ksa64-host -- capture target/phase1-vertical.kst
    cargo run -p ksa64-host -- inspect target/phase1-vertical.kst
    cargo run -p ksa64-host -- phase2-capture target/phase2-ascent.kst2
    cargo run -p ksa64-host -- phase2-inspect target/phase2-ascent.kst2

`capture` executes the checked Phase 1 mission, writes one 32-byte header and 257 40-byte frames, then immediately reads and validates the resulting file. `inspect` performs the same validation without running the mission.

`phase2-capture` executes KSA-2A, writes one 40-byte header and 901 64-byte frames, and strictly validates the 57,704-byte result. `phase2-inspect` validates an existing canonical `KST2` stream without rerunning physics.

The compact text view reports scenario identity, timestep and stride, stream length and CRC, final physical state, rolling state checksum, and frames carrying cutoff, depletion, or numeric-fault events. Decimal values are presentation only; validation uses canonical raw integers.

## Strict validation

Inspection rejects:

- truncated headers, partial frames, and trailing bytes;
- unknown versions, lengths, numeric contracts, reserved fields, status bits, or event bits;
- header or frame CRC failures;
- a stream bound to a different scenario, timestep, or telemetry stride;
- an initial frame that differs from the validated scenario;
- non-monotonic steps, nonterminal off-stride frames, or inconsistent mission time;
- numeric-fault frames without end-of-run;
- terminal frames before the end of the file or a missing terminal frame;
- successful terminal frames that do not reach the scenario step limit.

The writer adapter propagates I/O errors through the existing mission failure type. File format, scheduling, dynamics, and checksumming remain owned by `ksa64-core`.

## Phase 3 host validation

Phase 3 host support lives in `ksa64_host::phase3`. It captures each closed-loop case through the canonical KST3 sink, strictly inspects an existing stream against the unchanged KSC2 scenario plus its exact KSC3 image, and derives KRP3 only from an accepted stream. The library reports the first bad frame and rejects framing, identity, CRC, reserved-field, cadence, time, terminal, sensor-projection, and engine/phase inconsistencies.

The checked example regenerates all four reviewed case sets during development:

    cargo run -p ksa64-host --example generate_phase3_artifacts

Normal completion uses `phase3/check.ps1`, which validates frozen artifact hashes and tests inspection/derivation without silently updating golden files. Independent physical acceptance comes from `phase3/reference/verify_missions.py`; it parses KST3 separately rather than treating the host inspector as its oracle.
