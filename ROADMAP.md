# KSA64 roadmap

This roadmap orders work by learning value and risk reduction. Each phase should remain useful even if later phases never happen.

## Phase 0: feasibility and numeric foundation

Purpose: settle the implementation language and demonstrate trustworthy arithmetic before building simulation features.

Status: complete. Rust/rust-mos, explicit two-word widening, Phase 1 numeric formats, overflow containment, the baseline integrator/timestep, deterministic data formats, and analytic fixtures are accepted and checked.

Planned work:

- Run the rust-mos versus Oscar64 experiment.
- Establish signed integer, overflow, rounding, and saturation rules.
- Perform range analysis for the initial physical quantities.
- Select fixed-point formats from measured ranges rather than convenience.
- Define deterministic scenario and telemetry formats.
- Establish native, emulator, and real-hardware timing procedures.
- Create analytic tests for arithmetic, interpolation, and integration.

Exit criteria:

- One toolchain is selected with a written rationale.
- Native and C64 executions agree for the representative fixed-point workload.
- Numeric error and performance are measured, not estimated.
- No unresolved arithmetic behavior can silently vary by target.

## Phase 1: vertical flight laboratory

Status: complete.

Purpose: create the smallest end-to-end vehicle simulation.

Current result: the production `no_std` core executes the complete validated vertical mission through immutable, fail-closed transitions. Final linked-layout timing measures raw dynamics at 118,111.48 PAL cycles per step (8.34 Hz, 4.10 percent headroom) and checksum plus canonical telemetry at 175,307.68 cycles per step (5.62 Hz). Host capture and strict inspection reproduce the 257-frame stream. A 38,519-byte C64 acceptance PRG reports zero failures, while the 28,353-byte post-run display retains an 80-byte status sink and is verified from screen memory. An 80-digit Decimal comparison separates fixed-point error from integrator bias and supplies the accumulated altitude/velocity deltas displayed by the C64. All exit criteria pass; see `phase1/COMPLETION.md`.

Planned capabilities:

- One-dimensional altitude and vertical velocity.
- Variable vehicle and propellant mass.
- Constant or tabulated thrust and mass flow.
- Altitude-dependent gravity.
- Tabulated atmospheric density.
- Quadratic drag.
- Semi-implicit Euler, followed by RK2 only if measurements justify it.
- Text telemetry on the host and C64.

Exit criteria:

- Analytic special cases pass within declared tolerances.
- Host and C64 results match under the exact-arithmetic mode.
- Results are compared against a high-precision calculation.
- The C64 reports cycle, memory, and accumulated-error measurements.

## Phase 2: two-dimensional multistage ascent

Status: complete. The rotating-Earth planar model, nominal/failure missions, KST2 validation, measured C64 target path, and PETSCII/SID replay pass the completion audit.

Purpose: produce a recognizable launch trajectory and orbital-insertion attempt.

Planned capabilities:

- Altitude and downrange motion.
- Vertical and horizontal velocity.
- Pitch program.
- Multiple stages, ignition, cutoff, and separation.
- Mach-dependent drag coefficients.
- Dynamic pressure and Max-Q detection.
- Spherical, rotating Earth only when required by the selected model.
- Apogee, perigee, specific energy, and orbit classification.
- PETSCII flight-status and trajectory displays.
- SID event and alarm cues.

Exit criteria:

- A configured fictional vehicle can reach a stable or deliberately failed orbit.
- Stage events conserve the quantities required by the model.
- Coast propagation satisfies energy and angular-momentum tolerances.
- Compatible segments agree with independent trajectory tools after assumptions are aligned.

## Phase 3: simulated avionics and closed-loop flight

Status: complete. Transport-isolated sensors, aided navigation, flight phases, closed-loop insertion, actuator monitoring, four deterministic mission cases, KST3/KRP3 validation, and bounded PAL target evidence pass the completion audit.

Purpose: turn the trajectory program into an aerospace simulation architecture.

Planned capabilities:

- Explicit vehicle-truth, sensor, navigation, guidance, control, and actuator boundaries.
- Quantized accelerometer, gyro, altimeter, and clock models.
- Bias, drift, noise, delay, and dropout injection.
- Programmed guidance followed by closed-loop attitude or steering control.
- Flight phases, sequencing, alarms, and abort states.
- Deterministic replay of scenarios and failures.

Exit criteria:

- Flight software cannot read private truth state.
- Sensor and actuator interfaces can be transported without changing the core.
- Nominal and selected failure scenarios are repeatable.
- Control performance is measured against declared stability and tracking criteria.

## Phase 4: adaptive storage and statistical analysis

Status: complete. Deterministic campaigns, independent float64 analysis, stock streaming retention, preserving adaptive REU archives, interactive UI, and strict host/C64 disk export pass the completion audit. Phase 3 run-zero behavior and artifacts remain exact; see `phase4/COMPLETION.md`.

Purpose: add reproducible statistical analysis and capacity-scaled storage while keeping stock C64 operation complete and treating the REU as explicit optional storage rather than ordinary addressable RAM.

Accepted capabilities:

- Keyed deterministic parameter variation and ordered campaign aggregation.
- A 64-run routine campaign and frozen 1,024-run native reference campaign.
- Streaming statistics, five retained stock summaries, and a sparse stock trajectory.
- Preserving REU detection, explicit DMA, and adaptive histories from 128 KiB through 16 MiB.
- Four bounded analysis pages with retained-run browsing and drill-down.
- Strict KSC4/KSR4/KPH4/KST4/KRA4/KXV4 evidence and post-run IEC export.

Exit criteria:

- Simulation behavior is unchanged when recording is disabled.
- REU transfers are explicit and bounded.
- Runs are reproducible from scenario, configuration, and random seed.
- Statistical results can be independently analyzed on the host.

## Phase 5: three-dimensional rigid-body dynamics

Status: complete. Spatial translation, rigid and reduced flexible dynamics, multirate vehicle behavior, strict avionics, truth-isolated guidance, integrated mission and campaign evidence, adaptive history, measured target probes, and stock replay pass the completion audit. See `phase5/COMPLETION.md`.

Purpose: add attitude dynamics only after the translational architecture is stable.

Candidate capabilities:

- Three-dimensional position and velocity.
- Quaternion attitude representation.
- Angular velocity, inertia, forces, and torques.
- Engine gimbal and actuator dynamics.
- Aerodynamic moments.
- Attitude sensors and control.

Entry criteria:

- The 2-D model, test framework, and avionics split are mature.
- Range and performance estimates show that the representation is feasible.
- A specific learning or simulation goal requires 6-DOF.

Exit criteria:

- Standard rigid-body and torque-free cases pass.
- Quaternion normalization and drift are controlled.
- Reduced 6-DOF cases agree with an independent implementation.

## Phase 6: Commodore-in-the-loop

Purpose: split the architecture across physical computers.

Status: software baseline accepted. Exact split execution, deterministic links,
the stock-C64 KSA-6R flight profile, passive ground systems, and a complete
externally paced 1x PAL target flight, and the host/VICE deployment launcher now pass. Live physical-link acceptance
remains open; see `phase6/COMPLETION.md`.

Possible configurations:

- One C64 runs the vehicle world; a second runs flight software.
- A third C64 receives telemetry and acts as mission control.
- Mission control computes an independent trajectory estimate.
- Failure injection can affect links, sensors, avionics, or vehicle systems.

Exit criteria:

- The transport has framing, checksums, timeouts, and deterministic replay.
- Disconnecting or delaying a computer produces defined behavior.
- Single-machine and multi-machine runs can use the same core models.
- Mission-control estimates can be compared with onboard estimates.

## Phase 7: multi-profile mission packs and hobby vertical evaluation

Status: complete. The accepted implementation and measurements are recorded in `phase7/COMPLETION.md`.

Purpose: establish a reusable, explicitly versioned profile and pack boundary,
then prove it with a second physical scale rather than by generalizing the
accepted orbital models in place.

Current result: the Firestorm 54/I211W reference mission executes from ignition
through dual-deploy recovery using one portable exact implementation. Strict
KVP7-KRA7 evidence, an independently checked 1,024-run campaign, stock-C64
replay, a 129-state native/MOS trace, and a complete 17.72-minute PAL target
mission pass. No REU is required.

Accepted capabilities:

- A typed evaluation facade over the frozen KSA-2A, KSA-5A, and new hobby
  vertical profiles.
- Host-compiled, bounded vehicle, motor, mission, campaign, telemetry, summary,
  plot, and archive formats.
- A separate SI/fixed-point numeric contract for model through high-power
  rockets.
- A published-data Firestorm 54 / AeroTech I211W reference configuration.
- Sampled thrust, changing motor mass, launch-rail constraint, powered ascent,
  coast, dual-deploy recovery, and ground contact.
- Stable objective/constraint summaries, ordered candidate grids, and
  deterministic uncertainty campaigns.
- Native, independent high-precision, stock-C64, and VICE evidence without an
  REU requirement.

Exit criteria:

- Every accepted Phase 0-6 artifact remains unchanged.
- Legacy missions execute through the facade without replacing their
  implementations.
- The published-data reference mission completes from ignition through ground
  contact and produces strict KST7/KSR7/KPH7 evidence.
- Native and target exact executions agree for the bounded accepted path.
- Candidate and uncertainty results are independent of worker count,
  presentation, and storage.
- The evaluator exposes metrics without imposing one universal optimization
  score.

## Phase 8: spatial hobby flight and validated vehicle modeling

Status: complete. The accepted implementation and evidence are recorded in `phase8/COMPLETION.md`.

Purpose: add the physical dimensions that require geometry, attitude, wind, and model correlation while preserving the useful vertical profile.

Accepted result:

- Separately versioned `HobbySpatialV1`; `HobbyVerticalV1` remains unchanged.
- Provenance-backed component geometry with derived mass, CG, diagonal inertia, CP, static margin, restoring forces, and damping.
- Local ENU rail constraint, 6-DOF powered/coast flight, layered deterministic wind/gusts, weathercocking, and 3-D dual-deploy recovery.
- Explicit Mach, AoA, and 3 km Firestorm environment envelopes with fail-closed outcomes.
- Strict KVP8/KMP8/KMC8/KWP8/KST8/KSR8/KPH8/KSC8/KRA8 contracts.
- Independent float64 evidence and 19 aligned OpenRocket 24.12 comparisons.
- A 1,024-run deterministic campaign, adaptive stock/REU retention, host exports/plots, and seven-page stock-C64 replay.
- Exact finite native/MOS evidence; the built full target path projects above the 30-minute completion threshold and is not automatically run.

Exit criteria: all pass. Phase 0–7 regressions remain exact; representative mass/stability, trajectory, external comparison, campaign determinism, stock packaging, and finite target gates are accepted.

## Phase 8.5: unified avionics and exact event execution

Status: complete at the explicit stock-fit decision boundary. The accepted record is in `phase8_5/COMPLETION.md`.

Accepted result:

- Non-breaking `VerticalPointMassV1` and `LocalEnu6DofV1` source aliases over frozen serialized identities 3 and 4.
- Exact 32 Hz releases with retained physical deadlines and sensor-N/command-N/effective-N+1 ordering.
- Strict KAP8/KAC8/KLE8/KLR8/KAT8/KAS8/KMR8 identities, codecs, checksums, and fail-closed link behavior.
- Truth-blind local IMU/barometer/GPS/attitude-aid navigation, recovery sequencing, feedback, health, and timer backups.
- Monitor-only original Firestorm plus a separately identified fictional 20 g two-axis-gimbal derivative holding launch-rail attitude.
- Host/host and host-world plus VICE/C64-flight placements with the same live F1-F7 Mission Control presentation.
- A deterministic 64-run campaign and named fault matrix, exact across worker counts.
- A stock-C64 flight endpoint meeting its PAL deadline.
- A measured monolithic stock target requiring 71,500 resident bytes. Banking cannot fit it into 64 KiB, so implementation stopped without silently choosing overlays, feature cuts, an REU, or a separate rewrite.

The frozen Phase 8 truth-triggered executor and standalone stock world remain available. A combined-stock overlay/rewrite/expansion choice is optional follow-up work and does not block the Phase 9 host optimizer.

## Phase 9: design optimization and robustness workbench

Status: implemented. The optimizer consumes the accepted Phase 8.5 avionics-aware evaluation identity and does not introduce a second production simulator.

Accepted result:

- Strict KOM9/KDV9 manifests and canonical materialized candidates.
- Exact nominal, eight-case search, and 64-case finalist robustness tiers.
- Feasibility-first constraints, Pareto ordering, exact grids and sensitivity, deterministic NSGA-II V1, and DE/rand/1/bin.
- Proposal, evaluation, generation, checkpoint/resume, archive, and report bytes independent of one, four, or eight workers.
- KOE9/KRA9/KRE9/KSN9/KFP9 evidence with retained KAS8 case records, strict corruption rejection, and independent Python verification.
- Two accepted Firestorm-derived studies, a coupled demonstration, and an explicitly experimental broad-airframe search excluded from validated physical evidence.
- Persistent bounded JSONL integration for external optimizers.
- Live seven-page host workbench, self-contained HTML/JSON/CSV reporting, and a stock-C64 finalist browser below `$C000`.

The 15,391-byte stock browser builds below `$C000` and its finite one-instance VICE probe validates four finalists bound to manifest `e86077d4`. Selected exact reruns continue through the accepted Phase 8.5 host-world/C64-flight endpoint.

The optimizer selects candidates; the portable core only evaluates them.

## Phase 9.5: advanced control effectors

Status: complete. All twelve gates are accepted: native effectors and workbench, deterministic campaigns/searches, the interim host-world plus externally paced stock-C64-flight baseline, passive advanced Mission Control, adaptive finalist retention, selected-finalist stock reruns, and the bounded completion audit.

Purpose: extend the Phase 8.5 control-allocation boundary with physically modeled aerodynamic and reaction-control effectors, using the Phase 9 workbench to size, tune, compare, and robustly evaluate them.

Target execution policy:

- The interim accessible baseline is host-world plus externally paced stock-C64 flight over strict KLF6/KLR9 step-and-ack cells. It proves target execution and exact closed-loop behavior, not wall-clock realtime fitness.
- Realtime stock-C64 flight remains a priority optimization track. Preserve the measured PAL deficit and investigate only measured changes, including a 6502-specific rewrite of hot kernels and C64 Ultimate acceleration/integration.
- C64-world execution remains a priority long-run role but temporarily follows the host-world baseline while the remaining software layers mature. It is not removed, converted into a second simulator, or made dependent on an REU.
- Selected optimized candidates use a strict KFB9 startup bootstrap to configure a separate stock flight image. The frozen reference endpoint and all candidate identities remain unchanged.
- A passive F1–F7 host Mission Control layer and KMR9 recording observe either host or C64 flight placement without entering evaluation identity.

Candidate capabilities:

- Geometry-, Mach-, angle-of-attack-, and dynamic-pressure-aware aerodynamic canards with bounded deflection, slew, lag, force/moment, drag, hinge-load, authority, and model-envelope contracts.
- Cold-gas RCS thruster sets with explicit placement, controlled axes, valve and minimum-impulse behavior, deterministic pulse allocation, consumable depletion, and changing mass properties.
- Phase- and regime-aware control allocation across motor gimbal, canards, and RCS, including blended vehicles and deterministic authority handoff.
- Host-compiled effector packs, capability identities, controller/allocation profiles, strict evidence, independent physical checks, and target-bounded exact execution.
- Phase 9 parameter search, Pareto analysis, sensitivity work, and nested uncertainty campaigns over effector sizing, placement, authority, consumables, and controller settings.

Validation policy:

- KSA64 remains the sole runtime authority for canard, RCS, depletion, mass-property, actuator, allocation, and authority-handoff behavior.
- Analytic cases and a small independent float64 implementation are the primary physical and numerical evidence.
- Basilisk may generate optional, frozen secondary fixtures for selected fixed-step spacecraft-attitude, RCS force/torque, pulse, depletion, and mass-property cases. It is not an oracle for KSA64 canard aerodynamics, exact-event scheduling, mixed-effector allocation, or authority handoff.
- Normal builds, tests, and CI require neither Basilisk nor network access. Every external fixture is versioned and records the generating tool, input, configuration, tolerance, and content hash.
- Phase 9.5 does not add optional Phase 10 frame/time tooling merely to prepare for global flight.

Exit criteria:

- The shared Phase 8.5 scheduler, navigation, guidance, sequencing, command timing, and truth boundary remain unchanged.
- Canard and RCS force, torque, lag, saturation, depletion, mass-property, and failure cases agree with independent analytic or float64 references within declared tolerances.
- Any retained Basilisk comparison is reproducible from pinned inputs and remains corroborating evidence rather than a runtime dependency or acceptance authority.
- Gimbal-only, canard-only, RCS-only, and at least one mixed-effector mission are deterministic across worker count, placement, recording, and replay.
- The Phase 9 optimizer can compare and robustly evaluate effector designs without receiving private truth or changing the production evaluator.
- Unsupported capability combinations and operation outside a validated aerodynamic or consumable envelope fail closed.

## Phase 10: global atmospheric and suborbital flight

Status: complete. All twelve gates are accepted.

Purpose: cover the region where local and orbital missions overlap without forcing all vehicles into one coordinate representation.

Accepted result:

- WGS 84, elapsed TAI, pinned leap/EOP data, central-plus-J2 gravity, compiled IERS/IAU transforms, and compiled U.S. Standard Atmosphere profiles.
- Explicit release-bound ownership across local ENU, ECEF atmospheric flight, GCRF coast, ECEF entry, and local recovery.
- The controlled KSA-G10R mission completes all transitions and recovery; the independent uninstrumented float64 model passes its physical and event tolerances.
- A frozen KSA-5A insertion handoff completes a bounded one-orbit global coast.
- Seed `0x4b5341a0` 64/256-case archives are byte-identical at one, four, and eight workers; all 256 cases recover without numeric/frame/time faults.
- F1–F7 global Mission Control, strict reports, and stock trajectory replay are passive.
- The no-REU stock flight endpoint fits below `$C000` and exact-matches finite host/VICE release and transition probes. It is externally paced, not realtime.

Accepted capabilities:

- A separately versioned `GlobalEcef6DofV1` profile for rotating-Earth, long-range atmospheric and suborbital flight.
- Strict local-ENU, Earth-fixed, and Earth-inertial state transforms with continuous position, velocity, attitude, angular rate, time, and identity.
- Mission-declared deterministic frame transitions and local launch/recovery views around a global authoritative trajectory.
- Reuse of the Phase 8.5 avionics boundary so the same flight computer can navigate local, Earth-fixed, or Earth-inertial missions through explicit frame-aware aiding and guidance.
- Reuse of Phase 9.5 actuator capabilities and control allocators without coupling global coordinates to a particular effector family.
- Native, independent high-precision, bounded C64, and external reference evidence appropriate to the expanded model envelope.

Validation authority and external-reference strategy:

1. The portable deterministic `GlobalEcef6DofV1` implementation is authoritative for KSA64 behavior.
2. An independent float64 implementation covers the complete accepted atmospheric/global six-degree-of-freedom model and is the primary numerical reference.
3. SatKit is the preferred specialized host-side fixture generator for time scales, Earth orientation, frame transformations, gravity, and selected ballistic or orbital coast cases.
4. Orekit is the escalation path when SatKit lacks a required transform derivative, epoch/data behavior, or independently useful coverage. It is not a mandatory dependency from the beginning.
5. GMAT supplies occasional independent exoatmospheric and near-orbital trajectory cross-checks. It does not define atmospheric forces, frame ownership, or canonical KSA64 results.

SatKit, Orekit, and GMAT generate frozen evidence only. Normal tests and CI require no external tool, network access, or live Earth-orientation or leap-second data. Only one model owns an entity's state during an interval; external tools never co-propagate or correct the production trajectory at runtime.

Accepted Earth/time/frame contract:

- Declare the reference ellipsoid, gravity model, Earth-rotation/orientation and any precession/nutation model, supported time scales, continuous internal integration scale, leap-second source, Earth-orientation-data source and validity window, extrapolation/failure policy, and every permitted simplification.
- Compile accepted leap-second and Earth-orientation inputs into versioned offline data. UTC is an input/output representation, not a discontinuous integration clock across a leap second.
- Freeze transform direction, axis, quaternion, angular-rate, velocity-transport, and epoch conventions before accepting force or trajectory comparisons.
- Do not select higher-fidelity Earth/time models by prestige alone. Phase 10 range and accuracy analysis chooses the smallest declared model that satisfies its mission envelope, then versions that choice.

Accepted transform and transition evidence:

- Test multiple epochs, including leap-second and Earth-orientation-data boundaries plus an explicit out-of-coverage failure case.
- Test the equator, both sides of the date line, high altitude, near both poles, and the exact poles. Exact-pole local frames must declare a reference meridian/longitude because ENU heading is otherwise ambiguous.
- Prove round trips and mission transitions preserve position, velocity, attitude, angular rate, and simulation time within declared tolerances across ENU/ECEF/ECI boundaries.
- Compare quaternion attitude as a physical rotation so equivalent `q` and `-q` encodings do not create a false failure.
- Separate frame/time-only fixtures, force snapshots, one-step transition cases, and integrated trajectories. Evidence reports must attribute disagreement to frame/time conventions, force/environment models, or numerical integration rather than collapsing them into one trajectory delta.
- Record fixture provenance: tool and version, source-data versions and hashes, inputs, epoch/time scales, frames and transform direction, Earth/gravity/atmosphere configuration, raw output, conversion script, tolerance rationale, fixture hash, and regeneration instructions.

The vehicle's organizational category never selects this profile automatically. A small sounding rocket may require it, while a large vehicle may legitimately use a local profile for a bounded launch-site experiment.

## Phase 11: mission operations and programmable flight

Status: complete. The accepted implementation and evidence are recorded in
`phase11/COMPLETION.md`.

Purpose: turn the accepted simulator into an operable and programmable mission
system without adding a new physical model. Phase 11 freezes the contracts that
future graphical authoring, procedures, user flight software, and sustained
spacecraft missions will consume.

Planned architecture:

- Formalize a `FlightSoftwarePackage` envelope binding package identity,
  compatible KLR contract and capabilities, release schedule, persistent
  memory, safe-state behavior, and target-specific resource evidence. KLR8,
  KLR9, and KLR10 remain separately versioned sensor/command contracts; Phase
  11 does not invent one magical universal sensor ABI.
- Support interchangeable reference, user-native, rust-mos/6502, and external
  hardware flight endpoints through the existing KLF6 boundary. A bytecode VM
  or friendly language remains optional until a measured use case justifies
  its interpreter and toolchain cost.
- Add a versioned mission-plan contract containing planned guidance events,
  operator decision points, and declared prediction-model identities.
- Add separately identified onboard-estimate and ground-estimate prediction
  services for event times, frame transitions, apsides, ground track, and
  atmospheric impact. SIM-truth prediction remains a SIM Director-only
  counterfactual and never masquerades as an independent operational estimate.
- Add an epoch-tagged operator/uplink command protocol. Procedures and UI code
  may request only declared actions; flight software and world authority retain
  their existing power to validate, reject, safe, and record them.
- Add deterministic procedure packs with public operational-data guards,
  simulated-time timeouts, branches, cautions, acknowledgements, permitted
  actions, scripted regression inputs, and exact replay.
- Add Observer, Guided Operator, Flight Controller, Flight-Software Engineer,
  and SIM Director roles over one authoritative run. These are responsibility
  and information boundaries, not physics difficulty levels.
- Add an immutable mission-session bundle binding vehicle/environment/mission,
  flight software, plan, procedures, faults, operator actions, canonical
  telemetry, predictions, and deterministic debrief evidence.
- Add a headless mission-authoring SDK, schema validation, pack compilation,
  source/provenance ledger, and test harness. Graphical authoring is Phase 12.
- Extend Phase 9 sensitivity and counterfactual evidence into model-derived
  design explanations and debriefs. Reports distinguish observations,
  hypotheses, and causal evidence and never turn assumption-backed campaign
  fractions into real-world reliability claims.

Reference operations scenario:

- Reuse the accepted KSA-G10R physical world unchanged.
- Demonstrate nominal operations and a deterministic GNSS-loss procedure using
  only transported measurements, ground estimates, prediction residuals, and
  authorized epoch-tagged actions.
- Replay the complete human or scripted action log to identical mission,
  procedure, prediction, and checksum evidence.
- Prove at least one alternative flight-software package crosses the same
  profile-specific ABI and failure boundary as the reference implementation.

Target implementation status:

- `SafeholdRecoveryV1` fits as an independent flat stock-C64 image.
- The complete portable reference package exceeded the flat below-`$C000`
  boundary, so the authorized stopgap uses explicit stock-RAM banking beneath
  I/O and KERNAL. The one-instance, warp-disabled VICE gate exact-matches 13
  native operations without an REU; it is externally paced and not realtime.
- Physical loading/link acceptance, a 6502-specific rewrite, C64 Ultimate
  acceleration, and the portable C64 world remain parallel target tracks.

Exit criteria:

- Existing Phase 0-10 artifacts remain exact.
- Execution placement, operator role, hints, procedure presentation, and
  prediction display cannot alter physics except through explicit recorded
  commands.
- Onboard and ground predictors start from their own estimates and identify
  their models; neither receives private truth.
- Procedure time is simulation time, so a paused or externally paced C64 does
  not create a false deadline or timeout.
- Package, plan, procedure, action-log, prediction, and session corruption
  fail closed.
- Reference and alternative flight packages produce bounded, attributable
  resource and timing evidence on every claimed target.

Explicit non-goals:

- No new atmosphere, gravity, vehicle, subsystem, n-body, or entry physics.
- No universal spacecraft scripting language or bytecode VM requirement.
- No graphical 3-D vehicle editor, arbitrary code plugins, or authoring on the
  C64.
- No claim that an evidence ledger is certification, launch approval, or a
  calibrated real-world probability of success.

## Phase 11.5: product consolidation and unified application shell

Status: complete and accepted on 2026-07-26. See `phase11_5/COMPLETION.md`.

Purpose: make the accepted simulation, operations, workbench, evidence, and target capabilities discoverable as one host product without changing authoritative models or frozen artifacts.

Delivered:

- One default `ksa64` executable and a shared public Rust application facade.
- A deterministic two-tier catalog with 13 current experiences, seven C64 targets, and opt-in Phase 0–11 historical tools.
- Unified project, mission, campaign, optimization, evidence, target, and audit commands using stable domain IDs.
- Guided KSA-G10R GNSS-loss operations as the flagship quick start.
- Direct Phase 12 Rust APIs; Mission Foundry does not need to parse CLI output or spawn phase binaries.
- Exact Phase 11 and legacy telemetry compatibility wrappers retained through at least Phase 13.
- Explicit stored-versus-live target dispatch preserving one-instance, warp-disabled, cooldown, cleanup, and long-run confirmation policy.
- Pre-Phase 12 hardening: focused domain adapters, a complete nested request family with safety metadata, separate accepted-product, authored-project, and session domains, and a deterministic incremental GNSS-loss operations session with exact KSB11 finalization.

Accepted evidence:

- Every Phase 0–11 frozen artifact remains unchanged.
- Unified and Phase 11 compatibility sessions are byte-identical.
- Catalog and quick-start output are deterministic.
- Stored verification cannot launch VICE, and live probes require an explicit flag.
- rust-mos packaging and the finite safehold/banked VICE probes remain exact with no new C64 program.

Explicit non-goals remain new physics, avionics, formats, target implementations, GUI, 3-D viewer, repository migration, REU overlay, Ultimate acceleration, physical-link acceptance, and the deferred 6502 rewrite.

## Phase 12: Unreal feasibility, passive operations, and Mission Foundry

Status: active. Phase 12A is complete and accepted; Phase 12B is the next
implementation slice. Phase 11.5 and its accepted hardening amendment remain
the frozen product, application, and live-session foundation. Phase 12A
completion evidence is in `phase12/COMPLETION.md`, with the next boundary in
`phase12/PHASE12B_HANDOFF.md`.

The hard entry criterion applies to every Phase 12 subphase: a graphical client
operates a live mission only through `LiveMissionSession`. It may not build
another execution loop, drive flight packages directly, or present completed
evidence as live operation. Unreal owns presentation and wall-clock scheduling;
Rust owns mission state, role filtering, actions, and evidence.

### Phase 12A: Unreal toolchain and live-bridge feasibility

Status: complete and accepted. See `phase12/COMPLETION.md`.

Purpose: prove on native Windows that a pinned Unreal Engine 5.8 Launcher build
can consume `Ksa64Application` through a versioned, failure-contained C ABI
without changing any accepted Phase 0–11.5 artifact.

The bounded slice includes the Windows toolchain lock, short-path checkout,
empty C++ Unreal project, Git LFS/source policy, Rust bridge, independent native
C++ harness, minimal runtime plugin, packaged smoke test, and one supervised
loopback-only MCP inspection/mutation experiment. It adds no renderer, scene
graph, coordinate conversion, visual interpolation, authoring UI, NASA asset,
new K-format, physics, or alternate simulator.

Accepted result: the 13-entry catalog and guided GNSS-loss KSB11 remain exact;
the versioned Rust/C ABI contains failures and enforces role filtering; the
independent native harness and Unreal automation pass; and the packaged
Development build loads the commit-qualified bridge with Editor, MCP, Python,
and editor-only toolsets disabled. The accepted shell still contains no renderer,
scene graph, coordinate conversion, visual interpolation, authoring UI, NASA
asset, new K-format, physics, or alternate simulator.

### Phase 12B: live GNSS-loss operations presentation

Status: next; ready to plan from the accepted Phase 12A handoff.

Purpose: add the first bidirectional graphical operations slice over the
accepted live-session boundary.

The guided-operator GNSS-loss experience owns role-filtered live snapshots,
procedures, operator forms, stage/validate/commit actions, pacing, exact and
smooth presentation, simple orbit visualization, and exact KSB11 finalization.
It proves live operations and evidence fidelity, not a complete physical
mission or local-to-global world presentation.

### Phase 12C: complete global engineering viewer

Status: planned after 12B acceptance.

Purpose: replay the complete accepted Phase 10 KSA-G10R mission to prove the
engineering-viewer responsibilities that the short GNSS-loss coast scenario
does not exercise.

This phase owns ENU/ECEF/GCRF display conversion, large-world display domains,
Earth rendering, vehicle pose and component-event presentation, exact event
snapping, trajectory-source labels, entry, recovery, fixed screenshot
regressions, and packaged performance. Rust telemetry and recordings remain
authoritative; cameras, interpolation, effects, and rendering are passive.

### Phase 12D: Mission Foundry authoring and compiler parity

Status: planned after the viewer boundaries are accepted.

Purpose: provide a KSP/Juno-inspired host authoring experience while keeping
compiled packs, authority lanes, provenance, and evidence maturity distinct.

Planned capabilities include a bounded component tree and attachment workflow,
vehicle integration and derived engineering overlays, world/flight-software/
ground/SIM Director mission lanes, avionics binding, and explicit Sketch,
Evaluated, and Frozen Candidate states. GUI and headless compilation of the same
source must produce byte-identical packs and derivation ledgers. Editing a
source creates a new identity and never mutates frozen evidence.

### Phase 12E: production visual baseline

Status: planned after 12D.

Purpose: establish reviewable KSA-G10R and Firestorm visual assets, NASA-derived
visual reference material, materials, lighting, bounded Niagara effects,
quality tiers, and packaged performance gates.

Open-format masters and complete provenance remain source authority for visual
assets; `.uasset` and `.umap` files are generated target assets. NASA material
is visual/reference input only and must declare
`engineering_authority: false`.

### Shared Phase 12 limits

- No universal CAD, CFD, FEA, or arbitrary plug-in execution system.
- No hidden automatic wiring of mechanical, propellant, power, data, or
  control interfaces.
- No unrecorded HOTAS/manual-control path. A later manual-input source must use
  the Phase 11 epoch-tagged command contract and deterministic replay.
- The stock C64 consumes compiled packs and replay products; it is not required
  to parse authoring projects or render the 3-D scene.

## Parallel target-engineering track

Stock-C64 execution remains a project priority but does not block the
host-authoring phases:

- profile and rewrite measured global/advanced hot kernels for the 6502;
- evaluate C64 Ultimate acceleration without changing canonical behavior;
- restore a portable C64-world long-run role;
- accept physical user-port, ACIA, and Ethernet transports;
- use the Phase 11 package ABI to compare reference and target-specific flight
  implementations without creating a second simulator.

No optimization may silently lower rates, remove capabilities, require an REU,
or change accepted evidence.

## Candidate Phase 13: sustained orbital spacecraft and bounded systems

Phase 13 requires a separately reviewed concrete mission before its scope is
locked. The preferred experiment is one sustained Earth-orbital spacecraft
using the Phase 11 operations contracts and Phase 12 authoring workflow.

That mission may justify a compiled bounded subsystem slice containing one
electrical bus, battery/generation, avionics and communications loads,
propulsion valves, a small thermal state, and two or three meaningful
degradation/redundancy chains. It may also select a bounded subset of sustained
propagation, ground tracking, deorbit guidance, `SixAxisWrenchV1`, rendezvous,
docking, or station keeping.

The mission must define which capabilities are necessary; Phase 13 does not
begin as a universal spacecraft-systems graph.

## Later mission-driven backlog

- Runtime empirical atmosphere and higher-order Earth environment models only
  when a selected mission's error budget requires them.
- Lifting entry, thermal protection, ablation, precision landing, and
  propulsive landing only for a concrete entry/landing experiment.
- Multiple central bodies, n-body dynamics, lunar flight, Lagrange-point
  operations, and interplanetary propagation only after a mission defines the
  required ephemeris, frame/time, navigation, and validation contracts.
- An engineering-program layer may later derive evidence maturity, unresolved
  anomalies, campaign coverage, and hardware-in-the-loop status from immutable
  artifacts. It must not award arbitrary technology points or present
  assumption-backed campaign fractions as calibrated reliability.
