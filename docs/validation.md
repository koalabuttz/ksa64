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

## Accepted Phase 5 integrated-mission validation

Phase 5 Gates 1-8 separately verify fixed-point spatial arithmetic, rigid-body
Euler coupling, flexible modes, the rotating-Earth world, multirate vehicle,
strict spatial transports, aided navigation, and the complete guidance loop.
The Gate 8 mission layer freezes six outcomes only after the unchanged Phase 3
and Phase 4 paths continue to pass.

An independent Python audit converts raw ECI terminal position and velocity to
float64 orbital elements without using the Rust orbit classifier. Nominal and
gust missions remain inside 180-220 km and within 0.2 degree of 51.6 degrees.
The star-outage and RCS-depletion cases remain stable degraded orbits; gimbal
jam and damping loss abort irreversibly. Sampled nominal Max-Q remains below
60 kPa, angle of attack below 15 degrees, and navigation position error below
1 km.

Native tests freeze ordered outcome, step, event, and checksum evidence. A
bounded rust-mos probe verifies the generated guidance signature. Full target
missions are still excluded from routine validation until Gate 11 produces a
fresh linked-size and elapsed-time projection and the user explicitly approves
the run.
## Accepted Phase 5 spatial-campaign validation

Gate 10 verifies that run zero retains the Gate 8 nominal terminal state and all
three avionics checksum chains. KSC5 and KSR5 round trips, keyed samples, and
distribution bounds are tested natively; a finite rust-mos program checks the
same configuration/sample/summary path with signature `0xc921a2d2` and a
14,445-byte size-optimized image.

The frozen seed `0x4b534135` produces a 32-run routine campaign and a 256-run
reference campaign. Serial and eight-worker reference executions have identical
KSC5 and KSR5 bytes; the ordered summary chain is `0x3103d833`. The independent
Python parser reconstructs every variation checksum and computes float64 orbital
elements directly from raw terminal vectors. It finds 180 stable-orbit
classifications, 28 completed non-orbits, 48 safe aborts, and no numeric or
step-limit failures. The abort population is retained as controller robustness
evidence, not hidden by retuning during this gate.
## Accepted Phase 5 target-timing validation

Gate 11 uses three stock-compatible, naturally terminating PAL VICE programs.
Every target result agrees with the native exact-arithmetic probe, and three
runs produce identical cycle counts. Vehicle, avionics, and telemetry cost
15,565,702, 2,579,033, and 2,124,185 cycles respectively. Their conservative
sum projects the nominal mission to 19.69 hours, so no full target mission was
started. The accepted minor fast path preserves all frozen artifacts; a second
candidate was reverted after a rust-mos-only inertia divergence.

## Accepted Phase 5 adaptive-history validation

Gate 12 requires exact equality between recording-disabled and KPH5-observed missions. The strict 1,664-byte stock history has 99 ordered points, two CRC layers, and independently checked campaign/run identity. Stock retention selects runs `[0, 1, 4, 53, 2]` from the frozen 256-run KSR5 stream. Independent Python allocation agrees with Rust for no REU and all eight supported capacities. KRA5 corruption and interrupted writes reject the first bad record while retaining the previously committed prefix. A finite rust-mos codec/allocation probe freezes signature `0xb5783bf2`. PAL VICE additionally caught and rejected a target-only quotient-planner divergence before the accepted bounded-loop planner passed no-REU and every 128 KiB–16 MiB tier. No full C64 mission is required.

## Accepted Phase 5 mission-control replay validation

Gate 13 requires native KPH5 replay to reject corruption and identity substitution and to reproduce the frozen 99-point extrema/event summary. A naturally terminating setup phase in the 6,252-byte stock PRG validates the tape before rendering. PAL VICE checks all 1,000 screen bytes, key rows, plot population, pass marker, and cue hash `0x3b2fb64b`. The PRG loads only through `$206B`; no physics or campaign run is started.

## Accepted Phase 5 completion validation

Gate 14 combines the inherited Phase 4 evidence check with every Phase 5
generator, independent parser/analyzer, native regression, finite rust-mos
probe, PAL REU capacity case, stock replay, and three-run target timing gate.
Every checked-in Phase 5 SHA-256 sidecar is verified after those behavioral
checks. The audit is bounded and deliberately does not launch a complete target
mission or campaign.

The final evidence supports implementation verification and declared learning
objectives, not certification or correlation to a physical launch vehicle.
KSA-5A uses simplified gravity/environment, aerodynamic, flexible-body,
actuator, sensor, and guidance models. Campaign frequencies are results under
reviewed synthetic distributions rather than real-world probability claims.
`phase5/COMPLETION.md` freezes the accepted measurements and limitations.
## Target probe publication discipline

A target probe's completion magic is a commit marker. The probe must clear the
marker before work, write every result field with bounded volatile stores, and
publish the magic only after the complete result is visible. Monitors must
ignore records without the exact final marker. Publishing magic first creates a
race in which VICE or physical monitoring hardware can accept a partially
written record.


## Accepted Phase 6 software validation

Phase 6 first requires the allocation-free exact endpoints to reproduce the frozen Phase 5 terminal state and all three avionics checksum chains. Native link tests then cover framing, identity, ordering, replay, backpressure, deterministic impairment, timeout, and disconnect behavior. The realtime broker compares every returned KLR6 command and status cell with an independent native shadow flight computer.

Three naturally terminating PAL CIA probes measure the ordinary, navigation/status, and guidance releases. Their accepted maxima are 12,339, 23,656, and 14,914 cycles against a conservative 24,631-cycle release budget. A complete stock-C64 KSA-6R endpoint subsequently processed 12,692 epochs under 1x PAL x64sc, reached the frozen terminal state, matched all shadow cells, and reported zero deadline misses and alarms. Binary-monitor transactions pause emulation, so that externally paced run proves complete target exactness but not end-to-end realtime transport.

The bounded completion runner builds every endpoint below the stock boundary, reruns the finite timing and endpoint probes sequentially, performs one mailbox exchange, and verifies the frozen full-flight artifact and PRG hash. It refuses to start while x64sc is already running, and its harnesses close VICE after success or proven failure. A complete live SwiftLink, Turbo232, Ultimate, or user-port hardware run remains open.

## Accepted Phase 6 Mission Control visualization validation

The host test suite validates the frozen nominal KPH5 identity and CRC before using it as a plan. Independent orbital tests reproduce the accepted nominal perigee, apogee, eccentricity, and inclination from the raw reference state; cover elliptical, impacting, escape, circular, and degenerate cases; and check one-period propagation, Earth-fixed geography, environment estimates, residuals, and antimeridian splitting.

Presentation acceptance renders all seven pages at 80x24, 100x30, 120x40, 160x48, and 200x60. F2 is separately rendered in Ascent, Orbit, and Ground Track modes with Braille and ASCII plotting. ASCII mode must produce an entirely ASCII buffer with no replacement characters. A provenance test changes every omniscient director field while holding operational inputs fixed: F1 through F6 must remain byte-identical, while F7 must change.

The existing complete native mission, KMR6 recovery/export, explicit stop, and disconnect behavior tests remain in the same gate. The full workspace regression passes. A finite eight-epoch one-VICE realtime TUI smoke rendered the strict-ASCII Ground Track page, shadow-verified all eight command/status cells, reported zero deadline misses and alarms, and closed the emulator and bridge after postflight exit. No complete target mission was rerun for a presentation-only change.
## Accepted Phase 7 validation

Legacy-facade tests compare Phase 7 normalized results with the unchanged Phase
2 and Phase 5 executors. Pack-compiler tests rebuild checked-in KVP7/KMP7/KMC7
bytes from offline source data. Exact mission tests freeze 2,702 state
transitions, event order, extrema, terminal state, and checksum; an independent
float64 implementation separately attributes the remaining numerical error.

The 1,024-run campaign is reproduced with one and four workers and must be
byte-identical. An independent Python reader validates KSC7/KRA7 framing,
reserved bytes, CRCs, run ordering, every embedded KSR7, the keyed sampler,
variation identities, and aggregate extrema without using Rust codecs.

Target acceptance combines a 129-state field-by-field native/MOS trace, direct
validation of the complete 1,000-byte KPH7 replay screen, stock linked-layout
checks, and one complete target mission. The accepted mission consumes
1,047,635,269 net PAL cycles (17.72 minutes), lands with every event observed,
reports zero faults, and reproduces checksum `0xa61c5720`. Routine audits verify
the frozen complete-run evidence and binary hash but rerun only the finite trace
and replay.

The evidence establishes implementation consistency and declared numerical
behavior. The Firestorm/I211W model is published-data-based but not
flight-correlated, certification-grade, or a real-world probability model.
