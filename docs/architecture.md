# Architecture

## Status

This document describes the intended architecture before implementation. Accepted choices are recorded in `docs/decisions.md`; later phases may replace a model only through an explicit versioned decision.

## System qualities

KSA64 prioritizes:

1. Determinism.
2. Physical coherence within declared model assumptions.
3. Verifiability.
4. Portability between native and C64 builds.
5. Explicit resource costs.
6. Modular fidelity.
7. Retro-hardware character without retro-themed fakery.

Visual fidelity, real-time execution, and feature count rank below those qualities.

## Context

The system separates the simulated world from the software that attempts to fly through it.

    Scenario and vehicle data
                |
                v
    +---------------------------+
    | Vehicle world             |
    | environment and dynamics  |
    +-------------+-------------+
                  |
             truth state
                  |
                  v
    +---------------------------+
    | Sensors                   |
    | noise, bias, quantization |
    +-------------+-------------+
                  |
          measurements only
                  |
                  v
    +---------------------------+
    | Flight software           |
    | navigation, guidance,     |
    | control, sequencing       |
    +-------------+-------------+
                  |
              commands
                  |
                  v
    +---------------------------+
    | Actuators and propulsion  |
    +-------------+-------------+
                  |
                  +---------> vehicle world

Telemetry observes these boundaries but does not become part of the physics.

## Deployment forms

### Native host

The host build exists for rapid execution, automated tests, detailed logging, scenario generation, and comparison with external tools. In exact mode it uses the same fixed-point types and step ordering as the C64.

### Single C64

The first C64 build contains the portable core plus platform-specific input, display, sound, storage, timing, and optional REU support.

### Multiple C64s

Later deployments may move flight software and mission control onto separate machines. That split must use the same sensor, command, and telemetry interfaces established in the single-machine program.

## Major layers

### Numeric layer

Responsibilities:

- Fixed-point representations.
- Widening multiply, scaled divide, rounding, and saturation.
- Binary-angle representation and trigonometric tables.
- Interpolation primitives.
- Vector and matrix operations when later phases require them.

Rules:

- No implicit dependence on host integer widths.
- Overflow behavior must be explicit.
- Intermediate widths must be documented.
- Numeric wrappers should compile away.
- Formats are selected per physical quantity after range analysis.

One universal fixed-point format is unlikely to serve altitude, density, mass, angle, and acceleration safely.

### Model layer

Responsibilities:

- Gravity.
- Atmosphere and wind.
- Aerodynamics.
- Propulsion and mass flow.
- Staging and configuration changes.
- Translational and, eventually, rotational dynamics.
- Numerical integration.

Models should expose their assumptions. A simple spherical-Earth model is not a defect if the scenario and validation tolerances say that it is the chosen model.

### Avionics layer

Responsibilities:

- Sensor production.
- Navigation estimates.
- Guidance targets.
- Control commands.
- Actuator response.
- Flight-phase sequencing.
- Fault detection and abort behavior.

The avionics layer receives measurements and configuration, not direct access to private truth state.

### Application layer

Responsibilities:

- Scenario selection.
- Simulation clock and run control.
- Display pages.
- Telemetry recording.
- Failure injection.
- Batch and replay modes.

### Platform layer

Host and C64 platform code provide services without leaking platform details into the core.

Host services may include:

- File-backed scenarios.
- CSV or binary telemetry.
- Test runners.
- External-reference adapters.
- Profiling and diagnostics.

C64 services may include:

- VIC-II text and bitmap output.
- SID alarms.
- Keyboard and joystick input.
- CIA timing.
- Disk access.
- REU DMA.
- User-port communication.

## Simulation step

The exact order will be fixed before implementation because changing it can change results. The intended high-level sequence is:

1. Apply scheduled events and current actuator states.
2. Evaluate environment at the current truth state.
3. Evaluate propulsion, mass properties, aerodynamics, forces, and moments.
4. Integrate truth state over the timestep.
5. Generate sensor measurements from truth and sensor state.
6. Advance navigation, guidance, control, and sequencing at their configured rates.
7. Update actuator commands and internal states.
8. Emit telemetry and event records.

Subsystems may operate at different rates later, but the first implementation should use one fixed step unless measurement proves that a multirate design is needed.

## Time and determinism

- Simulation time is independent of wall-clock time.
- The baseline uses a fixed timestep.
- Random variation uses an explicit, portable pseudorandom generator and recorded seed.
- Inputs, events, and failures are scheduled in simulation time.
- Exact host and C64 modes must share operation order, table data, and rounding.
- Rendering, telemetry, and storage may run less frequently than physics.

The C64 may calculate slower than real time. That is an acceptable outcome.

## Data strategy

### Ordinary RAM

Keep hot data local:

- Current vehicle and integrator state.
- Current sensor, avionics, and actuator state.
- Frequently accessed table windows.
- Display working state.
- Communication buffers.

### REU

Treat the REU as an explicit backing store:

- Atmospheric and aerodynamic tables.
- Engine curves.
- Scenario data.
- Telemetry history.
- Batch results.
- Saved trajectories.

REU transfers should be coarse enough that DMA setup does not dominate the timestep. The portable core should not pretend REU memory is an ordinary pointer.

### Generated data

Expensive tables may be generated on the host, checked into the project in a deterministic form, and consumed by both targets. Each generated artifact should record units, scale, source model, valid range, and generation version.

## Numerical integration

Phase 1 uses semi-implicit Euler at a fixed 0.125-second step because it is cheap and its analytic error is now measured. RK2 is the next candidate if the completed vertical model shows enough error reduction to justify the additional model evaluation.

Integrator selection is model- and phase-specific. RK4 or adaptive integration is not automatically better on a machine where each force evaluation is expensive and exact cross-target behavior matters.

The timestep and integrator must be tested together. No trajectory result is meaningful without both.

## Portable-core boundary

The core should avoid:

- Heap allocation.
- Filesystem and console assumptions.
- Host floating point in exact mode.
- Platform clocks.
- Unspecified integer conversions.
- Recursion unless bounded and measured.
- Large hidden stack objects.
- Dynamic dispatch in hot paths.
- C64 memory-mapped I/O.

The host may provide a separate high-precision comparison path, but that path is a test aid rather than the product core.

## Planned source layout

Rust/rust-mos and the Phase 1 numeric foundation are selected. Phase 1 now has a production `core/` crate; later subsystems will extend this shape:

    core/
        numeric
        environment
        vehicle
        dynamics
        avionics
        scenario
        telemetry schema

    platform/
        host
        c64

    tests/
        analytic
        regression
        cross-target
        external reference

    tools/
        table generation
        telemetry comparison
        benchmark automation

The numeric contract, overflow policy, baseline integrator, and data formats are accepted in `phase0/numeric/FOUNDATION.md` and `docs/data-formats.md`. The production `core/` crate implements the numeric layer, scenario parser, generated environment sampler, immutable vertical truth, pure force evaluation, fail-closed semi-implicit-Euler transitions, and deterministic mission execution with a compact summary. Common-clock production timing is recorded in `phase1/TIMING.md`. Exact interpolation and acceleration-division fast paths put checked dynamics inside the raw PAL 8 Hz budget; canonical telemetry serialization is the next Phase 1 boundary.

## Extension path

Fidelity should increase through replaceable models:

    constant gravity
        -> altitude-dependent spherical gravity
        -> rotating spherical Earth
        -> higher-fidelity environment if a mission needs it

    constant density
        -> tabulated atmosphere
        -> winds and perturbations

    programmed pitch
        -> closed-loop steering
        -> rigid-body attitude control

    ideal measurements
        -> quantization
        -> bias, drift, noise, delay, and failures

The interfaces should allow these replacements without forcing all later complexity into Phase 1.

## Principal risks

- General 32-bit division or 64-bit intermediates may dominate execution time.
- Fixed-point range choices may conflict across ascent and orbital regimes.
- Compiler-generated stack and zero-page use may constrain the architecture.
- Display and telemetry work may disturb simulation timing if tightly coupled.
- A bit-identical host build can faithfully reproduce a physically wrong model.
- External tools may disagree because of hidden differences in frames, constants, atmosphere, or conventions.
- Premature 6-DOF work could consume the project before a useful launch simulation exists.

The experiment and validation documents address these risks directly.

