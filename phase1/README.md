# Phase 1: vertical-flight laboratory

Phase 1 now has its production numeric layer and versioned scenario-ingestion boundary. Vehicle dynamics, atmosphere evaluation, telemetry presentation, and C64 UI are not implemented yet.

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

To regenerate the Rust fixture binding deliberately:

    python -B phase1/reference/emit_numeric_bindings.py

Generated changes must be reviewed and committed with their SHA-256 digest.

## Next slice

Add generated production bindings for `earth.simple-atmosphere.v1`, then initialize an immutable vertical truth state from a validated `Scenario`. Force evaluation and time integration remain the following slice.
