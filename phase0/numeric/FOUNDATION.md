# Phase 1 numeric foundation

Status: accepted for the Phase 1 vertical-flight laboratory.

This document selects the product numeric contract that Phase 1 will implement. It does not implement vehicle dynamics. The independent generator in `phase0/reference/generate_numeric_foundation.py` proves the declared storage and intermediate ranges and emits the checked-in analytic and binary fixtures.

The benchmark contract in `phase0/CONTRACT.md` remains frozen. It is evidence for compiler selection, not the product contract.

## Units and stored formats

All signed values use two's-complement `i32`. `Qm.n` below names `n` fractional bits; the sign is included in the remaining stored bits.

| Quantity | Unit | Format | Phase 1 envelope | Resolution |
|---|---|---:|---:|---:|
| Mission time and durations | s | Q16.16 | 0 to 4,096 s | 15.259 microseconds |
| Altitude | km | Q20.12 | -2 to 2,000 km | 0.244141 m |
| Velocity | km/s | Q8.24 | -8 to 8 km/s | 0.059605 mm/s |
| Acceleration and gravity | km/s^2 | Q4.28 | -0.1 to 0.1 km/s^2 | 0.003725 mm/s^2 |
| Total, dry, and propellant mass | t | Q20.12 | 0 to 5,000 t | 0.244141 kg |
| Mass flow | t/s | Q16.16 | 0 to 100 t/s | 0.015259 kg/s |
| Force terms | MN | Q20.12 | up to 200,000 MN before drag halving | 244.141 N |
| Net force | MN | Q20.12 | -500 to 500 MN | 244.141 N |
| Density | kg/m^3 | Q4.28 | 0 to 1.5 kg/m^3 | 3.725e-9 kg/m^3 |
| CdA | m^2 | Q16.16 | 0 to 2,000 m^2 | 1.526e-5 m^2 |
| Speed squared | km^2/s^2 | Q12.20 | 0 to 64 | 9.537e-7 |
| Density-speed term | numeric MN/m^2 before CdA | Q12.20 | 0 to 96 | 9.537e-7 |
| Interpolation fraction | dimensionless | unsigned Q0.16 | 0 through 65535/65536 | 1/65536 |

The product changes two benchmark choices deliberately:

- Time moves from Q20.12 to Q16.16 so event times and later multirate scheduling have 16 times finer resolution while retaining more than eight times the Phase 1 duration.
- Speed squared and the density-speed product use Q12.20. The benchmark reused Q8.24, which becomes tight above roughly 11.3 km/s and leaves little room for the combined density-speed term. Q12.20 retains useful low-speed drag precision and materially increases headroom.

Phase 2 must repeat range analysis before reusing these formats. In particular, an orbital or escape scenario may require a wider velocity-derived representation.

## Declared envelope and coupled constraints

Independent per-field limits are insufficient for physical expressions. Every accepted Phase 1 scenario must also satisfy:

- `density * velocity^2 <= 96` in the contract's numeric units.
- `abs(net_force / mass) <= 0.1 km/s^2` whenever mass is positive.
- `total_mass >= dry_mass > 0`.
- `0 <= propellant <= total_mass`.
- The baseline timestep is positive and no greater than 0.125 seconds.

The generator evaluates worst-case raw products after these constraints. The largest selected product needs 56 bits including sign, so the accepted explicit two-word widening arithmetic covers every Phase 1 multiplication. Every rounded, scaled result fits `i32`.

These are model-domain limits, not claims about all launch vehicles. A scenario outside them must be rejected or trigger a new numeric-contract version; it must not be silently clipped into the Phase 1 model.

## Rounding and overflow

Decimal source values and scaled arithmetic round to nearest, with exact halves away from zero. This rule is independent of Rust casts, signed shifts, and host division behavior.

The production policy has three layers:

1. Scenario loading validates raw field ranges, coupled constraints, positive mass, nonzero denominators, record versions, and checksums before the run starts.
2. Hot-path addition and subtraction may omit branches only where the generated range proof covers the operation. Debug and host-test builds check those proofs.
3. Public scaled multiply and divide primitives saturate an escaped final result to `i32` and set a sticky numeric-fault flag. Division by zero sets a fault. The run stops at the next step boundary.

Saturation is containment for diagnostics, not a valid simulated state. A nominal regression case fails if the sticky flag is ever set. This keeps a bad input or missed proof deterministic without letting a plausible-looking saturated trajectory continue.

Explicit two-word intermediates implement widening multiply, shifted division, magnitude operations, and rounding on both host and C64. No path depends on compiler-provided target `u64` behavior.

## Initial integrator and timestep

Phase 1 starts with semi-implicit Euler at a fixed 0.125-second physics step (8 Hz simulated time). Telemetry defaults to every eight physics steps (1 Hz simulated time).

The choice follows current evidence:

- The frozen representative Rust kernel takes 109,263.83 PAL CIA cycles per step, below the roughly 123,000-cycle budget available at 8 Hz before display and later avionics work.
- Semi-implicit Euler evaluates forces once per step. RK2 needs another model evaluation and is not justified before the complete Phase 1 model can measure its error reduction and cost.
- In the generated constant-acceleration case over eight seconds, 0.125-second semi-implicit Euler has 4.951 m fixed-point position error against the continuous solution. Halving the step reduces the result to 2.266 m; doubling it raises the result to 10.078 m, demonstrating the expected first-order trend.
- Constant velocity advances exactly in its representable analytic case, and constant mass flow reaches the exact dry-mass boundary after 1,216 steps.

This is not a universal accuracy claim. Each Phase 1 scenario records tolerances by quantity and phase. RK2 becomes a measured candidate after the end-to-end vertical model exists; adaptive integration remains outside the exact C64 baseline.

## Analytic cases

`numeric-v1.json` contains raw checkpoints for:

- No-force constant-velocity motion.
- Positive constant acceleration.
- Negative constant acceleration.
- Constant mass flow with an exact dry-mass boundary.
- A three-timestep convergence comparison for semi-implicit Euler.

The generator calculates fixed-point paths with exact Python integers and continuous references with 80-digit `Decimal` arithmetic. These cases become Phase 1 implementation tests; they are not a second simulator.

## Phase 1 environment

The initial environment is `earth.simple-atmosphere.v1`:

- Density is the 19-knot table already frozen in `phase0/CONTRACT.md`, from 0 through 2,000 km. Values at and above 120 km are zero in this intentionally simple model.
- Gravity at each altitude knot is generated from `g(h) = g0 * (R / (R + h))^2`, where `g0 = 0.00980665 km/s^2` and `R = 6371 km`.
- Density and gravity use clamped piecewise-linear interpolation with unsigned Q0.16 fractions.

The existing Phase 0 vectors already prove exact host/C64 interpolation for these scales and table values. Phase 1 will generate production Rust bindings from the same declared source data rather than copy benchmark implementation code. This table is a deterministic learning model, not a claim to implement a standard atmosphere.

## Generated artifacts

| Artifact | Purpose |
|---|---|
| `numeric-v1.json` | Formats, range proof, intermediate widths, analytic checkpoints, and fixture metadata |
| `scenario-v1.bin` | Golden packed scenario record |
| `telemetry-v1.bin` | Golden telemetry header and two frames |
| `*.sha256` | Reviewable artifact identity |
| `scenario-v1.schema.json` | Host-side source-schema contract |
| `examples/phase1-vertical.json` | Human-readable source for the golden scenario image |

Regenerate with:

    python -B phase0/reference/generate_numeric_foundation.py

Verify without writing:

    python -B phase0/reference/generate_numeric_foundation.py --check

The generator uses only the Python standard library. Generated artifacts are reviewed and committed; the C64 never runs the generator or parses JSON.
