# Phase 5 spatial numeric foundation

Gate 2 adds one allocation-free spatial math implementation shared by native
Rust and the pinned rust-mos target. It does not use compiler-provided 64-bit
multiplication or division in the production kernels.

## Values and operations

- `FixedVec3<F>` stores exactly three signed 32-bit components and carries its
  fractional scale in the type.
- Position, velocity, acceleration, angular-rate, torque, and modal aliases use
  the Q formats frozen by the Phase 5 contract.
- Dot, cross, mixed-scale cross, addition, subtraction, and scalar operations
  use the existing sticky `NumericStatus` fault policy.
- `QuaternionQ30` is scalar-first Hamilton, represents an active body-to-ECI
  rotation, and occupies exactly four signed words.
- Hamilton products, conjugation, checked normalization, and vector rotation
  are deterministic integer operations.

Quaternion normalization needs the square root of a shifted 32-bit value. The
new square-root helper compares two-word trial squares against a two-word
radicand. It therefore avoids the rust-mos wide-integer optimizer risk already
identified in Phase 0.

## Evidence

`phase5/reference/generate_spatial_vectors.py` independently produces integer
vectors for dot and cross products, Hamilton composition, a 90-degree vector
rotation, and quaternion normalization. Native tests consume the generated
vectors directly. The finite `ksa64-phase5-spatial-sim` executable consumes the
same evidence under `mos-sim-none` through David's pinned rust-mos Docker image.

Both paths produce zero failures. Boundary tests additionally cover shifted
square roots, invalid scales, saturation propagation, and zero-norm fail-closed
behavior. This gate deliberately measures correctness only; representative
cycle timing begins after the coupled attitude kernel exists.