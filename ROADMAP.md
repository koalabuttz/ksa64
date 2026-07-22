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

Purpose: create the smallest end-to-end vehicle simulation.

Current progress: the production `no_std` core executes the complete validated vertical mission through immutable, fail-closed transitions. Three stable PAL common-clock runs measure checked dynamics at 160,904.64 cycles per step and per-successor checksum validation at 210,410.64; neither yet fits the provisional 123,156-cycle 8 Hz budget. Exact environment-division specialization is next; telemetry serialization has not begun.

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

## Phase 4: REU telemetry and statistical analysis

Purpose: use expanded storage without pretending the REU is ordinary addressable RAM.

Planned capabilities:

- Buffered transfer between main RAM and REU.
- Telemetry histories and post-flight plots.
- Scenario tables and aerodynamic databases stored outside hot memory.
- Seeded parameter variation.
- Batch execution and compact Monte Carlo summaries.

Exit criteria:

- Simulation behavior is unchanged when recording is disabled.
- REU transfers are explicit and bounded.
- Runs are reproducible from scenario, configuration, and random seed.
- Statistical results can be independently analyzed on the host.

## Phase 5: three-dimensional rigid-body dynamics

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

## Phase 7: mission framework

Purpose: make the system reusable rather than hardwired to one launch.

Candidate capabilities:

- Data-driven vehicles and mission programs.
- Saved scenarios and replay.
- Multiple central bodies.
- Rendezvous, deorbit, entry, or landing experiments.
- Ground tracking and pass prediction.
- Shareable validation packs.

This phase is intentionally open-ended. It should not distort the earlier architecture until concrete missions require it.

