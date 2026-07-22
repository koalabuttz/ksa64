# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The arithmetic and vertical-flight correctness gates are complete:

- The Phase 0 v1 numeric and vertical-workload contract is frozen.
- Generated Rust and C++ bindings consume the same checked JSON vectors.
- Specialized Rust and Oscar64 C++ pass every arithmetic vector.
- Both candidates match all 12 vertical checkpoints and checksum `0x3a014fa6`.
- Rust completes the correctness workload in 441,745,996 simulated cycles.
- Oscar64 completes it in 457,888,329 profiled cycles.
- Compiler-provided Rust `u64` arithmetic remains a reproducible failing baseline.
- Dynamics-only timing and arithmetic optimization have not begun.

See [RESULTS.md](RESULTS.md) for measurements, the rust-mos finding, and the next gate.

## Layout

    phase0/
        CONTRACT.md
        README.md
        RESULTS.md
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

1. Separate dynamics-only timing from checkpoint and checksum work.
2. Isolate multiply, divide, interpolation, and complete-step costs.
3. Inspect generated assembly and map-file contributions.
4. Test exact range-specific reciprocal strategies against restoring division.
5. Add CIA timing after emulator measurements are stable.

No language wins because it reaches a benchmark result first.

