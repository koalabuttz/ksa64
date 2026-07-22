# Phase 0

Phase 0 measures whether rust-mos or Oscar64 C++ is the better foundation for KSA64's fixed-point simulation core.

This directory is deliberately separate from future production source. Everything here is benchmark infrastructure, reference material, or disposable candidate code.

## Current status

The Phase 0 v1 contract is frozen for the first implementation pass:

- Physical units and fixed-point formats are defined.
- Rounding, saturation, division, and interpolation behavior are defined.
- The representative vertical-flight workload is defined.
- An independent Python reference generates exact fixed-point vectors and high-precision checkpoints.
- Neither candidate implementation exists yet.

## Layout

    phase0/
        CONTRACT.md
        README.md
        reference/
            generate_vectors.py
        vectors/
            phase0-v1.json
            phase0-v1.sha256

Candidate implementations will be added only after the generated vectors pass their self-check.

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

1. Implement the primitive arithmetic kernels in native Rust.
2. Run the arithmetic vectors.
3. Compile the same Rust code with rust-mos.
4. Implement the same kernels in the Oscar64-compatible C++ subset.
5. Run the same vectors natively and on the C64 target.
6. Inspect generated assembly before beginning the vertical-flight loop.

No language wins because it reaches a benchmark result first.

