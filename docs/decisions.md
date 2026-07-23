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
