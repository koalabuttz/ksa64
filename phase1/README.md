# Phase 1: vertical-flight laboratory

Phase 1 now has its production numeric layer, versioned scenario-ingestion boundary, generated Earth environment, immutable vertical truth, pure force evaluation, and one checked semi-implicit-Euler transition. Mission execution, telemetry presentation, and C64 UI are not implemented yet.

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
- Native, `mos-sim-none`, and `mos-c64-none` build paths.

The production core can now advance exactly one checked physics step. It deliberately has no mission run loop, telemetry writer, or presentation layer yet.

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

This verifies the accepted numeric artifacts, regenerates nothing, runs native tests, executes the same exact fixture pack through rust-mos, and builds the physical-C64 PRG.

To regenerate the Rust bindings deliberately:

    python -B phase1/reference/emit_numeric_bindings.py
    python -B phase1/reference/emit_environment_bindings.py
    python -B phase1/reference/emit_force_bindings.py
    python -B phase1/reference/emit_transition_bindings.py

Generated changes must be reviewed and committed with their SHA-256 digest.

## Next slice

Add a deterministic mission executor that repeatedly applies the checked transition up to the validated scenario step count, stops on the first fault, and returns a compact final summary and exact-state checksum. Keep telemetry serialization and UI outside this slice.
