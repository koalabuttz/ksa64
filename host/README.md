# KSA64 host telemetry tools

The host crate is a `std` adapter around the portable `ksa64-core`; it does not contain a second simulator. It captures records through the same `TelemetrySink` boundary used by C64 targets, then inspects the canonical binary stream with the core decoder.

## Capture and inspect

From the project root:

    cargo run -p ksa64-host -- capture target/phase1-vertical.kst
    cargo run -p ksa64-host -- inspect target/phase1-vertical.kst

`capture` executes the checked golden mission, writes one 32-byte header and 257 40-byte frames, then immediately reads and validates the resulting file. `inspect` performs the same validation without running the mission.

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
