# Phase 1: vertical-flight laboratory

Phase 1 now has its production numeric layer, versioned scenario-ingestion boundary, generated Earth-environment bindings, and immutable initial vertical truth state. Force evaluation, state integration, telemetry presentation, and C64 UI are not implemented yet.

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
- An immutable 28-byte initial vertical truth state that can only be constructed from a validated `Scenario`.
- Native, `mos-sim-none`, and `mos-c64-none` build paths.

The analytic integration loops exist only as self-tests of the numeric contract. They are not a vehicle simulator.

## Layout

    core/
        Cargo.toml
        src/
            lib.rs
            numeric.rs
            quantities.rs
            scenario.rs
            environment.rs
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

Generated changes must be reviewed and committed with their SHA-256 digest.

## Next slice

Add a pure vertical force-evaluation boundary that consumes validated vehicle configuration, immutable truth, and an environment sample. It should produce a typed force/acceleration snapshot without mutating truth state. Time integration remains the following slice.
