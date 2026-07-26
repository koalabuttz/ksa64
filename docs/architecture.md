# Architecture

## Status

This document describes both the implemented architecture through Phase 4 and the extension boundaries for later phases. Accepted choices are recorded in `docs/decisions.md`; later phases may replace a model only through an explicit versioned decision.

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

## Source layout

Rust/rust-mos and the Phase 1 numeric foundation are selected. The workspace now separates portable physics, stable transports, truth-blind flight software, host adapters, and the composition root:

    core/       portable fixed-point physics, formats, campaigns, storage contracts
    interface/  fixed-width sensor, actuator, and flight-output transports
    flight/     truth-blind navigation, guidance, sequencing, and abort logic
    sim/        world/sensor/flight composition and parameterized mission execution
    host/       capture, inspection, independent-analysis support, and export adapters
    phase*/     phase contracts, reference tools, target probes, and frozen evidence
    toolchains/ pinned compiler and emulator configuration

The numeric contract, overflow policy, baseline integrator, and data formats are accepted in `phase0/numeric/FOUNDATION.md` and `docs/data-formats.md`. The production `core/` crate implements the numeric layer, scenario parser, generated environment sampler, immutable vertical truth, pure force evaluation, fail-closed semi-implicit-Euler transitions, and deterministic mission execution with a compact summary. Common-clock production timing is recorded in `phase1/TIMING.md`. Exact interpolation and acceleration-division fast paths put checked dynamics inside the raw PAL 8 Hz budget. Canonical allocation-free telemetry records are now scheduled by an observer on the single checked executor and delivered through caller-provided sinks. Initial, stride, terminal, accumulated-event, and numeric-fault behavior matches an independent 257-frame stream oracle. A volatile discard sink forces every canonical byte to materialize during target timing: final-layout telemetry adds 7,504.00 cycles per physics step over checksum mode, while the complete path reaches 5.62 Hz. The host adapter captures canonical files, uses the portable record decoder, enforces stream-level cadence and terminal semantics, and renders a compact summary. The C64 adapter stores one canonical header, the latest frame, a frame count, and accumulated event bits in an 80-byte sink, then renders a direct 40x25 post-run page. Display rendering occurs after the measured mission, and a VICE screen-memory oracle verifies its final contents. A separate 80-digit Decimal model preserves the accepted operation order for fixed-point attribution and uses two refined RK4 runs for integrator attribution. Only generated final-error constants cross into the C64 presentation adapter; they never affect dynamics.

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


## Accepted Phase 2 implementation

The production core now extends the vertical laboratory with equatorial polar truth: radius, Earth-relative downrange, radial velocity, and inertial specific angular momentum. Pure model layers supply point-mass gravity, co-rotating atmosphere, Mach-dependent drag, and step-aligned open-loop pitch. A bounded executor owns ignition, burn, cutoff, separation, and coast transitions; raw execution, checksummed observation, and canonical KST2 recording all use that one executor.

The host runs complete nominal and failed-insertion missions, independent float64/RK4 comparisons, capture, and strict stream inspection. The C64 timing adapter measures representative exact raw and recorded paths without display work. Because a complete accuracy-first run takes hours on a PAL machine, the presentation adapter consumes a compact KRP2 index generated only from host-validated KST2, retaining constant memory while drawing PETSCII and scheduling SID cues. This keeps physics, validation, and presentation separate and preserves the truth/sensor boundary that Phase 3 will require.

## Accepted Phase 3 implementation

Phase 3 makes the truth boundary structural. `ksa64-interface` owns stable fixed-width transports; `ksa64-flight` depends only on that crate and therefore cannot import private simulator truth; `ksa64-sim` is the composition root that advances the world, generates sensors, runs flight software, applies the next command, and records telemetry. The physical KSA-2A model remains unchanged from Phase 2.

Each 0.125-second step applies the prior actuator command, advances world truth, derives imperfect measurements, advances navigation and flight software, validates the next command, and emits one coherent observation. This creates one-step command latency without exposing successor truth to the controller. Engine and staging requests belong to flight software, while the world retains physical authority to reject impossible operations.

KST3 is the canonical closed-loop regression stream. It records truth, sensor projection, navigation state, guidance/actuator state, events, alarms, and independent rolling checksum chains. KRP3 is produced only after strict whole-stream inspection and is a constant-memory presentation index, never another dynamics implementation. The C64 target uses finite representative probes for exactness and timing; a full target mission is conditional on a pre-run time and memory decision.

## Accepted Phase 4 implementation

Phase 4 adds an allocation-free campaign layer without changing the Phase 3 mission path. Every run variation is keyed by master seed, run index, parameter identity, correlation group, and draw index, so results are independent of execution order and worker count. Run zero bypasses variation and reproduces the frozen Phase 3 nominal checksums exactly.

`CampaignAggregate` folds fixed 128-byte KSR4 summaries strictly in run-index order. Native workers may execute simulations in parallel, but ordered merging preserves one canonical result. An independent Python implementation reconstructs distributions from KSC4 and computes authoritative float64 orbit, load, and navigation evidence from raw KSR4 cutoff states.

Storage is observational. Stock mode keeps streaming aggregates, five deterministic interesting-run summaries, and one sparse KPH4 trajectory. The REU transport uses preserving capacity probes and explicit DMA; a deterministic `StoragePlan` turns detected or user-capped capacity into additional summaries, full KST4 histories, and compact KPH4 histories. Detection, DMA, archive, UI, or disk failures cannot change physics, later seeds, or aggregate results.

KRA4 is an append-only committed archive with independently protected records. KXV4 divides a selected logical archive into identity-bound numbered volumes. The same bounded four-page UI works on stock and expanded systems; an REU changes retention capacity, not controls or simulation behavior. IEC export occurs after simulation in a separate target utility when full archive support would otherwise consume simulation RAM.

## Accepted Phase 5 foundation and missions

Phase 5 extends the planar stack additively with ECI translation, scalar-first
quaternions, diagonal rigid-body dynamics, transverse bending/slosh modes, four
32 Hz fast substeps, two-axis gimbal actuation, bounded upper-stage RCS, and
strict spatial avionics. Flight code still receives transported measurements
only.

The reviewed guidance is a compact host-generated quaternion reference that
follows local horizontal through the launch plane. The mission controller uses
body-frame quaternion error and stage-specific static gains; no heap,
floating-point runtime, or truth access is introduced. Six deterministic
integrated missions now form the input to KST5 telemetry and later Phase 5
campaign work.
Phase 5 Gate 9 adds a KST5 observer to the single integrated spatial mission executor. The no-std simulation layer owns fixed 96-byte headers and 424-byte frames, including embedded sensor and command transports; the host layer owns allocation, file capture, stream sequencing, and presentation. A rolling observation checksum covers every committed mission-cadence record, while per-record and nested CRCs localize transport damage. Target evidence remains a bounded codec probe rather than a full mission.
Phase 5 Gate 10 parameterizes the integrated vehicle and sensor boundary without
forking the mission executor. The allocation-free campaign core derives each
run independently from a master seed and run index, emits strict KSR5 summaries,
and folds them canonically. The host may distribute complete runs across worker
threads, but only ordered summaries enter the aggregate. An independent Python
analyzer reconstructs every variation from KSC5 and computes float64 orbital
evidence from raw terminal vectors. The target path is deliberately finite: it
checks configuration, keyed sampling, and summary codecs without executing a
multi-hour mission.
Phase 5 Gate 11 measures the vehicle, avionics, and telemetry boundaries in
separate stock-compatible executables because their combined timing image does
not fit the base machine. The regions are additive at the mission boundary and
are checked against one native exact-value oracle. The resulting projection
keeps long execution behind explicit user approval while retaining finite target
exactness as routine evidence.

Phase 5 Gate 12 introduces a second, explicitly noncanonical observation tier. KPH5 reduces a mission to bounded 16-byte spatial points for stock-C64 plotting, while KST5 remains authoritative. A stock retention reducer keeps aggregates plus five deterministic summaries. The same selection policy feeds capacity-scaled REU reruns. KRA5 is append-only and independently versioned; the shared REU DMA implementation serves KRA4 and KRA5, but neither archive API is visible to the physics, sensor, navigation, guidance, or sequencing layers.

Phase 5 Gate 13 consumes KPH5 through a portable strict replay reducer and a target-only presentation adapter. The reducer revalidates both CRC layers and run identity, then derives bounded extrema and event-cue evidence. The C64 adapter performs no dynamics and projects the already quantized Y–Z history with fixed shifts. VICE accepts the adapter only from the complete screen image and cue hash; KST5 remains the canonical flight record.

## Accepted Phase 5 completion boundary

Phase 5 closes with one accepted single-machine composition and several
strictly observational evidence paths. The same portable spatial world,
transport-isolated flight code, mission executor, KST5 observer, KSC5/KSR5
campaign layer, and KPH5/KRA5 storage contracts compile natively and through
the pinned rust-mos target. Optional storage and target presentation remain
outside the causal simulation path.

This composition becomes the Phase 6 regression oracle. A future physical
world/flight split must transport the existing bounded measurements and
commands, make added latency explicit, and preserve one model implementation.
Link framing, timeouts, replay, and electrical behavior are new adapters; they
are not permission to duplicate truth or guidance algorithms.


## Accepted Phase 6 endpoint architecture

Phase 6 makes the Phase 5 seam deployable without duplicating either side. The world endpoint alone owns vehicle truth, sensor synthesis, mission time, and canonical observation. The flight endpoint consumes fixed transported measurements and returns next-epoch commands. A passive Mission Control endpoint may observe telemetry and run an independent delayed/noisy ground estimate, but it has no command authority.

KLF6 is the general framed session protocol for exact-paced or realtime adapters. KLR6 is the compact reviewed 32 Hz stream used by KSA-6R. Native processes, TCP, C64 mailbox automation, ACIA cartridges, Ultimate UCI, and a future user-port adapter share these contracts; transports do not gain authority over physics or guidance. One C64 plus a host is the accessible baseline. Additional C64s are optional endpoint placements, not functional requirements.

The exact-paced split remains the Phase 5 regression oracle. KSA-6R is separately versioned because its 32/8/1 Hz multirate schedule and compact observations are designed around stock PAL compute and link budgets. The accepted target run proves exact full-flight execution at normal PAL CPU speed under external pacing. Live physical transport remains a separate acceptance gate.

## Accepted Phase 6 Mission Control presentation architecture

The host presentation retains the complete ordered `MissionControlUpdate` history and renders it without entering the world-to-flight or flight-to-world path. Live flight and KMR6 replay use the same presentation model. Freeze changes only the display cursor; the broker and recorder continue. Replay derives every plot and event panel from the prefix ending at the selected cursor.

A strictly parsed embedded copy of the frozen 99-point nominal KPH5 history supplies the planned ascent. The accepted nominal terminal state supplies its orbit target. Onboard and independent ground states supply the two observed tracks. Presentation-only osculating elements, atmosphere/load estimates, and Earth-fixed coordinates use accepted model constants but remain labelled `MODEL EST`; they are not transported measurements or canonical evidence.

F1 through F6 have no SIM Director dependency. They consume PLAN, ONBOARD, GROUND EST, and labelled MODEL EST sources. F7 alone may display omniscient world truth and is rendered with a distinct warning palette. This boundary is enforced by mutating all director fields and requiring the operational page buffers to remain unchanged.

The renderer selects compact, standard, wide, and ultra-wide panel arrangements from terminal dimensions. Braille and ASCII plotters share the same bounded world-to-canvas transforms. Plot selection and initial trajectory view are launcher settings only; KLR6, KMR6, KST5, endpoint RAM, and mission scheduling remain unchanged.
## Accepted Phase 7 profile and pack architecture

Phase 7 adds a profile-neutral `EvaluationSummary` around the frozen KSA-2A and
KSA-5A executors and the separately scaled `HobbyVerticalV1` evaluator. Profile
selection happens at composition or link time; there is no universal hot-loop
vehicle model and no dynamic allocation in the portable mission path.

Human-readable vehicle, motor, and mission sources are host inputs. The offline
compiler resolves decimal units, sampled thrust, identities, bounds, and CRCs
into KVP7/KMP7/KMC7 before execution. The portable evaluator owns all physical
state transitions. Host tools may compile candidates, distribute independent
runs, retain evidence, and analyze results, but they do not duplicate the
production dynamics.

The stock C64 links only the hobby profile and embeds bounded packs. KSR7 and
KPH7 support compact post-run inspection; KST7 and KRA7 remain host-retained
evidence. Optional storage remains observational and cannot alter mission or
campaign checksums. Phase 8 may extend hobby flight spatially, but the useful
vertical evaluator remains a separately identified smaller model.

## Accepted Phase 8 spatial hobby architecture

`HobbySpatialV1` is an additive profile, not a replacement for `HobbyVerticalV1`. The host compiler turns provenance-bearing component sources into fixed-capacity vehicle, motor, mission, and wind packs. The portable evaluator alone advances physical state; host analysis, OpenRocket comparison, recording, storage, and presentation remain observational.

The spatial world uses local east/north/up coordinates and scalar-first Hamilton quaternions. A rail-constrained one-dimensional segment hands continuous state to bounded six-degree-of-freedom powered and coast flight. At first recovery deployment, attitude is retired and the world continues as three-dimensional point-mass descent with staged canopy inflation, layered wind, drift, and flat-ground contact.

Geometry-derived mass properties interpolate with propellant fraction. Barrowman-compatible normal-force and CP models, geometry-based damping, and the reviewed Mach/Cd table are valid only inside the declared Firestorm envelope. Atmosphere and aerodynamics fail closed rather than extrapolating. The reference environment prefix through 3 km is embedded exactly for stock-target size; the source compiler and identity bind its meaning.

The stock C64 embeds generated pack constants and links separate mission, exact-trace, and replay images below `$C000`. Rust-mos static-stack storage may extend above the PRG end, so the finite-probe result mailbox is fixed at `$C800`, outside linked program and stack memory. REU-backed history changes retention only and can never enter the simulation transition path.

## Accepted Phase 8.5 avionics and placement architecture

Vehicle source, physical profile, reference frame, avionics profile, actuator capability, and execution placement are orthogonal identities. `VerticalPointMassV1` and `LocalEnu6DofV1` are canonical source aliases for frozen identities 3 and 4; legacy `Hobby*` names and all existing bytes remain valid.

The local avionics-aware executor advances to the earlier of the retained physical deadline, exact 32 Hz release, or exact mission event. Sensor N is sampled after the preceding interval, flight software produces command N, and the world holds it through the next interval. The flight kernel runs IMU/control at 32 Hz, aiding/recovery/health at 8 Hz, and GPS/mission guidance at 1 Hz. It receives no private truth.

Guidance emits an effector-neutral control demand. The original Firestorm binds monitor-only allocation; the separately identified fictional derivative binds a two-axis gimbal with declared mass, pivot, travel, slew, lag, safe state, rail inhibition, and burnout loss of authority. Canard and RCS capabilities remain fail-closed until Phase 9.5.

Host/host and host-world plus VICE/C64-flight placements share KLF6/KLR8 ordering and the F1-F7 Mission Control sink. The generic stock flight endpoint supports monitor and gimbal configuration in one 15,412-byte image. Presentation, pacing, recording, storage, and endpoint location never enter evaluation identity or simulation transitions.

The accepted standalone Phase 8 stock world remains available. The attempted self-contained combined target requires 71,500 resident bytes, exceeding even all physical C64 RAM. ROM banking cannot add capacity, so combined stock packaging stopped at its explicit decision boundary. Disk overlays, a stock-specialized rewrite, or expansion memory require a separate decision; none was chosen implicitly.

## Accepted Phase 9 optimization architecture

Phase 9 keeps design search outside the portable evaluator. A KOM9 manifest defines bounded variables, objectives, constraints, uncertainty aggregation, engine identity, seed, and budgets. The host materializes each KDV9 vector into identity-bound Phase 8/8.5 packs, then calls the unchanged avionics-aware evaluator. Duplicate materialized candidates share evidence; neither the optimizer nor presentation receives private truth.

Every proposal draw is keyed by manifest, engine, generation, individual, variable, and draw index. Evaluations may run on arbitrary workers, but results merge by candidate and uncertainty index. Generation proposals and selection occur only after the preceding ordered boundary. Live progress observes completed boundaries and cannot influence proposal generation.

The robustness ladder is nominal for every candidate, the same ordered eight cases for search selection, and the frozen 64-case set for deterministically selected terminal finalists. Hard constraints remain hard: feasible candidates dominate infeasible ones; exact violation ordering applies only among infeasible candidates. NSGA-II is the canonical multi-objective engine, DE is a scalar/lexicographic challenger, and grid search supplies exact surfaces.

KRA9 commits complete generation segments and embeds KRE9 copies of retained KAS8 case evidence. Resume accepts only an exact completed prefix of the desired deterministic archive, then atomically replaces it with the validated complete form. Reports, TUI state, archive paths, worker count, and C64 placement are observational.

The C64 receives a bounded KFP9 finalist package and does not run production searches. Exact finalist flight may be rerun through the accepted host-world/C64-flight endpoint. The standalone finalist browser is a separate stock image; Phase 9 does not reopen the rejected monolithic Phase 8.5 packaging decision.

## Planned Phase 9.5 and Phase 10 validation authority

Advanced effectors and global flight preserve the existing ownership rule: during any interval, exactly one KSA64 world model owns and advances an entity's state. An external program may produce an input fixture or an independent comparison, but it never supplies live corrections, shares integration authority, or becomes a fallback dynamics path.

Phase 9.5 keeps canard, RCS, depletion, changing mass properties, actuator, control-allocation, and authority-handoff models inside the portable evaluator. Its interim stock baseline places the authoritative world on the host and the genuine flight/allocation endpoint on a stock C64 using exact externally paced KLF6/KLR9 step-and-ack cells. Wall-clock pacing is observational and may pause; this placement is not a realtime claim. Analytic fixtures and a deliberately small float64 implementation are the primary independent evidence. Optional Basilisk comparisons are limited to selected fixed-step attitude/RCS cases and are committed as offline fixtures; they do not validate KSA64-specific aerodynamics, exact release scheduling, or allocator logic.

Selected optimized candidates cross the same seam through a strict KFB9 Start bootstrap. The host materializes and validates the KPE9/KPA9-bound candidate, the C64 receives only its bounded flight and allocation configuration, and both sides instantiate the same portable kernels. The original reference endpoint remains frozen. The host shadow compares every returned command/status cell before the world advances. F1–F7 Mission Control, KMR9 recording, KFE9 browsing, and stock/REU retention are passive consumers and cannot alter candidate materialization, command order, or physics.

Phase 10 uses a layered evidence architecture:

    deterministic GlobalEcef6DofV1
        authoritative KSA64 state transition

    independent complete float64 model
        primary numerical and physical comparison

    SatKit, then Orekit where needed
        specialist offline time/frame/EOP/gravity fixtures

    GMAT
        occasional exoatmospheric trajectory corroboration

The global profile consumes versioned, compiled Earth/time data. Normal execution and CI never retrieve live leap-second or Earth-orientation data and never require an external validator. The profile contract freezes the ellipsoid, rotation/orientation model, supported time scales, leap-second and EOP sources and validity, transform conventions, and failure policy before integrated trajectory acceptance.

Transform validation is intentionally separated from force-model and integration validation. Position, velocity, attitude, angular rate, and time continuity are tested at the ENU/ECEF/ECI seams before a trajectory comparison can be interpreted. This prevents a convention error from being misdiagnosed as an aerodynamic, gravity, or integrator defect.
