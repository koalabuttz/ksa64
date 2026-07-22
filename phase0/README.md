# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The compiler and arithmetic experiment is complete, and Rust/rust-mos is selected:

- Both candidates pass every arithmetic vector, vertical checkpoint, and checksum.
- Dynamics-only runners exclude checkpoint and rolling-checksum overhead.
- Exact drag-halving and interpolation-fraction specializations preserve `0x3a014fa6`.
- Rust falls from 344,098,644 to 225,070,332 tool-specific cycles, a 34.59% reduction.
- Oscar64 falls from 410,417,915 to 235,021,443 tool-specific cycles, a 42.74% reduction.
- Common PAL VICE 3.10 timing measures 223,772,332 Rust cycles and 235,627,088 Oscar64 cycles.
- Those measurements are identical across three runs and reproduce the exact final state.
- Rust uses 5.03% fewer common-clock cycles; both remain below the rough PAL 123,000-cycle 8 Hz budget.
- Oscar64 is essentially tied on isolated multiply, 6.69% faster on general divide, and 33.26% faster on the specialized fraction divider.
- Rust's complete kernel remains faster, so the primitive results identify optimization targets rather than a reason to change languages.
- The timed Rust build is 9,026 bytes with 17 zero-page bytes and a 66-byte linker-reserved static stack.
- The timed Oscar64 build is 5,865 bytes with no mapped zero-page allocation and a 122-byte static-stack envelope.
- Compiler-provided Rust `u64` arithmetic remains a reproducible failing baseline; the selected core uses explicit two-word widening.

See [RESULTS.md](RESULTS.md) for measurements, profile evidence, and the next gate.

## Layout

    phase0/
        CONTRACT.md
        README.md
        RESULTS.md
        benchmark.ps1
        check.ps1
        primitive_timing.ps1
        resources.ps1
        timing.ps1
        candidates/
            rust/
            oscar64/
        generated/
            phase0_vectors.rs
            phase0_vectors.hpp
            phase0_vertical.rs
            phase0_vertical.hpp
        reference/
            generate_vectors.py
            emit_candidate_vectors.py
            emit_vertical_bindings.py
            vice_primitive_timing.py
            vice_timing.py
        vectors/
            phase0-v1.json
            phase0-v1.sha256

## Generate the vectors

From the project root:

    python phase0/reference/generate_vectors.py

Verify that the checked-in output is current:

    python phase0/reference/generate_vectors.py --check

The generator uses only Python's standard library. Decimal arithmetic runs at 60 digits of precision; the exact fixed-point path uses explicit integer operations.

## What the vectors establish

The generated artifact contains:

- Scaled multiplication cases.
- Scaled division cases.
- Linear interpolation cases.
- Raw fixed-point constants and environment tables.
- Exact fixed-point vertical-flight checkpoints.
- High-precision checkpoints for the same tabulated model.
- A final rolling state checksum.

Rust and C++ must consume the same integer data and produce the same exact results before performance measurements count.

## Next implementation slice

Finish the remaining Phase 0 numeric-foundation work:

1. Perform range analysis for Phase 1 state, force, environment, and intermediate quantities.
2. Select production fixed-point formats and overflow behavior.
3. Choose the initial integrator and timestep from error and cost evidence.
4. Define deterministic scenario and telemetry formats.
5. Add the remaining analytic integration cases.

Reproduce the completed experiment from the project root:

    .\phase0\check.ps1
    .\phase0\benchmark.ps1
    .\phase0\timing.ps1
    .\phase0\primitive_timing.ps1
    .\phase0\resources.ps1

Rust was selected by the frozen rubric and representative full workload, not by reaching the benchmark first.
