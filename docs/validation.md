# Validation strategy

## Why validation is separate from portability

Running the same source on a host and C64 can show that both targets execute the implementation consistently. It cannot show that the model represents the intended physics.

If both builds omit a factor in the drag equation, they can agree perfectly and still be wrong.

KSA64 therefore uses several independent layers of evidence.

## Validation layers

### 1. Arithmetic tests

Test fixed-point primitives against exact integer expectations:

- Conversion at representable boundaries.
- Signed addition and subtraction.
- Widening multiplication.
- Scaling, rounding, and saturation.
- Division and reciprocal approximations.
- Interpolation endpoints and interior values.
- Binary-angle wraparound.

Boundary and overflow cases matter more than large quantities of random ordinary values.

### 2. Analytic dynamics cases

Use cases with known results:

- No-force constant-velocity motion.
- Constant acceleration.
- Constant mass flow.
- Vacuum vertical motion over short intervals.
- Circular orbit in an ideal two-body model.
- Simple ballistic trajectories.

Integrator error should be predicted from the chosen method and timestep rather than hidden inside a loose tolerance.

### 3. Invariants

Check properties that should remain constant or change monotonically:

- Specific orbital energy during unforced two-body coast.
- Angular momentum during central-force coast.
- Propellant never increases during a burn.
- Total mass changes only through declared events and mass flow.
- Simulation time advances exactly.
- Quaternion norm remains controlled in later 6-DOF work.

Invariants often reveal errors before final-state comparisons do.

### 4. Exact cross-target comparison

The native exact-arithmetic build and C64 build should use identical:

- State representation.
- Tables.
- Constants.
- Event order.
- Arithmetic.
- Pseudorandom sequence.

Compare checkpoints and rolling state checksums. When a mismatch occurs, use the first differing step rather than only the final result.

### 5. High-precision comparison

Use a compact host-only floating-point calculation to estimate error introduced by:

- Fixed-point quantization.
- Table spacing and interpolation.
- Timestep.
- Integrator.
- Simplified constants.

This comparison should share scenario inputs but does not need the product interface, displays, avionics, or storage architecture. Phase 1 implements this as an 80-digit Decimal semi-implicit-Euler run at the product step plus RK4 runs at 1/32 and 1/64 of that step. The finer pair must converge within 1 mm altitude and 0.01 mm/s velocity; the accepted evidence is recorded in `phase1/HIGH-PRECISION.md`.

### 6. Independent external tools

Choose a reference by regime:

| Regime | Candidate reference | Typical comparisons |
|---|---|---|
| Atmospheric rocket flight | RocketPy | altitude, velocity, dynamic pressure, mass, events |
| Orbital propagation | Tudat or GMAT | state vectors, energy, apogee, perigee, period |
| Satellite tracking | PREDICT or QUIKTRAK | pass times, azimuth, elevation, ground track |
| Rigid-body or GNC cases | A small established dynamics package or published check case | attitude, angular rate, controller response |

External tools are not automatically correct for a KSA64 case. Their assumptions must be aligned first.

## Comparison contract

Every cross-tool test records:

- Coordinate frame and axis orientation.
- Time system and epoch.
- Units.
- Earth radius, rotation rate, and gravitational parameter.
- Atmosphere and wind model.
- Vehicle mass, reference area, and coefficient conventions.
- Thrust direction and curve.
- Event definitions.
- Integrator and output cadence.
- Initial conditions.
- Expected tolerances and why they are reasonable.

Without this contract, differences are ambiguous rather than informative.

## Test artifacts

Each reusable validation case should eventually contain:

- Human-readable scenario description.
- Machine-readable input.
- Source or derivation for expected results.
- Expected checkpoints.
- Tolerances.
- Versioned output from the external reference when applicable.
- Native exact, high-precision, emulator, and hardware results.

Reference tools generate golden data during development; they should not become runtime dependencies.

## Tolerance policy

Do not begin with one universal percentage tolerance.

Set tolerances by:

- Quantity.
- Flight phase.
- Expected integration error.
- Fixed-point resolution.
- Reference-model differences.
- Accumulation duration.

A passing tolerance should explain what source of error it permits. If a tolerance is widened after a failure, record the reason and evidence.

## Regression policy

Once a case is accepted:

- Preserve its inputs and expected outputs.
- Record intentional model changes.
- Compare the first divergent timestep.
- Require review for updated golden results.
- Keep old results when they document a meaningful model version.

Visual similarity is never sufficient evidence for a numerical regression.

## Model correlation

KSA64 is a learning simulator, not a certified vehicle model. Even so, the project should preserve the distinction between:

- Verification: the equations and code were implemented as intended.
- Validation: the selected model is adequate for its stated use.
- Correlation: model parameters were adjusted against measured physical data.

Without wind-tunnel, engine, structural, or flight-test data, KSA64 can be internally rigorous while still making only limited claims about real vehicles.

