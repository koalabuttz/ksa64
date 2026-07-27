# Architecture

## Status

This document describes the implemented architecture through Phase 11.5 and the extension boundaries for later phases. Accepted choices are recorded in `docs/decisions.md`; later phases may replace a model only through an explicit versioned decision.

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

## Accepted Phase 9.5 and Phase 10 global architecture

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

The accepted global executor moves one continuous state through
`LocalLaunch -> EcefAscent -> EciCoast -> EcefEntry -> LocalRecovery`.
Qualification occurs in the world, but ownership commits only at the next
exact 32 Hz release before sensor sampling. The onboard navigator transforms
its own estimate using the public frame service and is never reset from truth.

Host-world/host-flight is the canonical complete-mission placement.
Host-world plus externally paced stock-C64 flight uses the same KLR10 cells
and shadow verification. The portable C64 global world is deferred; no second
production simulator or host correction path was introduced.

## Accepted Phase 11 operations and stock-target architecture

Phase 11 wraps the frozen global flight computer in versioned flight-software
packages without changing KLR10 or creating another world model. Mission plans,
ground observations, predictions, procedures, uplinks, acknowledgements,
journals, actions, and debriefs are operational records. They may influence a
mission only through the public load-validate-commit boundary. The world
remains authoritative, and onboard and ground predictors propagate their own
transported estimates rather than private truth.

The logical avionics loop and ground-operations link remain separate even when
they share one physical transport. Losing ground communication cannot stop
onboard sensing, navigation, guidance, control, recovery, prediction, or
journaling. Roles and procedures filter information and broker public actions;
they never change physics implicitly.

`SafeholdRecoveryV1` remains a compact flat stock-C64 endpoint. The full
`KsaG10rReferenceOpsV1` endpoint uses an accepted headless banked stopgap:
low helper code below the normal PRG, main code below `$C000`, package/static
state under BASIC and the I/O window, and high helper code beneath KERNAL. It
disables interrupts and maps BASIC, I/O, and KERNAL out before portable package
execution, performs no ROM or device I/O afterward, and requires no REU.

The bank layout changes packaging only. Native and C64 endpoints execute the
same portable package and exchange the same strict operations; the finite VICE
gate requires byte-exact replies and preserved code/guard regions. Because an
aided release takes seconds on the stock PAL CPU, the accepted placement is
externally paced host-world/C64-flight and makes no realtime claim. A physical
bank loader and link, a 6502-specific rewrite, C64 Ultimate acceleration, and
the portable C64 world remain explicit follow-on tracks.

## Accepted Phase 11.5 product architecture

Phase 11.5 adds a host-only product boundary above the accepted domain implementations:

```text
ProductCatalog
      |
      v
Ksa64Application facade
      |
      +--> accepted mission and operations services
      +--> campaign and optimization workbenches
      +--> strict evidence services
      +--> explicit target and historical-audit dispatch
      |
      +--> ksa64 CLI
      +--> Phase 12 Mission Foundry
```

The facade owns discovery, capability validation, orchestration, structured outcomes, and diagnostics. It does not own a vehicle state, duplicate a simulator, reinterpret telemetry, alter optimizer selection, or replace a strict evidence parser. Each catalog entry points directly to an accepted service adapter.

The catalog has a current tier for supported product experiences and a separate historical tier for engineering audits and compatibility tools. New user-facing state uses stable domain IDs and canonical source aliases; serialized profile variants, phase modules, K-format identities, artifacts, and hashes remain unchanged.

The CLI and Phase 12 consume the same Rust services. The GUI must never treat console text as an API. Catalog JSON is deterministic host metadata, not canonical simulation evidence, and Phase 11.5 therefore adds no K-format family.

Target automation separates stored verification, build, and live probe requests. Stored verification cannot create a process. Live VICE requires an explicit flag and delegates to the phase that owns the target evidence, preserving one-instance, warp-disabled, cooldown, cleanup, and long-run rules.


### Pre-Phase 12 application hardening

The public `Ksa64Application` remains one orchestration facade, but accepted project, mission, campaign, evidence, optimization, and automation adapters are implemented in focused host modules. Static `ApplicationService` dispatch remains appropriate for reviewed built-ins and is not extended once per user project.

The nested `ApplicationRequest` family covers Project, Mission, Campaign, Optimization, Evidence, Target, and Audit work. Each request declares a conservative permission class, a safe cancellation boundary, and whether explicit live confirmation is required. This metadata helps Phase 12 queue work without weakening the existing live-target gates; it is not authority to bypass those gates.

### Incremental mission-session authority

The flagship Phase 11 GNSS-loss experience now has one host-owned incremental orchestration boundary:

```text
Ksa64Application::start_mission
        |
        v
LiveMissionSession
  lifecycle + pacing + typed snapshots/events
        |
        +--> accepted flight-package release processing
        +--> stage / validate / commit operator actions
        +--> deterministic evidence accumulation
        +--> existing KSB11 finalizer
```

The session advances the accepted package; it does not reproduce flight dynamics or avionics. Presentation owns wall-clock scheduling but never state. Pause and pacing are noncanonical; release order and accepted actions are evidence. Scripted and interactive copies of one action transcript finalize identically. Experiences without an incremental adapter remain explicitly synchronous and cannot claim live-session capability.

Phase 12 must consume this object rather than driving flight packages directly or replaying a completed mission as if it were live.

Discovery remains three separate domains:

```text
AcceptedProductCatalog --> reviewed built-in experiences and maturity
ProjectWorkspace       --> authored source and Draft-to-Reviewed validation
RecentSessions         --> derived evidence with an explicit origin
```

The UI may present these together but cannot merge their identities. Reusing an accepted model profile does not make a user vehicle or mission accepted. Unknown binary evidence is opaque until an owning strict parser recognizes it.

## Phase 12A integration boundary

Phase 12A adds a host presentation seam, not another authority layer:

```text
Unreal packaged runtime / native C++ harness
                  |
       validated C function table
                  |
    versioned viewer-bridge DLL
      command + snapshot queues
                  |
     dedicated Rust session worker
                  |
          Ksa64Application
                  |
         LiveMissionSession
                  |
   accepted Phase 11 state and KSB11
```

The C ABI uses opaque handles, fixed-width structures carrying ABI and layout
sizes, explicit caller/Rust buffer ownership, typed errors, and immutable roles.
The DLL and adjacent manifest are commit-qualified and hash-validated before
use. UnrealBuildTool stages a prebuilt DLL; it does not run Cargo. Each handle
owns one Rust worker so the Unreal game thread only enqueues commands or polls
immutable snapshots/events. Queue pressure, render cadence, and passive pacing
cannot change release order, accepted actions, or final evidence.

Role filtering happens in Rust before data crosses the ABI. A guided operator
never receives SIM Director truth for UMG or C++ to hide. Canonical KSB11 bytes
are built by the existing finalizer and returned unchanged; catalog JSON,
snapshots, bridge diagnostics, camera state, and performance records remain
noncanonical host metadata.

The accepted Phase 12A implementation fixes each worker at a bounded 32-command
queue and 256-event queue. Full queues report deterministic `QueueFull` results
without advancing the session. The native harness and Unreal plugin load the
commit-qualified `e98df4921c03` bridge DLL (SHA-256
`d1605c4aa9a8b407d8e35ee76d965e404c1e7efcc357d8bd0704b73ade43272d`),
enforce its ABI/layout/hash manifest, and recover the unchanged 22,369-byte
KSB11 session. The packaged Development runtime stages that same artifact and
starts without Unreal Editor, MCP, or Python.

Phase 12A keeps development and product layers separate:

```text
Editor-only development       Packaged Mission Foundry
-----------------------       -------------------------
Unreal MCP (optional)         runtime C++ plugin
Unreal Python (optional)      versioned Rust bridge
asset/import tooling          role-filtered application data
Codex-assisted inspection     no editor/MCP/Python dependency
```

MCP stays loopback-only and serialized because the experimental UE 5.8 server
has no authentication and executes tool calls on the game thread. Its success
or failure cannot change build, automation, cook, packaging, or runtime
acceptance.

The first bridge proof uses the live guided GNSS-loss session solely for
lifecycle, action, role, and evidence fidelity. Phase 12B adds its operations
presentation. Phase 12C separately consumes the complete Phase 10 recording to
prove coordinate display domains, large-world continuity, component events,
entry, and recovery. No Phase 12A component owns rendering or coordinate
conversion.

## Phase 12B live operations boundary

Phase 12B generalizes the accepted global runner without creating a second mission authority:

```text
Phase 10 GlobalWorldMachine
        | KLR10 sensor and transition cells
        v
KsaG10rReferenceOpsV1
        | accepted flight commands
        v
Phase 10 world
        | public delayed/noisy tracking observations
        v
ground estimator -> procedure -> uplink broker -> existing KSB11 finalizer
        |
        v
role-filtered presentation views -> UKsa64LiveMissionSubsystem -> Slate desk
```

The full viewer session runs the accepted KSA-G10R mission and injects persistent GNSS loss during GCRF coast. The untouched/no-action path remains 22,015 releases; the accepted reference command transcript changes the later guided path and lands at release 21,591. The nine-release session remains a compatibility fixture, not a human-facing realtime exercise.

`UKsa64LiveMissionSubsystem` is the only Unreal object that talks to the bridge. It schedules exact releases with integer wall-clock accumulation, serializes commands, polls snapshots and cursored histories, publishes immutable view models, and saves evidence through Rust. Widgets cannot poll the bridge, parse canonical records, or construct uplink bytes. Unreal opens the Rust live session in `Fast` execution-capacity mode so each explicit bounded `Advance(n)` request is honored; Unreal alone maps realtime, pause, single-step, 4x, 16x, and maximum-fast presentation modes into those requests. This internal capacity setting is noncanonical, emits no pace evidence, and cannot affect KSB11 when the exact release and action transcripts are identical.

The ABI change is additive. Every ABI-v1 Phase 12A function and structure remains unchanged; feature bits expose fixed-layout operational, procedure, action, path, timeline, transport, and disposition views. Existing KLR10/KUL11/KUA11/KSB11 owners remain authoritative. A ground-navigation update is accepted only when its estimator identity, latest checksum, frame, and exact state fields match the current independent ground estimate; callers cannot author or alter its position or velocity.

Disposition deliberately separates mission objective, vehicle, procedure, operator, avionics, and evidence outcomes. A procedure deviation is operational evidence, not an automatic rewrite of the physical result. Realtime, pause, step, fast pacing, polling, sound, interpolation, runtime role, and hints do not enter the full-session identity. With an identical ordered action transcript, role and hints cannot change canonical evidence; role permissions may only govern which explicit recorded actions can be submitted. Exact releases, guards, receipts, events, and actions are never interpolated, prediction sources remain labelled, and Guided Operator never receives SIM Director truth.

The Phase 12B command desk is a 2-D operational view. Phase 12C alone owns Earth-scale 3-D domains, ENU/ECEF/GCRF display conversion, vehicle pose, cameras, entry, and recovery visualization.

### Accepted Phase 12B product topology

The accepted product qualifies the additive ABI-v1 bridge at build identity `0x120B0001` and source commit `423c116cf58632f344d4a48774a97a4487c34113`. The commit-qualified DLL SHA-256 is `da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565`. Both native harnesses, 17/17 Unreal operations tests, standalone packaging, exact full-session evidence, and the D3D12 command desk pass. Rust continues to own release order, role filtering, action validation, prediction identity, outcome classification, and KSB11 finalization.

Worker termination and evidence finalization are separate states. A clean request to stop a partial presentation session terminates the worker but leaves finalization `InProgress` with no archive; it is not reported as mission or evidence failure. Only an actual worker/finalizer error produces `Failed`, and only a Rust-sealed archive produces `Completed`.

Phase 12C consumes typed streams and recordings through this same boundary. It owns passive ENU/ECEF/GCRF display conversion, large-world origin management, Earth and vehicle pose, cameras, entry/recovery views, and exact visual snapping. It cannot use Chaos or scene state as physical authority, derive events from rendering, parse canonical K records, expose truth to operational roles, or modify Phase 12B dispositions. The accepted 2-D operations desk remains a regression surface and companion overlay.

## Phase 12B.5 cross-platform presentation boundary

Phase 12B.5 separates the accepted application/session authority from any one
loader, process model, renderer, or operating system:

The portable boundary is implemented in two deliberately narrow crates: `ksa64-session` owns exact mission advancement and in-memory KSB11 finalization, while `ksa64-presentation` owns `no_std + alloc` role-filtered DTOs and the noncanonical KPS1 codec. Filesystem/report/process services remain in `ksa64-host`.

```text
                 Ksa64Application / LiveMissionSession
                               |
              Rust-owned role filtering and action broker
                               |
        typed presentation-session and replay view contract
             /                 |                 \
    in-process ABI       sidecar/network       local WASM
       /   |   \          /    |    \             |
 Windows Linux macOS   web  Vita mobile       browser worker
       |                  presentation clients
       +---- Unreal ---- WebGPU/WebGL ---- SDL2 -----------+
```

The shared layer contains view semantics, release and event identities, role
permissions, action proposals and receipts, lifecycle, staleness, integrity,
and deterministic replay fixtures. It does not contain renderer objects,
windowing, input widgets, authoritative physics, or a client-side canonical
evidence parser.

Desktop in-process libraries use platform-native `.dll`, `.so`, or `.dylib`
artifacts. Vita uses a statically linked Rust/VitaSDK plus SDL2 executable.
Browser clients begin remote or replay-first; local WebAssembly authority runs
in a dedicated worker and is accepted only when it reproduces native KSB11.
Android and iOS use native static/shared Rust packaging appropriate to their
lifecycle rather than pretending the desktop dynamic-loader contract applies
unchanged.

The 8 GB Lenovo Chromebook Duet 11 has a specific all-local placement:

```text
Debian Crostini: Rust world + flight + LiveMissionSession
                         |
ChromeOS browser: role-filtered PWA Mission Control + WebGPU/WebGL viewer
```

The browser may be closed, throttled, or reconnected without changing the
Crostini-owned mission. An explicit local mission pause still occurs only at
an accepted release boundary and enters the action/lifecycle evidence when the
contract requires it.

Vita, browser, and mobile clients may submit only the same high-level Phase 11
actions as desktop Mission Control. Network exposure is loopback-only by
default. LAN operation requires explicit pairing, authentication, role binding,
origin policy, bounded sequencing, stale-message rejection, and a deterministic
disconnect/reconnect contract.

### Browser presentation and local authority

The accepted web stack is React plus TypeScript for semantic Mission Control,
Vite for builds with an explicit audited manifest/service-worker packaging
layer, and Babylon.js for 3-D. Babylon is integrated
directly through an imperative scene adapter rather than a React-specific
renderer wrapper:

```text
Browser main thread
  React DOM/SVG/Canvas operations UI
  Babylon.js WebGPU -> WebGL2 viewer
  presentation interpolation only
                         |
             transferable typed views
                         |
Dedicated Web Worker
  Rust/WASM browser session
  authoritative world + flight package
  operations + role filtering + KSB11 finalizer
```

The same presentation adapter may instead connect to a remote native authority
or a Rust-owned replay reader. Babylon receives camera-relative positions,
orientations, paths, source labels, validity, and exact-event metadata. Rust
alone owns ENU/ECEF/GCRF transformations, time, physics, event detection,
mission outcome, actions, role filtering, and canonical evidence. Babylon
physics and collision integration are disabled.

WebGPU selection is explicit: the client accepts it only after capability and
asynchronous initialization succeed, otherwise it constructs a separately
tested WebGL2 engine. Complete 2-D operations remain available without a valid
3-D adapter. Rendering backend, frame cadence, interpolation, quality, and
client lifetime remain outside simulation identity.

The Web Worker advances by accepted releases, never render ticks. A service
worker may cache the PWA but cannot own a mission. Remote authority continues
through browser disconnect according to the transport contract. A local-WASM
session interrupted by page close, browser discard, suspension, worker failure,
or panic remains incomplete unless a later validated checkpoint/replay contract
reconstructs it exactly; it cannot claim uninterrupted execution.

Local browser authority uses the accepted simulator and in-memory evidence
encoder through a browser-safe session crate. It does not compile desktop
filesystem, terminal, process, native-thread, target, campaign, optimizer, or
C-ABI code into WebAssembly. Native/WASM acceptance requires the identical
catalog, definition, action transcript, release/event ordering, checksum chains,
outcome axes, terminal release, KSB11 bytes, and SHA-256.

Phase 12C supplies one renderer-neutral global display model to desktop Unreal
and Babylon.js WebGPU/WebGL2. Phase 12C.5 productizes portable web, Vita/SDL2,
Android, and iOS clients and accepts the polished all-browser world plus viewer.
Rendering quality and placement remain outside simulation and evidence identity.
