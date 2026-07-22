# Phase 1: vertical-flight laboratory

Phase 1 has an end-to-end deterministic vertical mission executor, a passing raw physics budget, and canonical stride-aware telemetry emission: validated scenario ingestion, generated Earth environment, immutable truth, pure force evaluation, checked semi-implicit-Euler transitions, fail-closed execution, exact summaries, allocation-free binary serialization, event accumulation, and caller-provided sinks. Telemetry timing and C64 UI are not implemented yet.

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

The production core executes the complete golden 2,048-step mission and matches an independently generated final state and checksum. Exact interpolation and acceleration-division fast paths reduce checked dynamics to 114,981.59 PAL cycles per step, clearing the raw PAL 8 Hz budget with 6.64 percent headroom. Its telemetry stream scheduler is production code; storage and presentation remain separate later boundaries. The diagnostic C64 self-test is 47,447 bytes after adding a compact whole-stream checker rather than embedding the 10,312-byte oracle.

## Golden mission result

- Completed steps: `2048`.
- Mission time Q16.16: `16777216` (256 seconds).
- Final altitude Q20.12: `1555457`.
- Final velocity Q8.24: `31437299`.
- Final acceleration Q4.28: `-2346189`.
- Final mass/propellant Q20.12: `491520` / `0`.
- Engine-cutoff events: `1`.
- Rolling exact-state checksum: `0x72bf6e0e`.

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
    phase1/
        README.md
        check.ps1
        generated/
        reference/

## Reproduce

From the project root:

    .\phase1\check.ps1
    .\phase1\timing.ps1

The first command verifies the accepted artifacts, runs native tests, executes the exact fixture pack through rust-mos, and builds the physical-C64 PRG. The second builds the dedicated timing PRG and requires three stable measurements under the pinned PAL VICE common clock.

To regenerate the Rust bindings deliberately:

    python -B phase1/reference/emit_numeric_bindings.py
    python -B phase1/reference/emit_environment_bindings.py
    python -B phase1/reference/emit_force_bindings.py
    python -B phase1/reference/emit_transition_bindings.py
    python -B phase1/reference/emit_mission_bindings.py

Generated changes must be reviewed and committed with their SHA-256 digest.

## Next slice

Measure checksum plus serialization scheduling separately from raw physics using a discard sink and the established PAL common clock. Record the total and per-step/per-frame costs, determine whether 8 Hz remains feasible with canonical telemetry enabled, and keep transport or display costs outside that measurement.
