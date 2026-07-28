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

## Accepted Phase 8 validation

The completion audit first reruns the complete Phase 0-7 workspace regression and rebuilds all source-bound packs. Analytic fixtures cover spatial numeric types, component mass and parallel-axis calculations, CP and normal-force symmetry, drag interpolation, envelope rejection, inertial and torque propagation, rail release, crosswind, and keyed gust repeatability.

The frozen Firestorm/I211W mission produces strict KST8/KSR8/KPH8 evidence and is independently integrated in float64. Event times agree within their active timestep, apogee within 0.5%, landing position within the larger of 5 m or 2%, and predeployment attitude within 0.5 degrees. Nineteen aligned OpenRocket 24.12 checks cover mass/stability reconstruction and calm/crosswind flight metrics without tuning the assumption-backed drag table.

The 1,024-run seed-`0x4b534138` campaign is byte-identical with one and four host workers; the independent parser validates ordering, keyed variations, every CRC, aggregates, and corruption rejection. Stock and every REU retention plan preserve the same physical summaries.

Target evidence consists of all three programs linking below `$C000`, a 17-state field-exact native/MOS trace, and a seven-page replay with a frozen screen checksum. The finite trace measured 59,421,528 PAL cycles, approximately 3.71 million cycles per powered step. A complete mission conservatively projects to 2.35 hours, so it was not started under the established 30-minute projection-and-confirmation rule.

These results are engineering-model evidence only. Neither KSA64 nor the OpenRocket comparison provides launch approval, certification, regulatory advice, or safety authority.

## Accepted Phase 8.5 validation

Exact-clock analytic tests cover retained physical deadlines, arbitrary split points, scheduled events, release epochs at N times 8,192 Q18 units, and command effects first visible at N+1. Format tests cover identity, reserved bytes, CRC, truncation, duplicate/stale/reordered epochs, and unsupported frames or actuator capabilities.

Truth-blind avionics tests cover deterministic bias/noise, quantization, delay, dropout, stale/invalid samples, barometer/GPS correction, recovery arming, one-shot feedback, timer backups, deadline alarms, bounded link loss, and third-epoch safeing. The original Firestorm never receives a physical attitude command. The fictional 20 g derivative respects rail, slew, lag, saturation, jam, and burnout boundaries and passes the 5 m/s crosswind <=3 degree settling gate within 0.25 seconds.

The 64-run seed-0x4b534185 campaign is byte-identical with one and four workers and independently parsed. Host/host and native external placements agree exactly; finite monitor and gimbal VICE probes validate the generic stock endpoint. The standalone C64 kernel costs 21,184 aided and 10,843 fast PAL cycles, passing the 24,631-cycle budget.

The self-contained combined build is validation evidence of an explicit packaging limit: 71,500 resident bytes, 20,301 bytes beyond the ordinary region and at least 5,964 bytes beyond total physical RAM before reservations. No full combined mission was started. The completion audit validates the preserved Phase 0-8 artifacts and never silently launches long target work or multiple VICE instances.

These results are engineering simulation evidence, not launch approval, certification, regulatory evidence, or safety authority.

## Accepted Phase 9 validation

The accepted seed is `0x4b534139`. Ten studies—six primary accepted searches, a coupled demonstration, an experimental broad-airframe demonstration, and two quick fixtures—were run with one, four, and eight workers. All seven emitted artifacts for every study are byte-identical across worker counts.

Independent Python verification parses KOM9/KDV9/KOE9/KRA9/KRE9/KAS8 without Rust codecs; checks every framing field, reserved byte, CRC, identity, quantization rule, generation fingerprint, retained case, finalist tier, and feasibility bit; and reconstructs the terminal Pareto front. Unit fixtures separately recover an analytic known front and a DE integer-sphere optimum within one design quantum. Interrupted/resumed and uninterrupted search archives are byte-identical.

The persistent JSONL example proves hello, ordered duplicate-aware evaluation, checkpoint, malformed-input isolation, and close behavior. Reports are self-contained and passive; progress observers and renderers cannot alter SearchResult.

The rust-mos finalist browser is 15,391 bytes and ends at `$441E`, below the stock `$C000` boundary without an REU. After earlier monitor-handshake failures in an overloaded emulator environment, the unchanged probe and PRG passed twice in a clean one-instance environment. The frozen result validates four finalists, manifest `e86077d4`, status zero, and complete process cleanup. This supports a transient environment/starvation diagnosis rather than a wire-format or target-program defect.

The accepted physical studies remain inside the Phase 8 model envelope and promote only candidates satisfying all 64 hard-constraint cases. The broad-airframe demonstration is explicitly experimental, promotes no accepted finalists, and is never described as correlated or safety-valid.

## Accepted Phase 9.5 validation

Phase 9.5 validates each new physical responsibility at the narrowest useful level before composing missions:

- Analytic force, torque, pulse, depletion, centre-of-mass, and inertia cases.
- A small independent float64 implementation for canard/RCS dynamics, actuator response, allocation, and authority transitions.
- Deterministic native/MOS vectors and bounded target probes for exact portable behavior.
- Integrated gimbal-only, canard-only, RCS-only, and mixed-effector missions with frozen fault and handoff cases.

Basilisk is optional secondary evidence only for selected fixed-step spacecraft-attitude/RCS cases. It is not used to validate canard aerodynamics, exact-event release semantics, mixed allocation, or authority handoff. Any retained comparison is a frozen, versioned fixture. Routine tests and CI remain offline and do not install or execute Basilisk.

A Phase 9.5 external fixture is acceptable only when it records the generating tool and version, complete input/configuration, model assumptions, integration settings, raw output, conversion procedure, declared tolerance, content hash, and regeneration instructions.

Gates 1–9 now include exact native/MOS contract vectors, independent canard/RCS/allocator float64 checks, integrated Firestorm C9/R9/M9 missions, a deterministic 64-case campaign, and canard/RCS/mixed grid and NSGA-II studies whose archives are byte-identical at one, four, and eight workers. KSA-X1 remains experimental and produces no accepted finalist.

Gate 10 preserves the failed realtime and stock-world measurements as engineering evidence, then accepts host world plus externally paced stock-C64 flight as the interim placement. The clean endpoint is 44,306 bytes, ends at `$B511`, requires no REU, and passes an eight-release one-instance VICE probe with every KLR9 command/status cell shadow-verified. Simulated 32 Hz epochs and successor-command semantics are exact; wall-clock pacing may pause, so this is hardware-in-the-loop exactness rather than a realtime flight claim.

Gate 11 validates presentation and finalist workflows separately from physics. Passive observation produces the same 64-release host/host terminal checksums with and without the F1–F7/KMR9 sink. KFE9 corruption and deterministic stock/REU retention tests pass. The first stock browser build exposed excessive software-stack pressure from materializing design, aggregate, and KAS9 records simultaneously; page-scoped parsing fixed the proven target fault. The accepted 29,010-byte browser ends at `$7951`, reports eight mixed finalists in VICE, requires no REU, and closes the sole emulator instance.

The additive KFB9 selected-finalist endpoint is 39,963 bytes, ends at `$A41A`, and leaves 7,142 bytes before `$C000`. Strict native tests exercise the first accepted canard, RCS, and mixed finalists. Three sequential eight-release one-instance VICE probes then shadow-verify their KLR9 command/status cells and truth, navigation, flight, and allocator checksum chains. The PRG hash is `ea1c315aa44abccfbc112601319fa11997abcc2f351b47844e01399d2ff23597`; the canard, RCS, and mixed KFB9 hashes are respectively `569d1363f45b28e6ab277d946fff026cbd82c3d349a109413c99d7d420ba3b18`, `fc695f5e897f8066e3fedb44b796e2307901dcbb87df32f0a4d7c6a40609aa23`, and `1f7121b175f8cea46b12ef8dd72e58487b5f9fe3dcb485e4f5a777b94db11154`. These bounded runs prove configurable target execution, not wall-clock realtime flight.

## Accepted Phase 10 validation

Phase 10 uses four deliberately distinct evidence layers:

1. Frame/time-only transforms with force propagation disabled.
2. Environment and force snapshots at fixed states and epochs.
3. One-step and boundary-transition cases.
4. Integrated atmospheric, ballistic, and near-orbital trajectories.

The portable `GlobalEcef6DofV1` transition is authoritative. A separate float64 implementation covers the complete accepted global model. SatKit is the preferred specialist reference for time scales, Earth orientation, frame transformations, gravity, and selected coast fixtures. Orekit is used only when a documented SatKit gap or useful independent comparison justifies it. GMAT supplies occasional exoatmospheric trajectory corroboration.

Before accepting fixtures, Phase 10 must freeze:

- Reference ellipsoid and gravity identity.
- Earth rotation/orientation plus any precession/nutation model.
- Supported input/output time scales and the continuous internal integration scale.
- Leap-second and Earth-orientation datasets, versions, validity windows, and out-of-range behavior.
- Axis, transform-direction, quaternion, angular-rate, velocity-transport, and epoch conventions.
- Permitted simplifications and their validated mission envelope.

Transform cases span multiple epochs, leap-second and EOP boundaries, an explicit out-of-coverage failure, the equator, both sides of the date line, high altitude, near both poles, and exact poles with a declared reference meridian. Round-trip and transition tests cover position, velocity, attitude, angular rate, and simulation time. Quaternion comparisons use rotation equivalence so `q` and `-q` are not falsely reported as different attitudes.

Every evidence report separates frame/time disagreement from force/environment disagreement and integration accumulation. External fixtures record tool/data versions, hashes, inputs, epoch and time-scale declarations, source and destination frames, transform direction, Earth/gravity/atmosphere settings, raw output, conversion code, tolerances, and regeneration instructions.

Normal tests and CI consume checked-in fixtures without network access, live leap-second/EOP data, or installed external tools. No validator is permitted to own, correct, or co-propagate production state.

The exact uninstrumented world and separate float64 implementation differ by
0.001663% in apogee, 0.014047% in downrange, and 48.9 m at landing. All
in-flight events and transitions agree within one 32 Hz step; terminal ground
contact uses a separately declared four-recovery-step bound and differs by
0.09375 s. The controlled truth-blind mission independently completes 22,015
releases and all four frame changes.

The 64- and 256-case archives are byte-identical with one, four, and eight
workers. All 256 cases physically recover without numeric, frame, time, or
model-envelope faults. Finite warp-disabled stock-C64 probes exact-match
release-class and transition checksum chains without claiming realtime.

## Accepted Phase 11 validation

Phase 11 preserves the complete Phase 0-10 regression and validates the new
operational shell independently from authoritative physics. The completion
audit covers package/ABI identity, mission-plan transfer,
stage-validate-commit atomicity, ground-link isolation, onboard and
ground-estimate prediction, deterministic procedures and roles, action replay,
event-journal recovery, session bundles, deleting/truncating/corrupting every
new record family, and controlled debrief counterfactuals.

SafeholdRecoveryV1 has separate native and rust-mos evidence because it is an
independent limited package. The complete reference package retains the same
portable source but uses the accepted banked stock-RAM stopgap. Its host fixture
generator produces a strict 13-record transcript covering ordinary and aided
releases, prediction, stage, commit, ground blackout/reacquisition, and journal
recovery. The stock-C64 endpoint must match every output byte and navigation,
flight, and command checksum.

The accepted warp-disabled PAL VICE run matches all 13 records, preserves every
code segment and both bank guards, and uses 16 of the reserved 279 emergency
software-stack bytes. Final checksums are navigation c73060d2, flight 6e07595c,
and command 6ab926f2. Diagnostic host wall time is not a PAL cycle claim; the
endpoint is externally paced.

The independent flat safehold endpoint also passes a 16-release native/C64
exactness probe. Its 32,857-byte initialized image ends at `$8858`; the
measured 4,330-byte rust-mos static-stack reservation extends the complete
runtime footprint to `$9942`, leaving 9,918 bytes before `$C000`. The
package-local stack declaration and compiler static-stack reservation are
recorded separately.

This evidence proves portable implementation consistency on stock C64 CPU/RAM
under VICE. It does not prove a physical loader or user-port/ACIA/Ethernet link,
realtime operation, dissimilar redundancy, certification, launch approval, or
real-vehicle accuracy.

## Accepted Phase 11.5 product validation

Phase 11.5 validates orchestration and discoverability without creating a new physical oracle. The completion gate composes the frozen Phase 11 audit with:

- deterministic current/historical catalog ordering and an exact checked JSON snapshot;
- duplicate, missing-adapter, impossible-placement, and nonexistent-asset rejection;
- a stable non-mutating quick start;
- direct application-service tests for every advertised family;
- exact KSB11 session parity between unified and Phase 11 entrypoints;
- exact legacy telemetry alias parity;
- stored target verification with process creation excluded;
- explicit rejection of live target requests missing `--live`;
- rust-mos package/hash and stock-memory regression; and
- sequential finite VICE evidence using one warp-disabled instance at a time with cleanup.

The catalog advertises only scenarios callable through the product facade. Specialist crosswind, fault-matrix, and full engineering switches remain discoverable through their historical owner rather than being falsely promised by a smaller adapter.

The accepted catalog contains 13 current experiences and seven target descriptors. Its SHA-256 is `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`. The unified GNSS-loss session remains the frozen 22,369-byte Phase 11 evidence with identity `0x6d4122a0`.

This validation proves that presentation and orchestration preserve accepted behavior. It does not add real-vehicle accuracy, physical-link acceptance, realtime C64 performance, certification, launch approval, or safety authority.


## Accepted pre-Phase 12 application hardening

The Phase 11.5 follow-up validates organization and API semantics without changing product evidence:

- the facade is reduced to public orchestration and common request/error types while focused modules own domain adapters;
- every ordinary CLI product action routes through the nested request family;
- request-policy tests distinguish read-only, workspace-writing, external-process, and explicitly live work and expose safe cancellation boundaries;
- authored project IDs cannot shadow accepted product IDs;
- accepted model references do not grant accepted product maturity;
- recent sessions validate accepted-product or authored-project origins without entering either catalog;
- unknown binary evidence reports `recognized_format: false` and requires its owning strict parser; and
- the checked `ksa64.product-catalog.v1` bytes and SHA-256 remain unchanged.

Host tests, Clippy, CLI parity, the Phase 11 compatibility session, and the no-live Phase 11.5 completion audit remain the acceptance boundary. No target code changed, so the already accepted finite VICE evidence is reused rather than repeated.


## Accepted live-session application boundary

The pre-Phase 12 gate additionally proves:

- Compiled, Ready, Running, Paused, Completed, and Aborted lifecycle transitions reject invalid operations.
- A release advances only through the accepted Phase 11 package and exposes a truth-blind typed snapshot.
- Fast mode is bounded by the caller budget, real-time mode advances at most one release per scheduling call, paused mode advances none, and single-step advances exactly one before pausing. Wall time is not evidence.
- Stage, commit, and cancellation use the accepted atomic uplink records; no session API directly commands effectors.
- Scripted and manually submitted copies of the accepted GNSS-loss transcript produce byte-identical `CompletedMissionSession` and KSB11 bytes.
- `Ksa64Application` reports the live capability and starts the session; unsupported synchronous experiences fail closed.
- The Phase 11 operations console observes the live session instead of precomputing the completed result.

These tests validate application orchestration, not new flight physics. All prior authoritative checksums and formats remain frozen.
## Accepted Phase 12A bridge and Unreal feasibility

Phase 12A was accepted at source commit `e98df4921c03` without changing any
authoritative model, flight implementation, or canonical format. The gate
includes:

- the complete frozen Phase 0–11.5 audit, formatting, warnings-denied Clippy,
  and full native workspace tests;
- native C++ harness coverage for ABI and structure-size rejection, null and
  malformed inputs, immutable role filtering, lifecycle transitions, buffer
  ownership, diagnostics, contained panic, and deterministic `QueueFull` parity;
- exact guided GNSS-loss completion as the unchanged 22,369-byte KSB11 archive
  with SHA-256
  `38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8`;
- a 13-entry catalog with SHA-256
  `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`;
- two passing Unreal automation tests and zero failures;
- a cooked and packaged Development runtime that hash-validates the
  commit-qualified bridge, contains no editor-plugin binaries, and exits its
  standalone smoke test successfully;
- one loopback-only optional MCP inspection and disposable actor mutation, with
  the actor removed and no tracked asset retained; and
- repository hygiene checks proving generated Unreal directories are ignored,
  Unreal binary assets are LFS-governed, and no generated or non-LFS asset was
  committed.

The accepted bridge DLL SHA-256 is
`d1605c4aa9a8b407d8e35ee76d965e404c1e7efcc357d8bd0704b73ade43272d`.
This evidence qualifies the presentation boundary and failure containment only.
It adds no simulation authority, renderer, physics, real-vehicle validation, or
new canonical evidence semantics.

## Phase 12B live-operations validation

Phase 12B retains the entire Phase 12A gate and adds two distinct fixtures: the unchanged compressed nine-release session remains the ABI and exact 22,369-byte KSB11 compatibility oracle, while `FullMissionGnssLossV1` runs the complete mission with human-scale operator windows. The no-action path retains 22,015 releases (687.96875 seconds); the accepted four-action reference transcript lands at release 21,591 (674.71875 seconds) because its ground update changes the subsequent guided trajectory.

The measured scripted reference seals 2,911,464 KSB11 bytes with SHA-256 `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`. Its nested evidence is 175,232 KTT10 bytes (`456c512825388b7df1d65c1fa8f08a0c086c4be794c6912cc7e1223cd406e2e1`), 32,896 KPH10 bytes (`cef09c40f95fd75f52ec7a15f8e9db0e12f9d2ffd12b6c107bbc4c6cfb853223`), and a 512-byte KSR10 summary (`6aee34461cc0da65b79ba1954a48a6ad90803d29857bf444a53998ae9de622d1`). The accepted disposition is Degraded Success with PrimaryAchieved, Nominal, Completed, TimelyReference, DegradedOperational, and Complete axes.

The full-mission gate covers reference commanding, inertial continuation, conservative recovery, no action, and invalid/rejected actions. Delayed-valid classification is covered by bounded disposition fixtures; ground-communications blackout and reacquisition remain separately validated by the frozen Phase 11 operational probe. Each case records mission, vehicle, procedure, operator, avionics, and evidence disposition separately. Procedure nonconformance is not a proxy for physical mission failure.

Phase 12B completion acceptance requires inactive operations to reproduce Phase 10 exactly; scripted and UI copies of the same action epochs to yield byte-identical KSB11; presentation choices not to change release or evidence order; invalid actions to fail before state changes; Guided Operator buffers to contain no truth; prediction sources to remain bound and labelled; incomplete history, worker failure, abort, overflow, or failed finalization never to appear complete; exact data to snap across discontinuities; both C++ harnesses to preserve additive ABI-v1 compatibility; and the packaged 2-D desk to complete without Editor, MCP, Python, network, Starter Content, or NASA assets.

The typed Unreal bridge deliberately opens Rust in `Fast` execution-capacity mode so bounded `Advance(n)` requests are not internally reduced by a second realtime scheduler. Unreal is the only owner of wall-clock presentation scheduling for realtime, pause, single-step, 4x, 16x, and maximum-fast modes. The internal capacity choice is noncanonical and emits no pace evidence; validation therefore compares exact release/action transcripts and requires their completed KSB11 bytes to remain identical across presentation schedules.

The role boundary is validated in two layers. Live Guided Operator surfaces are filtered in Rust and contain no SIM Director truth. A completed KSB11 is a sealed, role-neutral post-run archive; it crosses the bridge as opaque bytes for hashing, storage, and the owning Rust verifier, not as a viewer-readable source of live truth.

Phase 12B product acceptance passed both native C++ harnesses, the Unreal Editor target, 17/17 `KSA64.Operations` automation tests, standalone packaging, exact packaged full-mission finalization, accessibility checks, and real-RHI screenshot/semantic evidence. The packaged mission ends at release 21,591 with the accepted 2,911,464-byte KSB11 unchanged.

Automation covers 30/60/144-Hz presentation schedules and proves identical action, release, and KSB11 evidence; it does not claim that every GPU sustains those display rates. The pinned-workstation D3D12 acceptance capture is 1920x1080 at exact release 6,080. A fixed-60-Hz sample with 120 warmup and 600 measured frames advances exactly 320 authoritative releases from 6,144 to 6,464, reports zero queue overflow and no pending command, and records 258,900 ns p99 plus 460,000 ns maximum bridge/presentation service time. These values pass the 1 ms p99 and 2 ms maximum gates, but measure poll/drain/path/enqueue service work rather than total GPU frame time.

The accepted presentation uses high contrast, reduced motion, 1.25 text scale, and disabled sound cues. Clean partial shutdown leaves finalization unfinished rather than falsely marking evidence failed; a completed archive exists only after Rust seals it. The packaged product has no runtime dependency on Editor, MCP, Python, Starter Content, NASA assets, or network services.


## Phase 12B.5 portable-runtime validation

Phase 12B.5 separates software implementation evidence from platform and physical-device qualification. Local Windows acceptance composes the frozen Phase 12B audit with:

- warning-free workspace formatting, Clippy, native tests, and no-default portable-crate builds;
- exact 21,591-release native authority, strict Rust KSB11 replay, independent C/C++ bridge execution, and real exported WebAssembly execution, all producing the accepted 2,911,464-byte KSB11 and SHA-256;
- Rust, TypeScript, and C KPS1 vectors plus corruption, size, sequence, nonce, cursor-gap, overflow, and role-isolation rejection;
- loopback WebSocket admission, authority continuation through disconnect, Noise XX comparison, Noise IK reconnect, immutable role binding, revocation, tamper rejection, and bounded queues/rates;
- a production PWA exercised in WebGPU, forced WebGL2, 2-D-only, and offline-shell modes; and
- Vita host fixtures, shared Noise transport, target compilation, and VPK construction.

Hosted Windows x64, Linux x64, Linux ARM64, and macOS ARM64 exact execution plus WASM worker exactness under the Node harness passed at `aae737c03b8d23e171f77d0b0e95b9dbff22746e` in runs `30326378656` and `30326378684`, including qualified archive generation on all four native hosts. Physical Lenovo Duet 11 and physical Vita evidence cannot be inferred from CI or desktop emulation. Vita3K is useful repeatable emulator evidence but does not replace physical controls, network, suspend/resume, memory, or frame-time measurements. Until the physical Duet, Vita3K emulator, and physical Vita gates pass, the phase is software- and hosted-portable-runtime-qualified but not fully accepted.

Client polling, rendering backend, reconnect, or replay may never become an authority input. A terminated WASM worker, truncated replay, browser suspension, or incomplete device run remains incomplete and cannot fabricate a sealed archive.


## Phase 12C global-display and renderer-parity validation

Status: complete and accepted at source commit `64d72f2a4ee0848bf7ff73c345fcd1cf56579ba1`.

The frozen entry gate requires the Phase 12B.5 entry commit to remain in
history, the 13-entry catalog hash to remain exact, the accepted 21,591-release
four-action KSB11 to remain 2,911,464 bytes with SHA-256
`7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`,
and the nominal KTT10/KPH10/KSR10 hashes to remain unchanged. The independent
physical Duet, Vita3K, and physical Vita qualification backlog is recorded but
not inferred from Phase 12C.

Portable validation covers:

- warnings-denied formatting, Clippy, and the native workspace;
- strict Rust/C++/TypeScript GlobalDisplay vectors and corruption rejection;
- additive KPS1 capability negotiation with legacy 1.0 byte preservation;
- additive C function-table layout, misuse, panic containment, and ABI-v1
  preservation;
- role filtering before native, broker, WebAssembly, or replay boundaries;
- bounded exact range/cursor behavior and explicit resynchronization gaps;
- exact 22,015-release nominal replay and 21,591-release GNSS-loss replay;
- identical normalized display records between live and verified replay;
- transition/event-preserving exact, one-second, and four-second paths;
- one shared in-process/WASM/native path builder, with routine release ticks
  excluded from the pin set and every semantic replay bookmark retained;
- exact live-path release cadence plus the frozen planned source's explicit
  initial point and accepted one-based sparse-sequence cadence;
- path checksum vectors that bind release, Q16 time, segment, event mask,
  anchor, and signed Q12 XYZ rather than geometry alone;
- preservation and presentation of raw stale, incomplete, terminal, and
  resynchronization-required path flags;
- normalization of camera and display frame into one supported shared view mode;
- snap behavior across frames, segments, deployment, invalidity, replacement,
  seek, gaps, and terminal events; and
- action, procedure, disposition, checksum, and KSB11 invariance under display
  polling, backend, camera, layout, and playback changes.

The nominal compatibility audit strictly decodes and hashes both lineages. The
frozen accepted path uses KTT10
`a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a`,
KPH10 `cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2`,
and KSR10
`9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa`.
Current exact re-execution must match its separately reviewed hashes and every
delta bound in `phase12/PHASE12C_NOMINAL_COMPATIBILITY.md`; neither lineage
may be silently substituted for the other.

Renderer acceptance additionally requires:

- `KSA64.Phase12C` Unreal automation with no failed, skipped, or in-process
  tests;
- a packaged Win64/D3D12 viewer that needs no Editor, MCP, or Python;
- construction only after an active packaged game world begins play;
- Launcher Renderer ABI alignment through compile-time `RayTracingMode=Inline`
  while runtime `r.RayTracing=False` remains enforced;
- semantic snapshots at all frame transitions, burnout, apogee, deployment,
  landing, actions, and faults;
- exact source pose, path product, event mask, discontinuity mask, and continuity
  identity parity at all nine nominal milestones;
- the same exact parity at six Guided Operator milestones: GNSS outage onset
  at release 5,760, outage qualification at 5,824, and accepted actions at
  6,080, 6,240, 6,560, and 6,720;
- identical raw path flags, temporal/event-aware point checksums, normalized
  view modes, source availability, release, frame, and dispositions at those
  snapshots;
- real WebGPU, forced-WebGL2, context-loss fallback, and complete 2-D lanes;
- no truth fields in non-director products and persistent truth labelling when
  enabled for a director;
- origin changes with no semantic pose or visible path discontinuity;
- 1920x1080 at 60 fps on the accepted RTX 2080 Super procedural Unreal tier;
- display publication and bridge polling each below 1 ms p99; and
- responsive 30-fps Babylon WebGPU and WebGL2 lanes.

`phase12/complete-phase12c.ps1` reports a portable/contracts PASS when only
the nonvisual gates run. It reports a completion PASS only when every default
gate and explicit Unreal build, automation, package, rendered-browser,
cross-renderer parity, and runtime-evidence input passes. Skipped gates remain
pending. The strict joined record must use
`ksa64.phase12c.cross-renderer-evidence.v2`, be regenerated from the raw
producer artifacts, and match the submitted record byte for byte. The accepted measurements and source-bound evidence are recorded in `phase12/PHASE12C_COMPLETION.md`.

The accepted completion run passed the full frozen audit, native and WASM exact missions, strict replay and corruption gates, both native C++ harnesses, all 74 web tests, the production web build, Unreal Editor build and automation, package/cook, packaged D3D12 runtime, rendered browser evidence, and strict source-bound parity reconstruction. The joined record is `target/phase12c-cross-renderer-64d72f2.json`, schema `ksa64.phase12c.cross-renderer-evidence.v2`, SHA-256 `c869a5dbc341ea6b5272e901882fe803dd2e15f1ab49cbeff48788527c01e50e`.

Accepted parity covers all nine nominal releases 29, 1,920, 3,579, 8,124, 12,669, 15,255, 15,257, 20,929, and 22,014, plus Guided Operator fault/action releases 5,760, 5,824, 6,080, 6,240, 6,560, and 6,720. Nominal replay contains 22,015 releases and four frame transitions. Non-director evidence remains truth-free; the source availability masks are 11 for SIM Director and 3 for Guided Operator, with truth disabled in the captured views.

Measured results satisfy the declared budgets:

- packaged Unreal/D3D12: 192.26 fps at 1920x1080, with 305,300 ns p99 and 366,300 ns maximum scoped display service;
- bridge availability polling: 8,500 ns p99; exact-range polling: 364,600 ns p99;
- Babylon: 71.895 fps WebGPU, 74.384 fps WebGL2, and 75.562 fps complete 2-D fallback, with context-loss fallback passing;
- origin continuity: 35 Unreal changes and one browser change without semantic discontinuity; and
- packaged viewer: 14 immutable files and 958,121,179 bytes excluding `Saved`; production web: 115 files and 3,340,231 bytes.

These measurements are bound to the accepted Windows/RTX 2080 Super and automated browser lanes. They do not qualify the physical Lenovo Duet, Vita3K, or physical Vita. Those Phase 12B.5 gates remain independently open and continue into Phase 12C.5.
