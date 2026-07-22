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

## Open decisions

The following remain deliberately unresolved:

- License for KSA64.
- Final simulator fixed-point format for each physical quantity.
- Overflow policy: saturation, checked failure, or proven range.
- Initial integrator and timestep.
- Initial Earth and atmosphere models.
- Telemetry file format.
- Target C64 and REU configurations.
- Minimum acceptable simulation rate.
- Whether the host comparison path shares model source through numeric generics or uses a deliberately independent compact implementation.

