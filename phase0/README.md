# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The arithmetic, vertical-flight correctness, and first performance gates are complete:

- Both candidates pass every arithmetic vector, vertical checkpoint, and checksum.
- Dynamics-only runners exclude checkpoint and rolling-checksum overhead.
- Exact drag-halving and interpolation-fraction specializations preserve `0x3a014fa6`.
- Rust falls from 344,098,644 to 225,070,332 cycles, a 34.59% reduction.
- Oscar64 falls from 410,417,915 to 235,021,443 cycles, a 42.74% reduction.
- The optimized kernels use 109,898 and 114,757 reported cycles per 0.125-second step.
- Compiler-provided Rust `u64` arithmetic remains a reproducible failing baseline.
- CIA timing on a common C64 target remains required before comparing languages directly.

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

1. Add identical CIA timer boundaries to both C64 dynamics runners.
2. Run both binaries through one cycle-accurate C64 environment.
3. Add primitive timing for multiply, general divide, and fast interpolation divide.
4. Investigate the remaining 2,048 acceleration divisions without changing results.
5. Confirm timing on real hardware when available, then apply the language rubric.

No language wins because it reaches a benchmark result first.

