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

Status: ready to plan. Optimization contracts consume the accepted Phase 8.5 avionics-aware evaluation identity rather than the Phase 8 truth-triggered boundary alone.

Purpose: build host-side search and analysis around the portable evaluator
without introducing a second production simulator.

Candidate capabilities:

- Parameter-grid, evolutionary, and Pareto-front search.
- Constraint policies, sensitivity analysis, robust objectives, and nested
  uncertainty campaigns.
- Reproducible optimization manifests, resumable archives, and selected
  trajectory retention.
- Rich host visualization plus stock/REU-scaled C64 browsing and replay of
  finalists.
- External optimizer adapters that submit compiled candidates through the
  stable evaluation contract.

The optimizer selects candidates; the portable core only evaluates them.

## Phase 9.5: advanced control effectors

Status: planned after the Phase 9 workbench and before Phase 10 global flight.

Purpose: extend the Phase 8.5 control-allocation boundary with physically modeled aerodynamic and reaction-control effectors, using the Phase 9 workbench to size, tune, compare, and robustly evaluate them.

Candidate capabilities:

- Geometry-, Mach-, angle-of-attack-, and dynamic-pressure-aware aerodynamic canards with bounded deflection, slew, lag, force/moment, drag, hinge-load, authority, and model-envelope contracts.
- Cold-gas RCS thruster sets with explicit placement, controlled axes, valve and minimum-impulse behavior, deterministic pulse allocation, consumable depletion, and changing mass properties.
- Phase- and regime-aware control allocation across motor gimbal, canards, and RCS, including blended vehicles and deterministic authority handoff.
- Host-compiled effector packs, capability identities, controller/allocation profiles, strict evidence, independent physical checks, and target-bounded exact execution.
- Phase 9 parameter search, Pareto analysis, sensitivity work, and nested uncertainty campaigns over effector sizing, placement, authority, consumables, and controller settings.

Exit criteria:

- The shared Phase 8.5 scheduler, navigation, guidance, sequencing, command timing, and truth boundary remain unchanged.
- Canard and RCS force, torque, lag, saturation, depletion, mass-property, and failure cases agree with independent analytic or float64 references within declared tolerances.
- Gimbal-only, canard-only, RCS-only, and at least one mixed-effector mission are deterministic across worker count, placement, recording, and replay.
- The Phase 9 optimizer can compare and robustly evaluate effector designs without receiving private truth or changing the production evaluator.
- Unsupported capability combinations and operation outside a validated aerodynamic or consumable envelope fail closed.

## Phase 10: global atmospheric and suborbital flight

Status: planned after Phase 9.5 establishes the advanced control-effector library.

Purpose: cover the region where local and orbital missions overlap without forcing all vehicles into one coordinate representation.

Candidate capabilities:

- A separately versioned `GlobalEcef6DofV1` profile for rotating-Earth, long-range atmospheric and suborbital flight.
- Strict local-ENU, Earth-fixed, and Earth-inertial state transforms with continuous position, velocity, attitude, angular rate, time, and identity.
- Mission-declared deterministic frame transitions and local launch/recovery views around a global authoritative trajectory.
- Reuse of the Phase 8.5 avionics boundary so the same flight computer can navigate local, Earth-fixed, or Earth-inertial missions through explicit frame-aware aiding and guidance.
- Reuse of Phase 9.5 actuator capabilities and control allocators without coupling global coordinates to a particular effector family.
- Native, independent high-precision, bounded C64, and external reference evidence appropriate to the expanded model envelope.

The vehicle's organizational category never selects this profile automatically. A small sounding rocket may require it, while a large vehicle may legitimately use a local profile for a bounded launch-site experiment.

## Post-Phase-10 mission backlog

The profile architecture may later support multiple central bodies,
rendezvous, deorbit, entry, landing, tracking, and pass prediction. These
missions remain deliberately unassigned until a concrete experiment can define
their required fidelity and evidence.
