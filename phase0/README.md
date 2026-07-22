# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The arithmetic, vertical-flight correctness, performance, and common timing gates are complete:

- Both candidates pass every arithmetic vector, vertical checkpoint, and checksum.
- Dynamics-only runners exclude checkpoint and rolling-checksum overhead.
- Exact drag-halving and interpolation-fraction specializations preserve `0x3a014fa6`.
- Rust falls from 344,098,644 to 225,070,332 tool-specific cycles, a 34.59% reduction.
- Oscar64 falls from 410,417,915 to 235,021,443 tool-specific cycles, a 42.74% reduction.
- Common PAL VICE 3.10 timing measures 223,772,332 Rust cycles and 235,627,088 Oscar64 cycles.
- Those measurements are identical across three runs and reproduce the exact final state.
- Rust uses 5.03% fewer common-clock cycles; both remain below the rough PAL 123,000-cycle 8 Hz budget.
- Compiler-provided Rust `u64` arithmetic remains a reproducible failing baseline.

See [RESULTS.md](RESULTS.md) for measurements, profile evidence, and the next gate.

## Layout

    phase0/
        CONTRACT.md
        README.md
        RESULTS.md
        benchmark.ps1
        check.ps1
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

1. Add primitive timing for multiply, general divide, and fast interpolation divide.
2. Confirm that the remaining 2,048 acceleration divisions are the next material bottleneck.
3. Record static RAM, zero-page, stack, and generated-code evidence for both timed builds.
4. Confirm the common timing method on real hardware when available.
5. Apply the language rubric and record the Phase 0 decision.

Reproduce the common timing gate from the project root:

    .\phase0\timing.ps1

No language wins because it reaches a benchmark result first.

