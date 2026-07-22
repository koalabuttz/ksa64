# Phase 1: vertical-flight laboratory

Phase 1 has an end-to-end deterministic vertical mission executor, a passing raw physics budget, canonical stride-aware telemetry emission, target-visible telemetry timing, host capture/inspection, a verified C64 post-run display, and independent high-precision accumulated-error evidence: validated scenario ingestion, generated Earth environment, immutable truth, pure force evaluation, checked semi-implicit-Euler transitions, fail-closed execution, exact summaries, allocation-free binary serialization and decoding, event accumulation, caller-provided sinks, strict stream validation, and compact host/C64 text views.

## Current slice

The `ksa64-core` crate provides:

- `no_std` host/MOS-compatible source.
- Strong one-word types for every accepted Phase 1 Q format.
- Explicit two-word scaled multiplication and division.
- Nearest rounding with exact halves away from zero.
- Checked addition, subtraction, and clamped interpolation.
- Sticky fault bits for saturation, division by zero, invalid shifts, and invalid input.
- Generated arithmetic, interpolation, constant-motion, acceleration, convergence, and mass-flow fixtures.
- A fail-closed parser for the 76-byte scenario image, including CRC, identity, range, mass, duration, and acceleration-envelope checks.
- Generated production bindings for the accepted `earth.simple-atmosphere.v1` density and gravity tables.
- Typed, clamped environment sampling through the production interpolation primitive.
- An immutable 28-byte initial vertical truth state that can only be constructed from a validated Scenario.
- Pure typed evaluation of thrust, weight, signed drag, net force, and acceleration without truth mutation.
- Generated exact cases for powered flight, bidirectional drag, both cutoff conditions, and numeric-envelope containment.
- A fail-closed single-step transition with semi-implicit Euler, bounded propellant consumption, exact cutoff events, and immutable successor truth.
- Generated transition cases covering ordinary motion, final partial consumption, burn-boundary cutoff, coast, and refused faults.
- Deterministic execution to the scenario step limit with a compact final truth, cutoff count, and rolling exact-state FNV-1a checksum.
- A failure result that preserves the last valid truth, checksum, cutoff count, numeric status, and cause.
- Native, `mos-sim-none`, and `mos-c64-none` build paths.
- Separate checked-dynamics and rolling-validation paths generated from one executor for controlled timing.
- A three-run PAL VICE common-clock measurement with target-visible CIA timing.
- Exact-length writers for canonical 32-byte telemetry headers and 40-byte frames.
- Private status/event bit types, explicit little-endian fields, rolling-state checksum storage, and per-record CRC-32.
- Byte-for-byte native, rust-mos, and C64 self-tests against the independent 112-byte record fixture.
- One observer-enabled checked executor shared by dynamics-only, checksum, and telemetry paths.
- Initial, configured-stride, final off-stride, and terminal numeric-fault emission through a caller-provided allocation-free sink.
- Sticky cutoff and depletion events that clear only after a sink accepts their frame.
- Explicit preservation of simulation and sink failures, including simultaneous fault-reporting failures.
- An independently generated 257-frame, 10,312-byte mission-stream oracle with CRC-32 `0xcf56fe65`.
- A three-path PAL timing harness measuring raw dynamics, per-successor checksumming, and checksum plus canonical telemetry.
- Three stable telemetry runs at 172,152.59 cycles per physics step (5.72 Hz), with telemetry itself adding 7,475.28 cycles per step or 59,569.54 cycles per emitted frame.
- Allocation-free portable decoders for canonical headers and frames, including exact schema, identity, reserved-bit, and CRC validation.
- A `std` host writer sink and inspector that enforce initial-state, order, stride, time, fault, and terminal-stream semantics.
- A host command that captures or inspects `.kst` files and renders interpreted final-state telemetry without using decimal values for validation.
- A constant-memory C64 sink that retains one header, the latest frame, frame count, and accumulated event bits.
- A direct 40x25 post-run status page verified from actual VIC-II screen memory under PAL VICE.
- Explicit separation of display work from the accepted raw and recorded timing regions; the status PRG is 28,149 bytes.
- An 80-digit Decimal comparison separating fixed-point/table error from integrator error and confirming RK4 convergence.
- Generated C64 presentation constants for the final fixed-minus-confirmed-RK4 deltas: -279.355 m altitude and -2.857 m/s velocity.

The production core executes the complete golden 2,048-step mission and matches an independently generated final state and checksum. Exact interpolation and acceleration-division fast paths reduce checked dynamics to 114,981.59 PAL cycles per step, clearing the raw PAL 8 Hz budget with 6.64 percent headroom. Its telemetry stream scheduler is production code; host storage and C64 presentation remain adapters outside the physics core. The diagnostic C64 self-test is 49,468 bytes with the C64 adapter module available, while the standalone post-run status PRG with accumulated-error reporting is 28,149 bytes.

## Golden mission result

- Completed steps: `2048`.
- Mission time Q16.16: `16777216` (256 seconds).
- Final altitude Q20.12: `1555457`.
- Final velocity Q8.24: `31437299`.
- Final acceleration Q4.28: `-2346189`.
- Final mass/propellant Q20.12: `491520` / `0`.
- Engine-cutoff events: `1`.
- Rolling exact-state checksum: `0x72bf6e0e`.
- Fixed minus same-step Decimal: +7.842186 m altitude, +0.042079 m/s velocity.
- Fixed minus confirmed RK4: -279.354992 m altitude, -2.856721 m/s velocity.

## Layout

    core/
        Cargo.toml
        src/
            lib.rs
            numeric.rs
            quantities.rs
            scenario.rs
            environment.rs
            dynamics.rs
            vehicle.rs
            self_test.rs
            telemetry.rs
            bin/
        tests/
    host/
        Cargo.toml
        README.md
        src/
        tests/
    phase1/
        README.md
        check.ps1
        timing.ps1
        telemetry_timing.ps1
        high_precision.ps1
        status_display.ps1
        telemetry-timing-v1.json
        high-precision-v1.json
        HIGH-PRECISION.md
        generated/
        reference/

## Reproduce

From the project root:

    .\phase1\check.ps1
    .\phase1\timing.ps1
    .\phase1\telemetry_timing.ps1
    .\phase1\high_precision.ps1
    .\phase1\status_display.ps1
    cargo run -p ksa64-host -- capture target/phase1-vertical.kst
    cargo run -p ksa64-host -- inspect target/phase1-vertical.kst

The first command verifies the accepted artifacts, runs native tests, executes the exact fixture pack through rust-mos, and builds both physical-C64 PRGs. The second preserves the dedicated raw/checksum timing gate. The third measures raw, checksum, and canonical telemetry paths together and requires three stable runs under the pinned PAL VICE common clock. The fourth recomputes and verifies the independent Decimal/RK4 comparison. The fifth runs the complete mission and verifies the finished PETSCII page by reading C64 screen memory from VICE.

To regenerate the Rust bindings deliberately:

    python -B phase1/reference/emit_numeric_bindings.py
    python -B phase1/reference/emit_environment_bindings.py
    python -B phase1/reference/emit_force_bindings.py
    python -B phase1/reference/emit_transition_bindings.py
    python -B phase1/reference/emit_mission_bindings.py
    python -B phase1/reference/generate_high_precision.py

Generated changes must be reviewed and committed with their SHA-256 digest.

## Next slice

Audit every Phase 1 exit criterion, rerun the complete correctness, timing, high-precision, host-capture, and C64-display gates, then freeze the phase-completion record. New dynamics belong to Phase 2.
