# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The arithmetic correctness slice is complete:

- The Phase 0 v1 numeric and vertical-workload contract is frozen.
- Generated Rust and C++ bindings consume the same checked JSON vectors.
- Native Rust and native C++ pass the arithmetic vectors.
- Specialized two-word Rust passes under `mos-sim` and builds for the C64.
- Oscar64 C++ passes in Oscar64's integrated C64 emulator.
- Compiler-provided Rust `u64` arithmetic is retained as a reproducible failing baseline.
- Vertical-flight implementation and cycle measurements have not begun.

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
        reference/
            generate_vectors.py
            emit_candidate_vectors.py
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

1. Implement the frozen vertical-flight step with specialized two-word Rust arithmetic.
2. Implement the same step with Oscar64-compatible C++ arithmetic.
3. Match all fixed checkpoints and the final FNV-1a checksum.
4. Inspect generated assembly and isolate kernel sizes.
5. Add CIA and emulator cycle measurements only after exact agreement.

No language wins because it reaches a benchmark result first.

