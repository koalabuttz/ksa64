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

## D-026: Execute missions with a canonical rolling checksum

Status: Accepted

Decision:

Run a validated Phase 1 scenario from its initial truth until the declared step count by repeatedly invoking the checked transition. After each successful successor, update a 32-bit FNV-1a checksum over explicit little-endian raw fields in this order: step, time, altitude, velocity, acceleration, total mass, and propellant. Do not hash the initial unadvanced state, memory layout, status, or event flags.

On success, return final truth, checksum, and cutoff-event count. On failure, return the last valid truth, checksum through that truth, cutoff count, sticky numeric status, and step-error cause. The executor performs no allocation, I/O, telemetry serialization, or presentation.

Rationale:

The loop centralizes completion and failure semantics without weakening the independently tested step boundary. Explicit checksum bytes remain identical across host and MOS targets and allow later telemetry tools to locate deterministic divergence. Returning the last valid snapshot makes a fault diagnosable without exposing a partially computed successor.

Consequence:

The exact core now performs an end-to-end vertical mission. Its common-clock execution cost must be measured before telemetry and UI are added; the independently generated golden mission fixes the current final state and checksum as regression evidence.

## D-027: Separate production dynamics timing from rolling validation

Status: Accepted

Decision:

Generate checked dynamics-only and rolling-checksum execution from one const-generic mission loop, then measure both regions in one dedicated C64 timing PRG using the established PAL CIA common clock. Treat 8 Hz as a provisional optimization target rather than a correctness requirement. Do not add telemetry until the two general environment interpolation divisions have been replaced with the exact specialized path already proven in Phase 0 and the common-clock measurement has been repeated.

Rationale:

Three identical runs measure checked dynamics at 160,904.64 cycles per step and dynamics with per-successor FNV-1a at 210,410.64, against a raw 123,156-cycle budget. The 49,506-cycle checksum delta is validation policy rather than physics. Phase 0 directly measured enough savings in the exact interpolation specialization to make it the highest-value next change without weakening the numeric contract.

Consequence:

The timing evidence is trustworthy, but the current executor is not yet an 8 Hz real-time core. Validation checksums remain available and deterministic, while interactive scheduling may choose a different validation cadence later. Telemetry and display work remain blocked behind the focused interpolation optimization and fresh measurement.

## D-028: Specialize integral-Q12 interpolation without weakening fallback behavior

Status: Accepted

Decision:

Add a checked interpolation primitive for tables whose positive knot spans are exact integral Q20.12 kilometre counts. Compute the Q0.16 fraction with the Phase 0 32-by-16 restoring divider, preserve nearest rounding with exact halves away from zero, and reject an invalid specialized span through `NumericStatus`. Keep the general interpolation and 64-by-32 division paths for other valid models.

Rationale:

The simple Earth table satisfies the narrower contract, and direct tests show the specialized and general primitives agree at clamps, knots, interiors, and rounding boundaries. The golden mission remains bit-exact across native Rust and MOS targets. Three PAL common-clock runs reduce checked dynamics from 160,904.64 to 127,932.69 cycles per step, a 20.49 percent improvement.

Consequence:

Checked dynamics rises from 6.12 Hz to 7.70 Hz but remains 4,776.69 cycles per step, or 3.88 percent, over the provisional 8 Hz budget. The next optimization target is the one remaining general acceleration division per step. Telemetry remains deferred until that measurement closes or deliberately revises the target.

## D-029: Use an exact reduced acceleration divider with general fallback

Status: Accepted

Decision:

For Q12 force divided by Q12 mass into Q28 acceleration, attempt a reduced 64-by-16 path only when force magnitude fits the accepted 21-bit envelope, mass raw units are divisible by 128, and the reduced denominator fits `u16`. Remove that exact power-of-two factor from both denominator and numerator shift, process only the proven occupied bits, and preserve the global rounding and saturation contract. Route every other input through the original general divider.

Rationale:

The golden vehicle's mass sequence satisfies the fast-path contract, while the fallback preserves all other validated scenarios. Direct tests compare specialized and general results for signed values, zero divisors, non-aligned masses, large masses, and out-of-envelope forces. Native and MOS mission results remain bit-exact. Three PAL common-clock runs reduce checked dynamics from 127,932.69 to 114,981.59 cycles per step.

Consequence:

Checked dynamics now reaches 8.57 Hz with 8,174.41 cycles per step, or 6.64 percent, of raw 8 Hz headroom. The arithmetic performance gate is closed. Per-successor rolling checksum validation remains separately measured above budget, so telemetry work must make validation policy and scheduling explicit rather than hiding it inside the physics result.

## D-030: Serialize canonical telemetry records into caller-owned buffers

Status: Accepted

Decision:

Write telemetry without allocation into exact-length caller-owned buffers. A stream header is exactly 32 bytes and derives identity, timestep, and stride from a validated `Scenario`. A frame is exactly 40 bytes and carries strongly typed truth fields, private-construction status and event flags, the rolling state checksum, and a CRC-32 over its first 36 bytes. Reject noncanonical output lengths before writing any field.

Rationale:

This keeps binary layout independent of Rust struct representation, padding, and host endianness. Private flag storage prevents reserved bits from leaking into v1 records, while explicit constructors cover every accepted flag. The writers reproduce the independently generated 112-byte golden stream—including header and frame CRCs—byte for byte in native tests and the MOS self-test.

Consequence:

Canonical records can now be produced on every target, but the mission executor does not yet schedule or transport them. The next gate will emit initial and stride-aligned frames through a caller-supplied sink, accumulate events between frames, and keep checksum/serialization timing separate from the passing raw physics budget.

## D-031: Schedule telemetry through an observer on the single checked executor

Status: Accepted

Decision:

Extend the checked mission executor with an internal immutable observation boundary rather than creating a second physics loop. A caller-provided telemetry sink receives one canonical header, the initial truth, each configured-stride successor, and a final off-stride successor when required. Rolling exact-state checksums continue to cover every successful successor, not merely emitted states. Cutoff and propellant-depletion events accumulate until a sink accepts a frame; the terminal frame carries end-of-run. A numeric failure emits a terminal fault frame at the last valid truth and checksum when the sink permits it.

Sink rejection stops execution at the observed truth and reports its error with the accepted-frame count. If a sink also rejects a numeric-fault report, preserve both causes. Storage, transport, display, and retry policy remain outside the core.

Rationale:

One executor prevents dynamics, cutoff counting, checksumming, and fault semantics from drifting between ordinary and telemetry runs. Immutable observations preserve the private truth boundary, while fixed arrays and a sink trait keep the core allocation-free. Event accumulation prevents a transition between telemetry strides from disappearing. The independent Python oracle fixes the full golden stream at 257 frames, 10,312 bytes, and CRC-32 `0xcf56fe65`; native and MOS checks agree.

Consequence:

Canonical mission telemetry is now schedulable without choosing RAM, REU, disk, serial, or screen transport. The diagnostic C64 self-test grows to 47,447 bytes. The next gate must measure rolling checksum plus serialization scheduling with a discard sink under the established PAL common clock, separately from the already passing raw dynamics budget.

## D-032: Measure canonical telemetry separately from validation policy

Status: Accepted

Decision:

Measure three production paths in one PAL C64 diagnostic binary: checked dynamics, dynamics with a rolling checksum over every successor, and checksum plus canonical stride-aware telemetry. Force all serialized bytes to materialize through fixed volatile discard buffers, but exclude display, disk, REU, and serial transport. Attribute telemetry cost by subtracting checksum mode from the full telemetry path in the same binary; do not compare separate binary layouts for that delta.

Rationale:

Three stable runs measure canonical telemetry at an additional 15,309,372 cycles per 2,048-step mission: 7,475.28 cycles per physics step, 59,569.54 cycles per emitted frame, and 4.54 percent over checksum mode. The full recorded-validation path costs 172,152.59 cycles per physics step and reaches 5.72 Hz. Per-successor state hashing is substantially more expensive than telemetry serialization, so treating the entire miss as an I/O problem would direct optimization at the wrong subsystem.

Consequence:

The telemetry timing gate is closed without weakening the canonical stream. Recorded validation may run below real time, while future interactive scheduling may select a cheaper checksum cadence and retain the 8.57 Hz raw dynamics path. Transport implementations must be measured separately because this result includes only synchronous volatile discard copies. The next gate is host capture and strict stream inspection through the accepted sink boundary.

## D-033: Decode canonically in the core and inspect streams at the host boundary

Status: Accepted

Decision:

Implement exact-length telemetry header and frame decoders in the portable `no_std` core. Reject unknown versions, declared sizes, numeric contracts, reserved values, status/event bits, and CRC failures. Optionally bind a decoded header to a validated scenario by exact ID, timestep, and stride. Treat nonzero reserved v1 fields as an error rather than silently assigning future meaning to an old version.

Place file I/O, whole-stream validation, and interpreted text presentation in a separate `std` host crate. The host inspector requires the scenario-derived initial frame, valid step order and stride, exact step-derived mission time, fault/end pairing, terminal placement, and a successful final step equal to the scenario limit. It reports raw stream identity and checksums while using scaled decimal values only for presentation.

Rationale:

Keeping record decoding beside serialization creates one portable binary contract and allows MOS self-tests to exercise both directions. Keeping allocation, files, and formatting outside the core preserves the C64 architecture. Stream-level checks catch valid-CRC records assembled into an impossible mission, while acknowledging that skipped successors cannot be reconstructed from stride-sampled telemetry alone.

Consequence:

The host can now capture the golden 257-frame stream to a `.kst` file, read it back, reproduce stream CRC-32 `0xcf56fe65`, and display the final physical state. Corrupt records, bad cadence, terminal misuse, and I/O refusal fail closed. Exact replay remains distinct from structural inspection. The next gate is a C64 text-status sink that stores only the latest accepted frame and renders outside the accepted timing regions.

## D-034: Render retained mission status after the measured C64 run

Status: Accepted

Decision:

Implement the C64 display as a telemetry sink adapter, not as another mission executor or a display dependency in the physics core. Retain exactly one canonical header, the latest accepted frame, a frame count, and the union of accepted event bits. After mission execution completes, decode that retained data through the portable record contract and render a direct 40x25 status page in screen and color memory.

Treat the final footer as the display-complete sentinel for automated screen-memory inspection. Keep rendering outside both accepted timing regions and label the page accordingly. Preserve events across frames because cutoff and depletion can precede the terminal end-of-run frame.

Rationale:

The golden stream is 10,312 bytes, but the useful post-run status requires constant storage. Reusing the canonical sink boundary proves that the C64 view consumes the same telemetry contract as host capture. Direct VICE screen-memory verification catches PETSCII conversion, layout, formatting, retained-state, and partial-paint errors without adding VIC-II work to physics or serialization timing.

Consequence:

The 27,866-byte status PRG reports the complete 2,048-step mission, 257 accepted frames, final state, checksum, event history, and measured raw/recorded rates on a real C64 memory map. Live refresh remains deliberately unmeasured and out of scope. The next gate is independent high-precision comparison and C64 accumulated-error reporting.

## D-035: Attribute accumulated error with Decimal and refined RK4 paths

Status: Accepted

Decision:

Implement the Phase 1 high-precision comparison as a deliberately independent Python `Decimal` model at 80-digit precision. Run semi-implicit Euler at the product's 0.125-second step and update order to isolate fixed-point/table quantization. Separately run RK4 at 1/32 and 1/64 of the product step to estimate integration error and require the two refined results to agree within 1 mm altitude and 0.01 mm/s velocity.

Generate reviewed Q16.16 presentation constants for the total fixed-minus-confirmed-RK4 altitude and velocity deltas. Display those constants only after the C64 mission completes; they cannot influence dynamics. Retain semi-implicit Euler for Phase 1 and reconsider the integrator when Phase 2 defines orbital-insertion accuracy needs.

Rationale:

At 256 seconds, fixed-point execution differs from unquantized same-step Decimal by +7.842186 m and +0.042079 m/s. The same-step Decimal result differs from confirmed RK4 by -287.197179 m and -2.898800 m/s, so timestep/integrator bias dominates numeric quantization. The total fixed-point deltas are -279.354992 m and -2.856721 m/s. The two RK4 paths differ by only 0.006487 mm and 0.000037 mm/s, well inside their declared convergence bounds.

Consequence:

Phase 1 has measured, decomposed accumulated-error evidence rather than a generic tolerance. The C64 status PRG grows to 28,149 bytes and reports the rounded total deltas as -279.355 m and -2.857 m/s. Adding a second force evaluation would erase the accepted raw 8 Hz margin, while the measured error is adequate for this learning laboratory; new integrator work is deferred to Phase 2 requirements.

## D-036: Accept Phase 1 against the final linked binaries

Status: Accepted

Decision:

Close Phase 1 only after one completion runner passes the generated-artifact, native, exhaustive rust-mos, target-practical C64, raw timing, telemetry timing, high-precision, host-capture, and C64 screen-memory gates. Treat the final linked-layout measurements as authoritative while retaining earlier timing snapshots as optimization history.

Use a dedicated C64 acceptance pack for analytic motion, mass flow, environment, forces, transitions, the full checksummed mission, canonical records, and scenario ingestion. Keep the larger exhaustive arithmetic and whole-stream pack under `mos-sim`, where it remains practical. Report the C64 acceptance result persistently through masked VIC-II color registers.

Rationale:

The final audit caught both transient/readback mistakes in the C64 result harness and a whole-program timing shift after the last adapters were linked. Three stable final runs measure raw dynamics at 118,111.48 cycles per step (8.34 Hz, 4.10 percent headroom) and checksum plus canonical telemetry at 175,307.68 cycles per step (5.62 Hz). The exact mission checksum and telemetry stream identity remain unchanged.

Consequence:

All Phase 1 exit criteria pass. The C64 reports cycle rates, an 80-byte retained-sink footprint, and accumulated high-precision deltas on its verified post-run page. The accepted final artifacts are 49,773 bytes for the exhaustive diagnostic build, 38,519 bytes for C64 acceptance, and 28,353 bytes for the status program. Further vehicle dynamics begin in Phase 2.


## D-037: Represent Phase 2 in rotating equatorial polar coordinates

Date: 2026-07-22

Status: accepted.

Use radius, Earth-relative downrange angle, radial velocity, and inertial specific angular momentum as the planar truth variables. Derive tangential velocity from angular momentum and radius. Use a rotating spherical Earth, a co-rotating atmosphere, fixed-capacity constant-engine stages, and step-aligned time/pitch guidance. Preserve Phase 1 contracts and add versioned Phase 2 records.

This representation makes torque-free angular momentum a state invariant, avoids absolute Cartesian position transforms in the hot path, and still supports genuine energy, apogee, perigee, and orbit classification. The generated Phase 2 contract proves the declared field and intermediate envelopes and supplies target-executable trig and square-root checks.

## D-038: Retain semi-implicit Euler for the initial planar production path

Date: 2026-07-22

Status: accepted; the powered KSA-2A mission satisfies the declared insertion envelope.

Implement both semi-implicit Euler and midpoint RK2 at 0.125 seconds. The fixed-point circular-orbit acceptance case produces identical raw states after one orbit, and the changing-radius C64 fixture produces identical terminal radius and radial velocity. The final linked snapshot measures midpoint at 452,574.37 cycles per step versus 451,562.59 for semi-implicit Euler in three stable PAL VICE runs; the earlier snapshot remains layout-history evidence. Refined RK4 coast evidence converges far inside the declared threshold.

Use semi-implicit Euler because midpoint's correction quantizes away at the accepted field resolution while adding structure and target cost. Reopen the decision if the powered KSA-2A mission misses its insertion-error thresholds.

## D-039: Model aerodynamic flight in the co-rotating local frame

Date: 2026-07-22

Status: accepted.

Use a generated altitude table for atmospheric density and speed of sound, with the air mass co-rotating at the declared spherical-Earth rate. Derive radial and tangential air-relative velocity from planar truth, interpolate a Mach-dependent drag coefficient, and apply drag opposite that relative-velocity vector. Express commanded pitch as step-aligned binary-turn knots measured from local radial toward prograde, then resolve thrust with the generated Q1.15 trigonometric table.

This makes surface co-rotation an exact zero-dynamic-pressure fixture, keeps the environment and guidance deterministic and allocation-free, and lets one pure force evaluator expose Max-Q, Mach, thrust, drag, and acceleration without leaking mutable truth. Native and rust-mos self-tests exercise the generated environment identity, interpolation, drag direction, physical dynamic-pressure scale, pitch endpoints, thrust axes, and angular-momentum response. The table is a compact Phase 2 learning model rather than a named standard atmosphere; higher-fidelity atmosphere and winds remain future model choices.

## D-040: Freeze KSA-2A as a generated packed multistage mission

Date: 2026-07-22

Status: accepted.

Represent Phase 2 inputs as an 884-byte CRC-protected `KSC2` image with fixed capacities for four stages, sixteen pitch knots, four aerodynamic tables, and sixteen Mach knots per table. Validate framing, identities, reserved fields, ranges, event alignment, mass invariants, stage topology, guidance, and aerodynamics before constructing truth. Execute ignition delay, burn, cutoff, separation delay, residual-propellant disposal, and stage activation only on 0.125-second boundaries.

Use the generated KSA-2A schedule as the integrated nominal mission and a five-percent-short upper-stage burn as the deterministic failed-insertion mission. The independent float64 path reaches 199.989 x 200.015 km; the exact fixed-point path reaches 188.169 x 188.169 km, with Max-Q 40.779 kPa and peak proper acceleration 55.283 m/s2. Both satisfy the declared nominal envelope, while both implementations classify the early-cutoff case as impact.

Generate target fixture constants from the packed source rather than maintaining a second handwritten configuration. Split parser/contract and mission acceptance executables so each fits the 64 KB target link region. Run the complete 900-second nominal and failure missions natively, the complete nominal mission under rust-mos, and the target failure path through its exact cutoff; omit only the redundant post-cutoff atmospheric tail from the instruction-level failure check because its variable-time arithmetic is pathologically slow.

## D-041: Observe one mission executor through canonical KST2 records

Date: 2026-07-22

Status: accepted.

Expose immutable initial/successor observations from the authoritative Phase 2 executor. Compile rolling planar-state checksums only into observed execution, while the raw wrapper uses the same generic path with checksum work removed. Serialize a 40-byte scenario-bound `KST2` header and 64-byte fixed-point frames containing planar truth, command, stage state, Mach, dynamic pressure, pending events, exact-state checksum, and record CRC.

The nominal golden stream contains 901 frames and 57,704 bytes, with stream CRC-32 `0x7d13b2bf` and final state checksum `0xcc57612b`. The host rejects bad framing, CRC, scenario binding, initial truth, cadence, time, numeric ranges, and terminal placement before displaying values. Generated portable fixtures reproduce the header, initial frame, and terminal frame exactly.

Provide a GMAT R2026a point-mass script and report comparator for the independent float64 cutoff state, with Earth radius and force-model assumptions made explicit. Do not make GMAT a build dependency or claim an unexecuted external run as automated evidence. C64 retained-state replay will consume the same sink/decoder contract rather than adding a second mission executor.

## D-042: Measure slow target execution and replay compact presentation data

Date: 2026-07-22

Status: accepted.

Build Phase 2 C64 artifacts with a dedicated size-optimized Cargo profile while leaving the accepted Phase 1 release layout untouched. Measure raw and checksummed/KST2 execution together over the same eight powered sea-level steps under the PAL CIA common clock, with no real-time acceptance floor. The final linked paths run at 1,232,700.625 and 1,368,798.500 cycles per step respectively, so a complete validated target run is expected to take hours rather than minutes.

Do not recompute all 7,200 physics steps merely to paint the post-run display. Generate a compact `KRP2` presentation tape only from the host-validated canonical KST2 stream. Bind it to the source stream CRC, scenario, terminal checksum, full-mission Max-Q, orbit, its own CRCs, and a reviewed SHA-256. Preserve the canonical KST2 header and terminal frame inside the tape and decode them on the C64 through the portable contract. Use a generated table-driven CRC only for the cold replay-tape integrity check.

Replay the 901 compact points into a 40x25 altitude/downrange plot and drive bounded SID cues for ignition, cutoff, separation, end, and impact alarm events. Verify the final page directly from VIC-II screen memory and freeze the event-schedule hash. `KRP2` is a derived display index, never an alternative physics or regression record; a physical C64 run would transport its canonical KST2 output to replay storage rather than retaining the 57,704-byte stream in main RAM.

## D-043: Make the flight computer truth-blind by construction

Date: 2026-07-23

Status: accepted.

Put sensor, actuator, and flight-output records in `ksa64-interface`; allow `ksa64-flight` to depend only on that crate; and make `ksa64-sim` the sole composition root. Use fixed-width little-endian records with CRC-32, explicit enum validation, reserved-byte checks, sequence checks, and fail-closed parsing. This turns truth isolation and transportability into compile-time and binary contracts instead of naming conventions.

## D-044: Preserve KSA-2A and add only kinematic closed-loop steering

Date: 2026-07-23

Status: accepted.

Do not change Phase 2 vehicle physics, introduce rigid-body dynamics, or model recovery in Phase 3. Retain the trusted early pitch program, transition to closed-loop upper-stage insertion, and represent the actuator as bounded pitch motion with command feedback. Flight software owns requests; world rules retain physical authority. The recovery bit is transport-visible but has no Phase 3 force model.

## D-045: Use deterministic aided inertial navigation with declared sensor failures

Date: 2026-07-23

Status: accepted.

Model quantized accelerometer, gyro, clock, limited altimeter, and delayed GPS-like PVT measurements with deterministic bias/noise/fault schedules. Use allocation-free inertial propagation plus bounded alpha/beta-like corrections rather than an EKF. Freeze gains through a deterministic bounded host search and compensate the declared two-step GPS latency before correction. This is sufficient for the selected recoverable outages and remains inspectable on a 6510.

## D-046: Validate closed-loop missions independently from canonical bytes

Date: 2026-07-23

Status: accepted.

Record KST3 as a scenario/config-bound stream containing truth, measurements, estimates, commands, actuator feedback, events, alarms, record CRCs, and four rolling checksum chains. Require the Rust host inspector to reject the first structural or semantic fault. Separately parse those bytes in Python and compute float64 orbit, coast, load, and navigation evidence. Generate KRP3 only through the strict host inspection path.

The independent audit is authoritative for Phase 3 orbit acceptance. It exposed a coarse fixed-point orbit-classification overstatement and delayed-GPS navigation lag before completion; both were corrected without changing the Phase 2 vehicle.

## D-047: Abort fail closed on sustained actuator disagreement

Date: 2026-07-23

Status: accepted.

During insertion, latch abort when commanded-versus-applied pitch error exceeds two degrees for 16 consecutive steps. Abort requests cutoff, inhibits subsequent ignition and separation, sets safeing and the transport-only recovery request, and continues the world ballistically. Invalid sensor transport uses the same latched fail-closed state.

## D-048: Gate full C64 missions before starting them

Date: 2026-07-23

Status: accepted.

Measure naturally terminating 64-step PAL probes for representative composed, guidance, fault, coast, and actuator paths. Start a full nominal target mission only if the linked program fits stock RAM and the conservative pre-run projection is no more than 30 minutes. The accepted program fits, but its 243.7-minute projection exceeds the threshold, so no full mission is started. Verify presentation separately with strict KRP3 parsing, PETSCII rendering, SID cues, and screen-memory inspection. Never cancel a run to manufacture timing evidence.

## D-049: Make campaign variation keyed and order-independent

Date: 2026-07-23

Status: accepted.

Derive every sampled value from the master seed, run index, parameter identity, correlation group, and draw index. Run zero is always the unmodified Phase 3 nominal case. Catalog order, worker count, and execution order therefore cannot change a run's inputs.

Use bounded uniform, triangular, Bernoulli, and clamped 12-draw CLT-normal distributions with explicit physical bounds. Keep controller gains, vehicle topology, event sequencing, and probabilistic fault topology outside the reviewed Phase 4 catalog until they receive coupled-invariant tests.

## D-050: Aggregate campaigns in canonical run order

Date: 2026-07-23

Status: accepted.

Allow native workers to execute runs in parallel, but sort results and fold fixed 128-byte KSR4 summaries strictly by run index. Keep streaming count, extrema, mean, variance, histogram, and ordered summary-chain state allocation-free. Treat the independently reconstructed Python/float64 campaign analysis—not the compact C64 classifier—as physical acceptance evidence.

The frozen 1,024-run campaign has identity `0xa2e9e9d5` and ordered summary chain `0x813ce420` across serial, 5-worker, and 12-worker execution.

## D-051: Keep the REU optional and storage observational

Date: 2026-07-23

Status: accepted.

Support stock C64 operation with streaming aggregates, five deterministic interesting-run summaries, and one sparse KPH4 history. Detect REU capacity with preserving DMA probes; permit users to disable or cap detected storage but never claim more capacity than observed. Convert available capacity into summaries, full KST4 histories, and compact KPH4 histories through one deterministic `StoragePlan`.

No simulation state, random draw, flight checksum, or campaign aggregate may depend on recording mode, detected capacity, DMA timing, archive success, or storage failure.

## D-052: Version Phase 4 evidence independently

Date: 2026-07-23

Status: accepted.

Preserve KSC2, KSC3, KST3, and KRP3 unchanged. Add separate strict KSC4 campaign configurations, KSR4 summaries, KPH4 presentation histories, run-bound KST4 detailed streams, committed KRA4 archives, and numbered KXV4 export volumes. Reject unknown meanings, nonzero reserved data, corruption, truncation, identity mismatch, and incomplete record chains rather than guessing.

KPH4 and the compact fixed-point orbit outcome are presentation and selection aids. They do not replace canonical detailed telemetry or independent float64 analysis.

## D-053: Keep IEC export post-run and separable

Date: 2026-07-23

Status: accepted.

Build exports from an explicit manifest, reject oversized one-volume selections before writing, and bind multi-volume order and logical offsets in KXV4 headers. Require the host joiner to reject missing, duplicate, reordered, mixed, truncated, or corrupt volumes.

Keep a small stock report path in the campaign application and place full archive IEC writing in a separate utility PRG. Disk commands, retries, or failures must never enter the simulation loop or consume its state budget.

## D-054: Validate large campaigns natively and probe the target finitely

Date: 2026-07-23

Status: accepted.

Use native execution for the 64-run routine campaign and 1,024-run reviewed campaign. Use bounded MOS/VICE probes for exact arithmetic, storage, DMA, UI, archive recovery, and IEC export. A complete target campaign is not a completion requirement.

The accepted composed path projects one C64 mission at 243.7 minutes, 64 runs at approximately 10.8 days, and 1,024 runs at approximately 173.3 days. Start any long target run only after a current projection and explicit user confirmation; never cancel a run merely to obtain timing evidence.

## D-055: Guide KSA-5A in the local launch plane

Date: 2026-07-23

Status: accepted.

Generate a bounded quaternion table whose inertial tilt includes both the reviewed local pitch schedule and reference downrange rotation. Use a 42.4-degree east-of-north launch azimuth so the final inertial plane accounts for the launch site's eastward rotational velocity. Command stage-two cutoff at step 3132. The independent float64 audit, not the compact fixed-point orbit classifier, decides compliance with the 180-220 km and inclination envelopes.

## D-056: Use body-frame attitude error without breaking Gate 7

Date: 2026-07-23

Status: accepted.

Compute the reviewed mission controller's quaternion error as current-conjugate times desired so its error vector is expressed in body axes. Project the one-frame-late star-tracker attitude to the current gyro epoch before aiding. Keep the Gate 7 legacy exact path selectable through its frozen gain profile; its transport/controller signature remains `0xaa0a0b0e`.

## D-057: Freeze six integrated Phase 5 missions before telemetry

Date: 2026-07-23

Status: accepted.

Freeze nominal, gust/slosh, star-outage/gyro-bias, gimbal-jam, damping-loss, and RCS-leak/depletion outcomes and checksum summaries. Nominal and gust must meet the reviewed targeting envelope. Sensor outage and RCS depletion may be stable degraded orbits; jam and damping loss must latch irreversible abort. A finite rust-mos guidance probe is required, but a complete C64 mission remains subject to projection and explicit confirmation.

## D-058: Record KST5 through the single spatial mission executor

Date: 2026-07-23

Status: accepted.

Add an observer boundary to the reviewed Phase 5 mission loop and make both ordinary and telemetry runs use it. Emit one 96-byte KST5 header and one 424-byte frame for initial truth and every committed 0.125-second successor. Embed the already strict spatial sensor and actuator records, preserve their CRCs, and add frame CRC plus a rolling observation chain. Host inspection and an independent Python parser must agree on the complete nominal stream. Target acceptance uses only a finite codec probe; it must not start a full C64 mission.

The frozen nominal stream contains 3,134 frames and 1,328,912 bytes, with CRC-32 `0xa9b3b94c` and terminal observation checksum `0x5b7b2419`. The size-optimized rust-mos codec probe is 16,778 bytes and signature `0x07bc3e16`. KST3 and KST4 remain unchanged.
## D-059: Extend keyed campaigns to the spatial vehicle

Date: 2026-07-23

Status: accepted.

Reuse the reviewed Phase 4 distribution families and keyed-draw semantics, but
version the spatial configuration and summary independently as KSC5 and KSR5.
Keep run zero byte-for-byte equivalent at the mission-summary boundary to the
frozen Phase 5 nominal path. Vary payload, stage thrust, atmosphere, aerodynamic
scale, spatial sensor errors, and gimbal lag/slew through explicit parameter
objects; do not mutate guidance gains, mission topology, or event sequencing.

Run the 32-run routine and 256-run reference campaigns natively, merge summaries
strictly by run index, and use a finite codec/sampling rust-mos probe instead of
starting a target campaign. Serial and eight-worker reference artifacts are
byte-identical with ordered KSR5 chain `0x3103d833`. Independent reconstruction
finds no numeric or step-limit failures. Preserve the 48 safe aborts as evidence
that the frozen controller is sensitive to reviewed actuator dispersion; do not
tune them away before the target timing gate measures representative kernels.
## D-060: Measure Phase 5 as split stock-compatible kernels

Date: 2026-07-23

Status: accepted.

Measure the four-substep vehicle, spatial avionics, and canonical KST5 observer
as separate naturally terminating PAL C64 executables. Require every executable
to fit below `$c000`, repeat each measurement three times, and compare its exact
outputs with a native build. Sum the regions and add a ten-percent margin for
the full-mission decision. A monolithic timing image is not a stock-memory
requirement.

The final measured sum is 20,268,920 cycles per 0.125-second mission step. A
3,133-step nominal mission projects to 17.90 hours, or 19.69 hours with margin,
so it is not eligible for automatic execution and was not started. Accept the
exact zero-variation scale fast path, which saves 30,264 cycles per step. Reject
and revert the faster stage-inertia division because rust-mos telemetry exposed
native/target divergence. Never cancel a run for timing evidence.
## Open decisions

The following remain deliberately unresolved:

- License for KSA64.
- The Phase 6 physical transport and multi-C64 deployment details.
- The Phase 7 mission set and data-driven configuration scope.

Simulation-rate requirements remain phase- and evidence-specific rather than one global threshold. Stock and 128 KiB through 16 MiB REU storage configurations are accepted Phase 4 decisions, not open architecture questions.

## D-061: Keep adaptive Phase 5 history observational and independently versioned

**Decision.** Stock hardware retains a streaming aggregate, five deterministic KSR5 summaries, and one stride-32 KPH5 baseline. REU capacity increases summary retention and then stores selected complete KST5 reruns before stride-8 KPH5 reruns. Reuse Phase 4 REU detection and DMA, but use new KPH5 and KRA5 identities. Storage failures may make evidence incomplete but cannot influence mission parameters, execution order, checksums, or later seeds.

**Reason.** A 1.33 MB KST5 mission cannot fit stock RAM and does not fit smaller REUs. A 1.66 KB spatial history preserves useful mission-control presentation on every C64 while a strict capacity ladder consumes whatever optional hardware is actually present. Separate format identities prevent Phase 4 readers from silently accepting spatial data. The first PAL target matrix rejected a quotient-based planner that passed native and `mos-sim`; bounded loops replaced its general divisions and preserve the exact allocation results with a hard 256-iteration ceiling.

## D-062: Replay KPH5 directly for stock Phase 5 presentation

**Decision.** Use the already strict, bounded KPH5 stream as the stock mission-control tape. Add a portable reducer for identity, extrema, events, and cue hashing, then a C64-only VIC-II/SID adapter. Project quantized Y–Z coordinates with fixed shifts and reviewed bounds. Do not derive a second replay format and do not recompute dynamics for display.

**Reason.** The complete KPH5 baseline is only 1,664 bytes and already binds its source run through two CRC layers. Direct replay avoids redundant formats, fits far below stock-RAM limits, and keeps visualization clearly subordinate to KST5 and independent physical analysis.

## D-063: Complete Phase 5 with bounded target evidence

Date: 2026-07-23

Status: accepted.

Close Phase 5 only after one runner validates inherited Phase 4 evidence, every
Phase 5 independent/native gate, all finite rust-mos and PAL VICE probes, and
all frozen artifact hashes. Do not make a complete C64 mission or campaign an
exit criterion: the accepted conservative projection is 19.69 hours for one
nominal mission, 26.26 days for 32 runs, and 210.07 days for 256 runs.

Treat the accepted single-machine world/flight composition as the Phase 6
regression oracle. Phase 6 may add framed physical transport, explicit latency,
timeouts, and replay, but it may not duplicate the physics or expose truth to
flight software. Leave the electrical transport and multi-machine scheduling
contract open for dedicated planning.


## D-064: Preserve exact Phase 5 behavior through KLF6 and version KSA-6R separately

Date: 2026-07-24

Status: accepted.

Use allocation-free world and flight endpoints plus KLF6 to reproduce the accepted Phase 5 mission exactly. Give the stock-oriented 32/8/1 Hz flight profile its own KLR6 cells and KSA-6R evidence instead of silently changing Phase 5 timing or measurements.

## D-065: Make one C64 plus a host the Phase 6 baseline

Date: 2026-07-24

Status: accepted.

Require every C64 endpoint to fit stock memory and require no REU. Support two- and three-C64 arrangements as optional endpoint placements. Keep the host, another C64, or future hardware free to own the world and Mission Control roles without changing the flight contract.

## D-066: Treat Mission Control and storage as passive

Date: 2026-07-24

Status: accepted.

Mission Control may consume delayed/noisy telemetry and calculate an independent ground estimate, but it cannot command the vehicle in Phase 6. Transcript, UI, archive, and REU behavior must not alter command ordering, checksums, or world state.

## D-067: Accept binary-monitor mailboxes only as target-exactness evidence

Date: 2026-07-24

Status: accepted.

Use a VICE-only mailbox to execute and shadow-verify the complete stock-C64 flight endpoint after the pinned Windows VICE ACIA receive path proved unusable. Because monitor transactions pause emulation, label this evidence externally paced and never present it as live transport timing. Keep physical SwiftLink/Turbo232, Ultimate, and user-port acceptance open.

## D-068: Make cross-target checksums byte-explicit

Date: 2026-07-24

Status: accepted.

Hash signed control values from explicit little-endian bytes and sign bytes rather than compiler-dependent widening. Split target loops whose bound is 256 into two bounded 128-byte spans. Compare every target command and status cell with a native shadow endpoint and retain 1,024-epoch checkpoints for early divergence detection.

## D-069: Close the Phase 6 software baseline without claiming physical-link completion

Date: 2026-07-24

Status: accepted.

Completion requires exact native splitting, deterministic link failures, passive ground systems, finite PAL timing, stock endpoint packaging, and one complete externally paced 1x PAL target flight. It does not require possession of multiple C64s or an REU. A finite run on actual compatible link hardware remains a documented follow-up rather than being hidden or inferred.


## D-070: Package host-first Phase 6 deployments without inventing missing endpoints

Date: 2026-07-24

Status: accepted.

Provide one launcher for host-world/host-flight and host-world/VICE-flight arrangements, with optional passive host Mission Control and fast, 32 Hz wall-paced, or manual-step execution. Reject VICE-world and multi-VICE selections until actual C64 world or Mission Control endpoint programs exist. Run native endpoints across the same TCP cell seam used by hybrid deployments rather than collapsing them into a privileged combined simulator. Mission Control may observe validated cells and independent delayed/noisy ground fixes but cannot issue commands; enabling it must preserve all terminal and avionics evidence.


## D-071: Keep live Mission Control host-native and passive

Date: 2026-07-24

Status: accepted.

Build the live F1–F7 presentation as a host-only Ratatui consumer of validated KLR6 and independent ground products. Reserve omniscient truth for a clearly labelled SIM Director page. Do not change KLR6, C64 memory budgets, physics, or flight-software scheduling to serve the display. Make realtime/step runs interactive, fast runs summary-oriented, and preserve explicit display overrides.

Record all presentation inputs in a host-only noncanonical KMR6 stream with per-record CRC-32, prefix recovery, replay, and CSV/JSON derivation. Keep KST5 and KLR6 authoritative. Operator pacing may delay releases but may not alter their order; stopping is explicit, and detaching returns the console while the mission and recorder continue headlessly.

## D-072: Give host Mission Control rich views without weakening provenance

Date: 2026-07-24

Status: accepted.

Use the strictly validated frozen KPH5 nominal history as the planned ascent and the accepted nominal terminal state as its orbit target. Render onboard navigation and the independent delayed/noisy ground estimate as separate observed paths. Label derived orbital, geographic, atmospheric, and load products as MODEL EST. Reserve omniscient world truth for F7 and enforce that F1 through F6 are render-invariant under arbitrary SIM Director truth changes.

Keep the upgrade host-only and presentation-only. Retain full ordered history for live/replay plots; make replay history prefix-exact; provide Ascent, Orbit, and Ground Track views plus responsive 80x24 through ultra-wide layouts and ASCII/Braille plotters. Do not change KLR6, KMR6, KLF6, KST5, C64 memory budgets, flight scheduling, or command authority.

## D-073: Split reusable missions, spatial hobby flight, and optimization

Date: 2026-07-24

Status: accepted.

Use Phase 7 to add explicit profile identities, bounded compiled packs, and a
credible vertical hobby/high-power evaluator. Preserve KSA-2A and KSA-5A by
calling their frozen executors through an additive facade rather than
refactoring them into a universal vehicle model. Treat KSA-6R as a realtime
flight/link profile over the KSA-5A physical world.

Reserve Phase 8 for component geometry, mass properties, 3-D hobby flight,
stability, wind, weathercocking, recovery drift, and external correlation.
Reserve Phase 9 for host-side optimization, Pareto and sensitivity analysis,
robust design campaigns, and result browsing. Keep broader central-body,
rendezvous, entry, landing, and tracking missions in an unassigned backlog
until a concrete experiment defines their required fidelity.

## D-074: Use a published-data Firestorm/I211W Phase 7 reference

Date: 2026-07-24

Status: accepted.

Use the published Giant Leap Firestorm 54 dual-deploy dimensions, dry weight,
recovery sizes, and recommended AeroTech I211 pairing together with the
public-domain, TRA-test-derived I211W RASP curve hosted by ThrustCurve. Commit
normalized inputs, source identity, retrieval date, attribution, license, and
checksums so builds remain offline and reproducible.

Use the selected curve's own motor and propellant masses rather than mixing it
with differing current product specifications. Label the rail, body Cd,
canopy Cd, deployment triggers, and inflation times as KSA64 modeling
assumptions. This is a published-data reference configuration, not
flight-correlation or certification evidence.

## D-075: Keep Phase 7 vertical, typed, and target-bounded

Date: 2026-07-24

Status: accepted.

Give HobbyVerticalV1 its own generated SI fixed-point contract and phase-aware
0.01/0.02/0.05-second schedule. Use typed mission phases plus orthogonal rail,
motor, and recovery states rather than a universal mission bytecode. The
portable evaluator reports physical metrics and validity bits without imposing
a safety, regulatory, or optimization score.

Link one profile per C64 image, require no REU, and retain only one summary and
an approximately 2 KiB plot on stock hardware. Run target jobs sequentially
with guaranteed VICE cleanup. Require a complete target mission only when the
measured projection is at most 30 minutes; otherwise use exact finite probes,
and never cancel a run merely because it is taking a long time.
## D-076: Keep table addressing target-width independent

Date: 2026-07-24

Status: accepted.

Perform fixed-point physical arithmetic in its declared signed word width and
convert only a proven bounded table quotient to `usize`. Never cast raw
fixed-point values or raw strides to `usize` before arithmetic: `usize` is 64
bits on the development host and 16 bits on the C64.

A 129-state native/MOS trace is permanent Phase 7 completion evidence. It found
the original environment lookup divergence at step 24, when a 2,048,000-raw
250 m stride truncated to 16,384 on the target. The repaired full stock-C64
mission must reproduce the host state checksum through ground contact.

## D-077: Add spatial hobby flight as a separate profile

Date: 2026-07-24

Status: accepted.

Preserve `HobbyVerticalV1` byte-for-byte as the inexpensive evaluator and add `HobbySpatialV1` with separately generated units, packs, steps, validity flags, and evidence. Do not widen or reinterpret KSR7; use KSR8 for the 32-metric spatial summary.

## D-078: Require provenance and explicit model envelopes

Date: 2026-07-24

Status: accepted.

Compile only bounded derived quantities onto the target while retaining each source value's published, measured, assumed, or derived provenance on the host. Reject flight beyond Mach 0.8, 15 degrees angle of attack, or the accepted Firestorm environment range. Never hide an external disagreement by tuning an undocumented coefficient.

## D-079: Retire attitude at recovery deployment

Date: 2026-07-24

Status: accepted.

Use rail-constrained motion followed by full six-degree-of-freedom ascent and coast. At first recovery deployment, preserve position and velocity but retire attitude dynamics and continue with a three-dimensional point-mass canopy model. Suspended-body and canopy-pendulum dynamics remain explicit non-goals.

## D-080: Treat external tools as independent evidence

Date: 2026-07-24

Status: accepted.

Align geometry, mass, motor, atmosphere, rail, recovery, and randomness settings with OpenRocket 24.12, preserve all inputs and exports, and compare declared metrics. OpenRocket is corroborating evidence rather than production truth. Incomplete historical Firestorm records are qualified context and cannot silently become acceptance data.

## D-081: Keep Phase 8 stock-compatible and bound target runs by evidence

Date: 2026-07-24

Status: accepted.

Embed generated pack constants, retain the exact Phase 7 environment prefix through 3 km, and link mission, exact-trace, and replay images below `$C000` without requiring an REU. Place the probe mailbox at `$C800` to avoid rust-mos static-stack storage. Measure finite PAL kernels first; because the full mission projects to 2.35 hours, do not start it without explicit user confirmation.

## D-082: Name model profiles by mathematics, not user category

Date: 2026-07-25

Status: accepted for Phase 8.5 implementation.

Use `VerticalPointMassV1` and `LocalEnu6DofV1` as the canonical public names for the frozen profile identities historically called `HobbyVerticalV1` and `HobbySpatialV1`. Preserve numeric discriminants, wire bytes, checksums, parsers, and source compatibility aliases. Model choice follows the required physics and coordinate envelope; labels such as model, high-power, sounding, experimental, and orbital describe vehicles or missions, not numerical profiles.

## D-083: Stabilize shared avionics before optimization

Date: 2026-07-25

Status: accepted as the Phase 8.5 roadmap boundary.

Insert Phase 8.5 before Phase 9. Bind every future evaluation to vehicle, physical/model profile, frame, mission, environment, avionics, actuator capabilities, uncertainty, and evaluator identity. Reuse one scheduler and avionics architecture across orbital and local vehicles through profile-specific navigation, guidance, sequencing, and capabilities.

Preserve both host-world/C64-avionics operation and a deliberately long combined-C64 world/avionics option. Both use identical next-epoch endpoint semantics; the monolithic build replaces serialization with an in-memory loopback. Keep the accepted Phase 8 standalone-world image even if the combined stock image requires later optimization or banking. Reserve ECEF/ECI transformations now, but defer global atmospheric propagation to Phase 10.

## D-084: Use an exact event clock and defer advanced effectors

Date: 2026-07-25

Status: accepted for the Phase 8.5 and Phase 9.5 roadmap.

Run the Phase 8.5 avionics-aware local executor from an exact 32 Hz event clock. Treat the existing physical timesteps as maxima and split them at exact avionics releases and mission-event boundaries. Preserve the frozen Phase 8 executor as a separate compatibility path. Sensor N produces command N, command N becomes effective at release N+1, and continuous commands remain held between releases across host, VICE, and monolithic loopback placements.

Prove active local control with an explicitly fictional two-axis motor-gimbaled Firestorm derivative while the real Firestorm remains monitor-only for attitude. Separate common guidance and body-control demand from a statically selected, capability-bound control allocator. Phase 8.5 implements only monitor-only and motor-gimbal allocation; unsupported effector families fail closed.

Add Phase 9.5 after the optimization workbench and before global flight to implement aerodynamic canards, cold-gas RCS, and mixed-effector allocation. Use Phase 9 to size, tune, compare, and robustly evaluate these models rather than expanding Phase 8.5 into multiple new vehicle-physics projects.


## D-085: Stop the combined stock image at the physical-capacity boundary

Date: 2026-07-25

Status: accepted.

The smallest self-contained combined Phase 8.5 world-plus-avionics link requires 71,500 resident bytes. RAM hidden beneath I/O and KERNAL cannot make this fit in 64 KiB, and forcing major boundaries out of line increased size. Stop at the plan's explicit decision boundary. Preserve the stock Phase 8 world and 15,412-byte stock flight endpoint; do not silently require an REU, remove avionics, or select disk overlays or a separate hand-specialized executor.

## D-086: Hold launch-rail attitude and declare a bounded fictional gimbal installation

Date: 2026-07-25

Status: accepted.

The active local controller holds the measured initial launch-rail attitude rather than coordinate-zero attitude. The fictional derivative declares a 20 g actuator installation, Q15 proportional gain 14,000, Q15 derivative gain 4,096, plus the frozen travel, slew, lag, pivot, rail, and burnout limits. This assumption-backed derivative is separate from the published Firestorm and exists to exercise the allocator. Its 5 m/s crosswind case must remain inside the Phase 8 envelope and reach <=3 degrees rail-relative error within eight releases.

## D-087: Keep optimization outside the production evaluator

Date: 2026-07-25

Status: accepted.

Compile bounded design manifests on the host, materialize identity-bound candidate packs, and evaluate them through the unchanged Phase 8.5 avionics-aware boundary. Search engines, worker scheduling, archives, reports, and external tools may choose candidates but cannot implement physics or access private truth.

## D-088: Use feasibility-first deterministic search

Date: 2026-07-25

Status: accepted.

Evaluate all candidates nominally, use the same ordered eight cases for search, and promote deterministic terminal finalists to all 64 Phase 8.5 cases. Feasible candidates always dominate infeasible candidates. Use exact fatal-class, violation-count, i128 normalized-violation, and candidate-identity ordering for infeasible candidates; never turn hard constraints into weighted penalties.

Freeze GridV1, Nsga2V1, and DifferentialEvolutionV1 identities. Any proposal or selection change requires a new engine identity.

## D-089: Commit optimization evidence at generation boundaries

Date: 2026-07-25

Status: accepted.

Merge evaluations by candidate and uncertainty index, then commit one complete KRA9 segment per generation or grid batch. Resume accepts only an exact completed prefix and must reproduce uninterrupted archive bytes. Retain KAS8 per-case evidence inside independently protected KRE9 records. Live progress publishes only after deterministic boundaries.

## D-090: Keep production search off the C64

Date: 2026-07-25

Status: accepted.

Use the C64 for bounded finalist browsing, presentation, and selected exact reruns through the accepted split flight endpoint. Do not run production optimization on the target or make REU capacity part of candidate identity. The 15,391-byte stock browser satisfies packaging and its frozen finite one-instance VICE probe validates four finalists without leaving an emulator process.

## D-091: Keep Phase 9.5 models native and external checks secondary

Date: 2026-07-25

Status: accepted for Phase 9.5 planning.

Keep canard, RCS, depletion, changing mass-property, actuator, control-allocation, and authority-handoff models in the portable KSA64 evaluator. Use analytic cases and a small independent float64 implementation as primary evidence.

Basilisk may provide optional frozen secondary fixtures for selected fixed-step spacecraft-attitude and RCS force, torque, pulse, depletion, or mass-property cases. It is not an oracle for KSA64-specific canard aerodynamics, the exact 32 Hz event scheduler, mixed-effector allocation, or authority handoff. It must not become a runtime, build, or CI dependency, and Phase 9.5 will not grow Phase 10 tooling solely to prepare for later global flight.

## D-092: Freeze an offline Earth, time, frame, and validator contract before Phase 10 dynamics

Date: 2026-07-25

Status: accepted for Phase 10 planning.

Keep `GlobalEcef6DofV1` authoritative for KSA64 and maintain one independent float64 implementation of its complete accepted model. Use SatKit as the preferred specialized offline reference for time, Earth orientation, frames, gravity, and selected coast cases; escalate to Orekit only for a demonstrated coverage or transform need; use GMAT occasionally for exoatmospheric or near-orbital corroboration.

Before global dynamics are accepted, declare and version the reference ellipsoid, gravity and Earth-orientation models, supported time scales, continuous integration time, leap-second source, EOP dataset and validity/extrapolation policy, transform conventions, and permitted simplifications. External tools generate frozen, provenance-complete fixtures; normal tests use those fixtures without external tools, network access, or live data.

Only one model owns an entity's state in any interval. External validators never co-propagate, correct, or replace the production state. Phase 10 chooses the smallest Earth/time model that meets its declared mission envelope after range and accuracy analysis; this decision records the required contract, not a premature fidelity choice.

## D-093: Use exact pulse edges and physical per-jet RCS forces

Date: 2026-07-25

Status: accepted.

Represent RCS commands as zero through eight 1/256-second quanta per jet in each 32 Hz successor interval. Valve edges and exact depletion are world split points. One-shot pulses are never replayed. Apply every installed jet force at its physical location; nominally balanced pairs do not erase residual translation caused by mismatch, failure, or quantization.

Compile both regulated and ideal-isothermal blowdown sources into bounded remaining-propellant supply tables consumed by one portable interpolation path. The accepted Firestorm RCS derivative uses blowdown and protects a 20-percent reserve.

## D-094: Freeze PriorityResidualV1 as the accepted mixed allocator

Date: 2026-07-25

Status: accepted.

Keep the Phase 8.5 local flight computer unchanged. An additive advanced wrapper translates its two-axis demand, adds roll demand, and creates a physical three-axis torque request. `PriorityResidualV1` consumes vehicle-compiled effectiveness and authority tables, allocates in a declared group order, predicts achieved torque after quantization and saturation, and passes the exact residual onward.

Use motor gimbal, canards, then RCS during powered flight; canards then RCS after burnout while aerodynamic authority exists; and RCS at low dynamic pressure. Pitot loss selects a conservative truth-blind navigation/atmosphere fallback. If that estimate is invalid, canard authority fails unavailable and the residual passes onward.

## D-095: Require two separate stock-C64 advanced endpoints

Date: 2026-07-25

Status: accepted.

Support host-world/C64-flight and C64-world/host-flight in addition to host/host. The flight and world remain separate stock images so a user with one physical C64 can choose either role. Neither endpoint may require an REU. The advanced flight endpoint retains the 24,631-cycle PAL release gate; the world endpoint may execute slower than simulated real time.

Do not reopen the impossible combined stock world-plus-avionics image. Do not silently lower rates, move allocation to the host, remove effectors, or require expansion if a stock endpoint misses its gate.

## D-096: Defer deliberate six-axis guidance

Date: 2026-07-25

Status: accepted.

Phase 9.5 guidance commands physical roll, pitch, and yaw torque only. RCS translation remains a physical consequence of individual jet forces but is not intentionally commanded. Reserve `SixAxisWrenchV1` for future docking, station keeping, rendezvous, and propulsive-landing missions so those uses can define their own translation, authority, navigation, and safety contracts.


## D-097: Use externally paced C64 flight as the interim Phase 9.5 stock baseline

Date: 2026-07-25

Status: accepted; reschedules but does not erase D-095.

Use host-world plus stock-C64-flight step-and-ack execution as the accessible Phase 9.5 baseline. The host remains the physical authority, releases exact KLR9 sensor cells at simulated 32 Hz epochs, waits for the C64 to execute the genuine advanced flight and allocation kernels, shadow-verifies returned command/status bytes, and only then advances the world. This preserves event and successor-command semantics but is explicitly not a realtime claim.

Realtime C64 flight and the C64-world endpoint remain priority follow-on tracks rather than Phase 9.5 blockers. Preserve the measured PAL and stock-fit deficits. Investigate a measured 6502-specific rewrite and C64 Ultimate acceleration/integration as distinct future strategies; neither may silently change canonical physics, command ordering, or evaluation identity. Keep the portable C64 world and deliberately long target-run objective on the roadmap while prioritizing host-world/C64-flight, Mission Control, storage, and finalist workflows now.

## D-098: Configure selected finalists through an additive flight bootstrap

Date: 2026-07-25

Status: accepted.

Keep the frozen Phase 9.5 reference flight endpoint unchanged. Configure selected optimized canard, RCS, and mixed candidates through a separate stock-C64 flight image and a strict 352-byte KFB9 payload in the KLF6 Start frame. KFB9 binds the manifest, study, candidate, vehicle, effector, and allocator identities and carries only the bounded flight and allocation configuration. KPE9/KPA9 remain the design evidence and the host remains world authority.

Require the host to materialize and validate the candidate, construct the same portable flight/allocator configuration used by its shadow, and compare every returned KLR9 command and status cell before advancing the world. Treat F1–F7 Mission Control, KMR9 recording, KFE9 browsing, stock/REU retention, and replay as passive. Keep the selected-finalist endpoint stock-compatible and REU-independent; expansion capacity may increase retained histories only.


## D-099: Close Phase 9.5 on the exact-paced stock-flight baseline

Date: 2026-07-25

Status: accepted.

Close Phase 9.5 after the complete frozen regression, independent model checks, fresh accepted workbench reproduction, MOS packaging, passive presentation validation, and finite baseline/canard/RCS/mixed stock-C64 probes pass. Treat host-world plus externally paced stock-C64 flight as the accepted interim deployment baseline without representing it as realtime.

Preserve the measured realtime-flight and portable-world deficits as explicit follow-on work. Do not lower the 32 Hz release rate, move allocation to the host, remove effectors, or require an REU to manufacture a target pass. Carry the measured 6502-specific rewrite, C64 Ultimate integration, portable C64-world long-run, and physical-link tracks forward. Begin Phase 10 at the global frame/time contract and fixture boundary described in the Phase 10 handoff.

The completion audit launches at most one VICE instance, closes it after success or proven failure, and waits 20 seconds between sequential Windows emulator processes after a startup-only transient was observed. It never starts a complete target mission without a fresh projection and explicit confirmation.

## D-100: Freeze the accepted global Earth and time authority

Date: 2026-07-26

Status: accepted.

Use `GlobalEcef6DofV1` with WGS 84 ellipsoidal geodesy, `CentralJ2V1`,
compiled IERS 2010 / IAU 2006–2000A transforms, elapsed TAI integration, and
the pinned 2024-01-01 epoch. ECEF owns atmospheric ascent and entry; GCRF owns
exoatmospheric coast. Source coverage is identity-bound and extrapolation
fails before propagation.

## D-101: Split physical-world and avionics validation authority

Date: 2026-07-26

Status: accepted.

Use a separate float64 implementation to validate the complete
uninstrumented/prescribed-attitude physical path. Validate controlled global
flight independently through truth-blind navigation/control tests, exact link
cells and checksum chains, named faults, analytic controller evidence, and
campaigns. Do not compare a prescribed-attitude reference directly to a
closed-loop controlled mission and mislabel controller differences as physics
error.

SatKit/Orekit/GMAT remain frozen fixture generators or corroborators. They
never own runtime state.

## D-102: Accept externally paced stock-C64 global flight without a realtime claim

Date: 2026-07-26

Status: accepted.

Use host-world plus stock-C64-flight as the Phase 10 hardware baseline. The
37,403-byte flight endpoint, 35,247-byte timing program, and 17,002-byte replay
fit below `$C000` without an REU. Exact step-and-ack preserves logical epochs
and successor-command ordering, but measured release costs of 54.9 through
114.1 PAL slots explicitly fail realtime operation.

Defer the portable stock global world, 6502-specific optimization, C64
Ultimate acceleration, and physical-link acceptance without changing the
canonical global evaluator.

## D-103: Correct global event qualification and declare terminal-contact tolerance

Date: 2026-07-26

Status: accepted.

Qualify apogee from velocity projected onto WGS 84 geodetic up rather than a
single quantized altitude decrease. After ECEF-to-local recovery handoff,
qualify main deployment against recovery-site local AGL rather than
ellipsoidal altitude.

The independent model keeps the one-step bound for rail clearance, burnout,
apogee, drogue, main, and all frame transitions. Terminal ground contact has a
separate four-recovery-step (0.125 s) tolerance for accumulated fixed-point
descent quantization; the accepted difference is 0.09375 s. This deviation is
explicit evidence, not coefficient tuning.

## D-104: Keep flight packages profile-specific

Date: 2026-07-26

Status: accepted.

Add a versioned flight-software package envelope over existing KLR contracts,
but do not replace KLR8, KLR9, or KLR10 with one universal sensor ABI. A
package declares its compatible ABI, segments, schedule, capabilities,
persistent memory, safe state, command-loss policy, and claimed targets.

The Phase 10 reference operations package delegates its inactive path to the
frozen `GlobalFlightComputer`. `SafeholdRecoveryV1` is a smaller independently
identified coast/entry/recovery implementation compiled for host and rust-mos.
It demonstrates interchangeability, not dissimilar safety redundancy.

## D-105: Use atomic load, validate, acknowledge, and commit commanding

Date: 2026-07-26

Status: accepted.

Permit ground navigation updates, bounded targets for declared plan events,
contingency selections, navigation modes, and high-level continue/hold/safe/
recovery/abort requests. Flight software stages and validates a complete load,
returns an explicit receipt, and changes active state only after a separate
commit on an exact 32 Hz release.

Never permit ground operations or procedures to command individual effectors.
Stale, corrupt, partial, incompatible, excessive-residual, and late loads fail
closed. Committed loads survive loss of ground contact; uncommitted loads
never activate.

## D-106: Separate the ground link from the simulated avionics loop

Date: 2026-07-26

Status: accepted.

Treat onboard sensor/actuator transport and spacecraft/ground communications
as distinct logical links even when one physical host/C64 connection carries
both. A simulated ground blackout may delay or drop telemetry and uplinks but
cannot remove onboard sensors, actuator authority, the committed mission
plan, compact prediction, recovery logic, or the event journal.

Procedure time and command deadlines use simulation time. Paused or externally
paced target execution therefore cannot manufacture an operational timeout.

## D-107: Defer live package handover and REU overlays

Date: 2026-07-26

Status: accepted.

Select one flight package before a session. Do not implement live PASS/BFS
engagement in Phase 11. Record a future REU feasibility study covering
executable overlays, versioned handoff state, DMA latency, atomic validation,
rollback, backup freshness, failure recovery, and C64 Ultimate interaction.

An REU cannot execute code directly and one CPU plus storage is not hardware
redundancy. The later target-engineering track must preserve that distinction.

## D-108: Accept banked stock RAM as the Phase 11 reference-operations stopgap

Date: 2026-07-26

Status: accepted.

Keep host-world plus externally paced C64 flight as the accessible Phase 11
baseline. Preserve the complete portable KsaG10rReferenceOpsV1 implementation
and place it in stock 64 KiB with an explicit headless bank layout rather than
silently requiring an REU or beginning the deferred 6502-specific rewrite.

Reserve $0200-$0427 for the mailbox/result, $0428-$053E for a bounded emergency
software stack, $053F-$0800 and $0801-$BFCC for initialized low/main code,
$C000-$E1FD for package state and the compiler static stack, and $E1FE-$FFDA
for code beneath KERNAL. Disable interrupts and map BASIC, I/O, and KERNAL out
with CPU port $34. No ROM or I/O call is permitted after entry. The custom
linker must reject any bank overflow or state-layout drift.

Accept the stopgap only with native-generated byte-exact operation vectors,
stock-memory VICE execution, code and guard preservation, measured emergency
stack use, no warp, one emulator instance, no REU, and no realtime claim. The
accepted probe covers 13 ordinary/aided, prediction, commanding, blackout,
and journal-recovery operations; it uses 16 of 279 emergency-stack bytes.

This does not close physical link/loading acceptance. Continue the physical
loader, 6502-specific rewrite, C64 Ultimate acceleration, portable C64 world,
and realtime target tracks without changing KLR10 or simulation authority.

## D-109: Freeze Phase 11 mission operations and hand off Mission Foundry

Date: 2026-07-26

Status: accepted.

Accept the Phase 11 operational shell without adding new authoritative physics.
Freeze the profile-specific flight-package envelope, mission plan, atomic
load-validate-commit boundary, separate ground communications, estimate-based
prediction, deterministic procedures/roles, action replay, session bundle,
debrief, and headless authoring SDK. Preserve KLR8, KLR9, KLR10, and every
Phase 0-10 artifact.

Accept `SafeholdRecoveryV1` as an independent limited package and the banked
stock-RAM `KsaG10rReferenceOpsV1` endpoint as an externally paced no-REU
stopgap. The flat safehold runtime ends at `$9942`; the banked reference
endpoint matches 13 native operations with preserved guards and 16/279
emergency-stack bytes used. Neither target is a realtime or physical-link
claim.

Hand the frozen contracts to Phase 12 Mission Foundry. Keep physical
loading/links, a 6502-specific rewrite, C64 Ultimate acceleration, the portable
C64 world, REU overlays, and live package handover on their declared separate
tracks.

## D-110: Consolidate products through a host application facade

Date: 2026-07-26

Status: accepted.

Add one deterministic `ProductCatalog` and `Ksa64Application` facade above the accepted Phase 0–11 implementations. Make `ksa64` the primary host executable and keep `ksa64-host` plus documented phase programs as compatibility surfaces through at least Phase 13.

Use stable domain IDs for new product discovery while preserving serialized profile variants, K-format identities, phase modules, artifacts, and hashes. Keep the current supported catalog separate from historical audits and specialist tools. Catalog JSON and application outcomes are host metadata, not canonical evidence.

Phase 12 must call Rust application services directly. It may not invoke the CLI, parse console output, duplicate physics, or bypass strict evidence parsers. Target verification is stored and non-live by default; VICE and hardware always require an explicit request. No physics, avionics, optimizer, canonical format, or C64 program changes are authorized by this decision.


## D-111: Harden the application seam before Mission Foundry

Date: 2026-07-26

Status: accepted.

Keep `Ksa64Application` as the public facade while moving project, mission, campaign, evidence, optimization, and automation adapters into focused host modules. Use static adapter dispatch for reviewed built-ins; do not create an `ApplicationService` variant for each user-authored project.

Complete `ApplicationRequest` as a nested Project/Mission/Campaign/Optimization/Evidence/Target/Audit family. Attach conservative permission, cancellation, and explicit-live-confirmation metadata while retaining typed compatibility methods and the existing target/audit safety checks.

Represent accepted built-ins, authored projects, and recent sessions with separate types and identity namespaces. Authored validation may reach Reviewed but never inherits Accepted product maturity from a reused model. Promotion into the accepted catalog requires a separately reviewed catalog and evidence decision.

Treat unknown binary evidence as opaque until an owning strict parser recognizes it. Preserve the Phase 11.5 catalog bytes, all frozen artifacts, CLI compatibility, and every C64 program.


## D-112: Require an incremental mission-session boundary before Mission Foundry

Status: accepted on 2026-07-26.

Do not let Phase 12 implement its own mission loop or present a completed replay as live operation. Add a host-only `LiveMissionSession` behind `Ksa64Application` with explicit lifecycle, exact release stepping, bounded advancement, typed truth-blind snapshots/events, operator actions through the accepted stage-validate-commit path, passive pacing controls, and deterministic KSB11 finalization.

The first accepted live adapter is the flagship KSA-G10R GNSS-loss operations scenario. Other synchronous evaluators fail closed when asked for a live session. Capability discovery is additive application metadata and does not change the frozen product-catalog bytes.

The existing completed-session command becomes a compatibility wrapper over the same session engine for this scenario. Identical ordered action transcripts must produce identical procedure, prediction, journal, checksum, and bundle evidence. Phase 12 owns graphical timelines, forms, maps, 3-D views, and wall-clock scheduling only; it cannot co-own simulation state.

## D-113: Select Unreal Engine 5.8 and stage Phase 12

Date: 2026-07-26

Status: accepted; Phase 12A implementation accepted on 2026-07-27.

Use the current Unreal Engine 5.8 Epic Games Launcher build on native Windows
11 for Phase 12, pinned to its exact installed build, supported Visual Studio
2026/MSVC/Windows SDK toolchain, and resolved E: installation/cache paths. Use
a short-path `C:\dev\KSA64` checkout and Git LFS before adding Unreal binary
content. Do not build the engine from source in Phase 12A.

Connect Unreal through a versioned in-process Rust `cdylib`/C ABI that calls
`Ksa64Application` directly and owns each `LiveMissionSession` on a dedicated
worker. Require opaque handles, fixed-width layouts, ABI and structure sizes,
explicit buffer ownership, immutable role selection, bounded nonblocking
queues, typed diagnostics, commit-qualified/hash-verified DLLs, and panic
containment. Retain an out-of-process sidecar as a separately decided fallback
if the in-process boundary cannot protect the harness and editor adequately.

Treat Unreal MCP and Python as supervised editor-development tools only. Keep
MCP loopback-only and optional; normal builds, tests, cook, packaging, and the
shipped product cannot depend on the editor, MCP, Python, Codex, or CLI text.
Rust remains sole authority for simulation, live lifecycle, role filtering,
actions, and canonical evidence. Unreal owns presentation and wall-clock
scheduling only.

Split Phase 12 into 12A toolchain/bridge feasibility, 12B live GNSS-loss
operations, 12C complete Phase 10 global engineering replay, 12D Mission
Foundry authoring/compiler parity, and 12E production visual assets and
performance. GNSS-loss proves the live application/action/evidence boundary;
it cannot alone prove ENU/ECEF/GCRF transitions, large-world continuity, entry,
or recovery. Defer all rendering, coordinates, NASA assets, and authoring UI
from 12A. Preserve every Phase 0–11.5 artifact, catalog identity, authority
lane, and K-format.

**Phase 12A implementation note.** The in-process boundary met the accepted
containment and packaging gates: each handle uses bounded 32-command and
256-event queues; roles are filtered in Rust; invalid layouts, ownership,
lifecycle, and a test panic fail without crossing the ABI; and the
commit-qualified DLL is hash-checked by the native harness and packaged Unreal
runtime. The exact existing KSB11 session is returned unchanged. The sidecar
fallback was therefore not required or implemented; it remains available only
through a future reviewed decision if later editor or runtime evidence reveals a
containment problem. Unreal MCP also remained optional development tooling.
