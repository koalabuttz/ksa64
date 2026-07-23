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

## Accepted Phase 3 validation

Phase 3 freezes four deterministic closed-loop cases: nominal, a 15-second altimeter dropout, a 60-second GPS outage, and a stuck steering actuator. Native tests cover transport rejection, truth isolation, world authority, sensor scheduling, navigation, sequencing, abort behavior, KST3 inspection, KRP3 derivation, and exact Phase 2 compatibility.

An independent Python reader validates KST3 framing and CRCs without using the Rust codec, then computes float64 orbital elements, post-cutoff coast propagation, load extrema, cutoff navigation error, and GPS-outage bridge error. The accepted orbital cases remain between 180 and 220 km at both apsides, below 0.01 eccentricity, 60 kPa Max-Q, and 60 m/s^2 acceleration. Cutoff navigation remains within 1 km and 10 m/s; outage bridging remains within 5 km and 30 m/s. The stuck case must latch abort and propulsion safeing.

Exact native/MOS agreement is checked with finite C64 probes that compare every named state field plus truth, sensor, navigation, and flight checksum chains. Three stable PAL runs freeze the target measurements. The target presentation path independently validates all KRP3 records and is accepted only when final VIC-II screen memory and event cue counts match the reviewed evidence.

`phase3/check.ps1` validates generated evidence, every SHA-256 sidecar, formatting, `no_std` compilation, lints, and all native tests. `phase3/complete.ps1` also runs the Phase 2 compatibility audit and both naturally terminating C64 gates.

## Accepted Phase 4 validation

Phase 4 adds statistical breadth without treating repetition as independent physical validation. The portable campaign engine is checked against an independently implemented distribution generator, and the frozen 1,024-run campaign is parsed and analyzed by Python without using the Rust codecs or orbit classifier. Serial, 5-worker, and 12-worker executions produce identical ordered KSC4/KSR4 artifacts.

Run zero is the primary compatibility gate: it must reproduce the accepted Phase 3 truth, sensor, navigation, flight, and KST3 checksum chains exactly. Recording-disabled, stock-retention, and every supported REU plan must produce identical mission and aggregate checksums.

Target acceptance is split into finite bounded probes:

- MOS and native vectors establish exact distribution, configuration, summary, and aggregate behavior.
- PAL VICE verifies the stock UI directly from screen memory.
- Preserving REU probes cover no REU and 128 KiB through 16 MiB, including explicit DMA ordering, archive commits, and recovery.
- IEC probes compare all exported bytes with the host source and require visible failure on disk-full conditions.

Archive and export corruption tests reject the first invalid record, truncation, identity mismatch, incomplete archive, missing/duplicate/reordered volume, oversize selection, and disk error. Storage failure may make evidence incomplete but cannot alter simulation state or later random draws.

The full target campaign is not an acceptance requirement. The measured closed-loop path projects one C64 mission at 243.7 minutes, 64 runs at approximately 10.8 days, and 1,024 runs at approximately 173.3 days. No long run is started without a current projection and explicit confirmation, and no run is canceled to manufacture timing evidence.

The frozen audit and measurements are in `phase4/COMPLETION.md`.
