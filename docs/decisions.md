# Decision record

This is a lightweight architecture decision log. Accepted decisions guide implementation. Provisional decisions require an experiment or design result before becoming final. Superseded decisions remain here with links to their replacements.

## D-001: Build a new core

Status: Accepted

Decision:

Build the KSA64 simulation core for this project rather than adapting an existing lunar lander, satellite tracker, or commercial game.

Rationale:

Existing programs solve narrower problems and generally mix state, physics, controls, and presentation in ways that do not support the intended architecture. Adapting one would replace most of its internals while inheriting constraints and possible licensing obligations.

Existing programs remain useful as historical references, presentation inspiration, and validation aids.

## D-002: Maintain one portable simulation core

Status: Accepted

Decision:

Compile one deterministic core for both a modern host and the C64. Do not maintain separate desktop and C64 simulators.

Rationale:

One core minimizes model drift and makes native testing fast. It also permits direct cross-target comparisons.

Consequence:

Host and C64 agreement verifies implementation consistency only. Analytic tests and independent tools are still required for physical validation.

## D-003: Use fixed-point arithmetic in the product core

Status: Accepted

Decision:

Use explicitly scaled integer arithmetic, lookup tables, and widening intermediates where needed. Do not depend on software floating point in the hot simulation path.

Rationale:

The C64 has no floating-point hardware, and the simulation repeatedly evaluates arithmetic-heavy models. Fixed point gives explicit control over range, precision, cost, and deterministic behavior.

Consequence:

Formats will be selected per quantity after range analysis. Overflow, rounding, saturation, and unit conventions must be documented and tested.

## D-004: Use Rust and rust-mos for the portable core

Status: Accepted

Decision:

Use Rust for the shared host/C64 simulation core and rust-mos for the C64 target. Keep platform-specific hardware code behind explicit modules and preserve the native exact-arithmetic test path.

Rationale:

Both candidates passed the frozen arithmetic and vertical-flight contract. On the representative 2,048-step kernel under the same PAL VICE 3.10 CIA clock, Rust used 223,772,332 cycles versus Oscar64's 235,627,088, a 5.03 percent cycle reduction. Rust's timed PRG is larger and uses 17 bytes of zero page, but its 9,026-byte image and 66-byte linker-reserved static stack leave credible room for Phase 1.

Oscar64 was faster on isolated general division and fast interpolation division, but that advantage did not survive the representative full workload. The experiment rubric explicitly favors the incumbent when it is correct, feasible, and within 25 percent of the challenger; Rust is instead modestly faster overall and has the lower-risk development workflow for this project.

Consequence:

New production simulation code begins in Rust after the remaining Phase 0 numeric decisions are settled. A measured hotspot may still use an assembly or foreign-function helper without reopening the project-language decision.

## D-005: Retain Oscar64 C++ as a reference challenger

Status: Accepted

Decision:

Keep the Phase 0 Oscar64 implementation as an independent comparison, generated-code reference, and source of optimization ideas. Do not use it as the production-core language.

Rationale:

Oscar64 produced a 35 percent smaller timed PRG and excellent isolated arithmetic code: essentially equal scaled multiplication, 6.69 percent fewer cycles for general scaled division, and 33.26 percent fewer cycles for the specialized fraction divider. Those results are valuable when optimizing Rust kernels, but the complete flight-dynamics workload is the controlling measurement.

Constraints:

Oscar64 comparison code remains restrained embedded C++ with static allocation, plain value types, no exceptions or RTTI, and no dependency from the production core.

## D-006: Defer other language ports

Status: Accepted for Phase 0

Decision:

Do not initially implement equivalent benchmarks in plain C, Prog8, or Millfork.

Rationale:

Rust and Oscar64 directly test the two strongest shared-core options. Prog8 and Millfork weaken native-source portability, while plain C remains a low-risk fallback if both leading candidates fail.

Revisit when:

- Rust and Oscar64 results are inconclusive.
- A compiler defect blocks a leading candidate.
- A focused kernel needs a C or assembly baseline.

## D-007: Start below 6-DOF

Status: Accepted

Decision:

Develop vertical flight first, then a two-dimensional ascent and orbital model. Do not begin with three-dimensional rigid-body dynamics.

Rationale:

The simpler models exercise the important numeric, environment, propulsion, staging, integration, and validation problems while remaining explainable and computationally tractable.

6-DOF begins only after a concrete need and a successful performance study.

## D-008: Separate truth, sensors, flight software, and actuators

Status: Accepted

Decision:

Flight software must consume simulated measurements and produce commands. It must not read private truth state.

Rationale:

This boundary makes sensor error, navigation, closed-loop control, failure injection, and later hardware-in-the-loop operation meaningful.

## D-009: Use specialized independent references

Status: Accepted

Decision:

Use analytic solutions and different external tools for the regimes they model well rather than searching for one authoritative oracle.

Examples:

- Analytic motion and conservation cases for fundamentals.
- RocketPy for compatible atmospheric rocket cases.
- Tudat or GMAT for compatible orbital propagation.
- PREDICT or historical QUIKTRAK comparisons for later ground tracking.

All comparisons must align frames, constants, units, atmosphere, vehicle parameters, and integration assumptions.

## D-010: Use tables for expensive stable functions

Status: Accepted

Decision:

Prefer validated lookup tables and interpolation for trigonometry, atmosphere, speed of sound, aerodynamic coefficients, and engine curves when direct evaluation is too costly.

Rationale:

Host-generated tables trade inexpensive storage for scarce 6510 cycles and can be shared exactly by both targets.

## D-011: Keep hot state in C64 RAM

Status: Accepted

Decision:

Use ordinary RAM for current simulation state and frequently accessed data. Use the REU as explicitly transferred backing storage.

Rationale:

The REU provides capacity, not a flat 16 MB address space. Hiding transfers behind ordinary pointer semantics would obscure costs and complicate portability.

## D-012: Do not require real-time execution

Status: Accepted

Decision:

Simulation time may advance slower or faster than wall-clock time.

Rationale:

Numerical usefulness matters more than visual frame rate. Rendering and telemetry can run at lower rates than physics.

## D-013: Avoid copying restrictive reference code into the core

Status: Accepted

Decision:

Study existing code only within its license. Reimplement needed ideas independently unless a deliberate dependency and compatible project license are chosen.

Rationale:

The known C64 MoonLander project uses CC BY-NC-SA terms, and PREDICT uses GPL terms. KSA64 should not acquire licensing constraints accidentally through casual copying.

## D-014: Freeze a benchmark-only numeric contract before implementation

Status: Accepted for Phase 0

Decision:

Use the versioned contract in `phase0/CONTRACT.md` as the common specification for the Rust and Oscar64 implementations. Generate shared arithmetic and vertical-flight vectors with an independent, standard-library-only Python program before writing either target implementation.

Rationale:

This prevents either language implementation from silently defining the expected behavior. Exact units, scales, rounding, saturation, operation order, checkpoints, and checksums make cross-target disagreements diagnosable.

Consequence:

The Phase 0 formats and simplified vehicle model are benchmark fixtures, not final simulator architecture decisions. Results may justify changing the eventual product formats, overflow policy, integrator, or environment models.

## D-015: Use explicit two-word widening arithmetic

Status: Accepted

Decision:

Implement required 64-bit intermediate operations as explicit two-word algorithms in the fixed-point core. Do not use compiler-provided Rust `u64` arithmetic on the pinned rust-mos toolchain for these kernels.

Rationale:

The compiler-provided `u64` baseline produces two reproducible C64-target failures despite passing natively. The explicit four-part 32-by-32 multiplication, two-word shifting, and restoring 64-by-32 division pass every arithmetic vector, vertical checkpoint, and checksum on both targets while reducing the Rust correctness PRG from 14,576 to 6,580 bytes.

Consequence:

Keep the failing baseline as a regression probe. Reevaluate compiler-provided widening only after a pinned toolchain update passes the complete frozen contract.

## D-016: Adopt the Phase 1 numeric contract

Status: Accepted

Decision:

Use `ksa64.numeric.phase1-v1` from `phase0/numeric/FOUNDATION.md` for the vertical-flight laboratory. Stored quantities use per-field signed `i32` Q formats, interpolation fractions use unsigned Q0.16, and widened products use the accepted explicit two-word implementation.

Rationale:

The generated range analysis covers the declared independent and coupled Phase 1 envelope. Its largest product requires 56 bits including sign, every scaled result fits `i32`, and the selected formats preserve useful physical resolution. Product speed-squared intermediates use Q12.20 rather than inheriting the benchmark Q8.24 fixture.

Consequence:

A scenario outside the declared envelope is rejected or requires a versioned numeric contract. Phase 2 repeats range analysis before inheriting these choices.

## D-017: Treat saturation as an aborting numeric fault

Status: Accepted

Decision:

Validate scenarios before execution and range-prove branch-free hot-path operations. A public arithmetic primitive that escapes its proven result range saturates deterministically, sets a sticky numeric-fault flag, and causes the run to stop at the next step boundary. Division by zero is a fault.

Rationale:

Silent wrapping is unacceptable, while continuing a saturated trajectory can look plausible and hide an invalid model. Sticky containment preserves diagnostics and deterministic cross-target behavior without making saturation a normal physical result.

## D-018: Begin with semi-implicit Euler at 0.125 seconds

Status: Accepted for Phase 1

Decision:

Use one semi-implicit Euler evaluation per fixed 0.125-second physics step. Emit telemetry every eight steps by default. Measure RK2 against the completed Phase 1 model before adopting it.

Rationale:

The selected Rust kernel fits the raw PAL 8 Hz cycle budget. Generated analytic cases demonstrate the expected first-order error trend, signed behavior, exact constant-velocity motion for representable inputs, and exact mass-flow boundary. RK2 would add another force evaluation before measured end-to-end evidence justifies its cost.

## D-019: Version scenario images and exact telemetry records

Status: Accepted

Decision:

Use exact decimal strings in human-authored scenario JSON, then validate and pack a versioned, little-endian C64 scenario image. Use fixed binary telemetry frames containing raw fixed-point values, a rolling exact-state checksum, and record CRCs. CSV is an optional host view, not the canonical regression artifact.

Rationale:

This prevents locale and binary-floating-point parsing from defining C64 inputs, gives host and target byte-for-byte fixtures, and separates human editing from compact runtime storage. The layouts are defined in `docs/data-formats.md`.

## D-020: Use a simple tabulated Earth environment in Phase 1

Status: Accepted for Phase 1

Decision:

Use the existing 19-knot altitude/density table and a gravity table generated from `g(h) = g0 * (R / (R + h))^2`, with `g0 = 0.00980665 km/s^2` and `R = 6371 km`. Identify the table set as `earth.simple-atmosphere.v1`.

Rationale:

This model is deterministic, cheap to interpolate, already covered by exact host/C64 benchmark evidence, and adequate for learning vertical-flight architecture. It is not presented as a standard atmosphere or a real-vehicle prediction.

Consequence:

Any replacement atmosphere or Earth model gets a new identifier and comparison contract.

## D-021: Keep the production numeric core no_std and fixture-free by default

Status: Accepted

Decision:

Implement Phase 1 numeric behavior in the production `ksa64-core` crate with `#![no_std]`. Represent physical quantities as distinct `repr(transparent)` integer wrappers, pass an explicit sticky `NumericStatus` through fallible arithmetic, and compile golden fixtures and self-test loops only when the `fixtures`, `sim`, or `c64` features request them.

Rationale:

One shared source now runs natively and through rust-mos without allocating, invoking software floating point, or depending on platform services. Strong wrappers reject unit-category mistakes at compile time without increasing storage. Explicit status keeps exceptional behavior deterministic, while feature-gated fixtures prevent the 11,794-byte diagnostic PRG from being mistaken for production runtime cost.

Consequence:

Vehicle and environment code will consume these wrappers and status rules rather than raw unlabelled integers. Host-only high-precision comparisons remain outside the exact product core.

## D-022: Validate packed scenarios before constructing truth state

Status: Accepted

Decision:

Parse only the exact 76-byte `KSC1` v1 record in the product core. Check framing, CRC-32, numeric-contract ID, environment ID, field envelopes, inert/dry-mass relationships, total duration, step-aligned burn duration, and conservative thrust acceleration before returning strong configuration types. Preserve reserved flag bits without assigning v1 meaning.

Rationale:

The C64 should never begin a run from corrupted, incompatible, or numerically impossible configuration. Performing these checks once at ingestion keeps later hot paths small. Preserving reserved flags follows the format rule that readers ignore unknown bits and permits compatible metadata additions without weakening version and identity checks.

Consequence:

Vehicle truth state may be created only from a validated `Scenario`. Human-readable JSON remains a host concern; the exact core parses no text and allocates no memory.

## D-023: Generate the environment and keep initial truth immutable

Status: Accepted

Decision:

Generate the production Rust binding for `earth.simple-atmosphere.v1` from the frozen Phase 0 source data and verify the generated digest in the Phase 1 gate. Sample density and gravity through typed, clamped interpolation. Keep `Scenario` fields private and expose read-only accessors so callers cannot fabricate a value that bypasses ingestion checks. Construct the 28-byte initial vertical truth state only from such a validated scenario, with private fields and no public dynamics-transition methods.

Rationale:

One canonical generator prevents the benchmark fixture and production environment from drifting while keeping raw source data out of the runtime crate. A private validated scenario makes the construction boundary meaningful rather than conventional. Separating immutable initialization from force evaluation and integration lets each physics boundary acquire exact tests before any method can advance simulated time.

Consequence:

The environment and initial state are production code, but they do not yet constitute a simulator. The next slice will return a typed force or acceleration snapshot without mutating truth; only a later integration gate may create successor truth states.

## D-024: Evaluate vertical forces without mutating truth

Status: Accepted

Decision:

Use positive-up signed vertical dynamics. Thrust is nonnegative upward, weight is reported as a nonnegative downward magnitude, and drag is a signed force opposing velocity. Compute `net = thrust - weight + drag`, then divide by mass for acceleration. The engine is active only while propellant remains and mission time is strictly before burn duration. Return all results in a private-field `VerticalForceSnapshot`; do not mutate truth.

The evaluator records `InvalidInput` in the sticky numeric status when net force or acceleration escapes the accepted Phase 1 coupled envelope. Environment samples also have private fields, so production callers can obtain them only through the accepted sampler. A generated and digest-pinned exact pack covers powered rest, upward and downward drag, burn-time cutoff, propellant cutoff in vacuum, and a deliberate acceleration-envelope escape.

Rationale:

A signed drag component removes ambiguity at negative velocity while preserving a direct sum of forces along the modeled axis. Keeping evaluation pure makes force arithmetic independently testable and prevents an accidental partial update when a numeric fault occurs. Explicit model-domain faults catch values that fit `i32` but invalidate the range proof.

Consequence:

Force evaluation is production-ready but simulated time still cannot advance. The next gate may introduce one checked semi-implicit-Euler transition that returns successor truth and performs bounded mass consumption; run loops and telemetry remain later work.

## D-025: Advance truth only through a fail-closed step

Status: Accepted

Decision:

Implement one pure `advance_vertical_state` operation. Reject a pre-existing numeric fault or a completed scenario before evaluation. For a valid step, sample the environment and evaluate forces from the old truth, update velocity before altitude using semi-implicit Euler, advance time, and consume `min(mass_flow * timestep, remaining_propellant)` only while the engine is active. Construct successor truth only after every arithmetic and model-domain check remains clear.

A successful result carries the immutable successor, the force snapshot used for the step, exact propellant consumed, and a boundary cutoff event. A numeric failure or scenario completion returns an error and no successor. Scenario ingestion additionally requires inert mass to cover dry mass and burn duration to align exactly with the fixed timestep.

Rationale:

Semi-implicit Euler matches the accepted Phase 1 numeric foundation and uses one force evaluation. Delaying successor construction prevents partially advanced states from escaping after a fault. Step-aligned constant thrust avoids silently applying a full-step burn past an arbitrary cutoff, while bounded consumption reaches zero propellant exactly without underflow.

Consequence:

The core can advance one trustworthy physics step but does not yet execute a mission. The next gate will repeatedly apply this operation to the validated step limit, stop on the first error, and produce a compact deterministic summary and exact-state checksum; telemetry remains separate.

## Open decisions

The following remain deliberately unresolved:

- License for KSA64.
- Target C64 and REU configurations beyond the baseline unexpanded C64.
- Minimum acceptable simulation rate once display and telemetry are included.
- Whether the high-precision host comparison uses numeric generics or a deliberately independent compact implementation.

