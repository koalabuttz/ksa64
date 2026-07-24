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

## Phase 4 campaigns and exports

Phase 4 host support executes parameterized closed-loop missions, emits KSC4/KSR4 evidence, folds summaries in run-index order, prepares stock- or REU-neutral archives, and joins strict KXV4 volumes. Parallel workers affect elapsed time only; configuration, summaries, aggregates, archive selection, and output bytes remain deterministic.

The reviewed smoke campaign contains 64 runs. The frozen reference campaign contains 1,024 runs with master seed `0x4b534134`. Its independent analyzer reconstructs every variation without Rust and computes authoritative float64 orbital, load, and navigation results:

    python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json --check

Host archive/export tests build a one-volume stock report and a synthetic three-volume selection, then require the joiner to reject corruption, missing, duplicate, reordered, or mismatched volumes. The actual C64 IEC utility is validated separately by `phase4/export-c64.ps1`.

See `phase4/HOST_ANALYSIS.md`, `phase4/FORMATS.md`, and `phase4/EXPORT.md` for commands, frozen identities, and accepted evidence.


## Phase 6 endpoint broker

Phase 6 host support can run the world broker over TCP while a native or C64 flight endpoint owns guidance and control. The broker reads the KLR6 readiness preamble, streams inertial/aid cells, validates returned command/status cells, and compares every result with a native shadow flight computer. The shadow is an observer only and never supplies commands to the world.

The frozen full target run used the same broker behind the VICE binary-monitor mailbox relay. See `phase6/README.md` and `phase6/COMPLETION.md` for exact evidence and the remaining physical-link boundary.


## Phase 6 deployment launcher

`phase6_launch` runs the host world and native flight endpoint across a localhost TCP seam, with optional passive host Mission Control and fast, 32 Hz wall-paced, or manual-step release. The repository-level `phase6/run.ps1` wrapper adds the host-world/VICE-flight combination while enforcing one-VICE-at-a-time lifecycle rules. See `phase6/LAUNCHER.md`.
