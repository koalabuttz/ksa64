# Phase 0 compiler and arithmetic experiment

## Research question

Can rust-mos support KSA64's representative fixed-point flight-dynamics workload within acceptable performance and memory costs, or does Oscar64 C++ provide a material advantage large enough to justify changing languages?

Result: complete. Rust/rust-mos is selected for the portable core. See [the Phase 0 results](../phase0/RESULTS.md) and decisions D-004 and D-015 in [the decision record](decisions.md).

The experiment is a decision tool, not the beginning of the simulator.

## Candidates

### Candidate A: Rust

- Native stable Rust test executable where possible.
- rust-mos C64 executable.
- No standard library on the C64 target.
- No heap allocation.
- Strong numeric wrappers and flight-state types.

### Candidate B: restrained C++

- Native Clang or GCC test executable.
- Oscar64 C64 executable.
- Static allocation.
- Plain structs, namespaces, inline functions, small templates, and numeric wrappers.
- No exceptions, RTTI, virtual dispatch, heavy containers, or stream I/O.

Plain C is the fallback baseline if both candidates reveal blockers. Prog8 and Millfork are outside the first comparison.

## Fair-comparison rules

Both candidates must use:

- Identical integer widths and signedness.
- Identical fixed-point scales.
- Identical constants and lookup-table bytes.
- Identical overflow and rounding rules.
- Identical operation ordering.
- Identical timestep and number of iterations.
- Equivalent bounds assumptions.
- Equivalent outputs and checksums.

The benchmark must not improve one result by silently changing the physical model or numeric contract.

## Workloads

### Kernel 1: signed scaled multiplication

Exercise:

- Positive and negative operands.
- Values near practical range limits.
- Widening intermediate behavior.
- Truncation and round-to-nearest variants.
- A specialized implementation against the compiler's general 64-bit path.

Measure correctness, cycles, code size, and temporary storage.

### Kernel 2: scaled division and reciprocal

Compare:

- General signed division.
- Normalized reciprocal approximation.
- Reciprocal lookup with refinement.
- Narrower range-specific division where physically valid.

Division is expected to be a major cost and should be measured independently.

### Kernel 3: table interpolation

Use an altitude-indexed atmosphere-like table with:

- Index calculation.
- Clamping at valid limits.
- Linear interpolation.
- Checked indexing.
- A proven-range or raw-access variant when safe.

The experiment should reveal whether bounds checks are eliminated and whether safe wrappers cost cycles.

### Kernel 4: vertical dynamics step

Use state equivalent to:

- Altitude.
- Velocity.
- Vehicle mass.
- Propellant mass.
- Mission elapsed time.

Evaluate:

- Altitude-dependent gravity.
- Tabulated atmospheric density.
- Quadratic drag.
- Constant thrust while propellant remains.
- Mass flow.
- Semi-implicit Euler update.

This is a benchmark model only. Its constants and formulas will not become the flight simulator merely because they were convenient for the experiment.

### Kernel 5: representative integration loop

Execute enough vertical-dynamics steps to make timer resolution and startup costs negligible. Produce:

- Final-state fields.
- A rolling checksum over intermediate states.
- Event counts such as burnout and ground contact.
- Cycle or timer measurements.

Rendering and file I/O remain outside the timed section.

## Variants to test

### Rust-specific

- Newtypes versus raw integers.
- Trait operators versus direct functions.
- Safe table indexing versus a proven-range access path.
- Compiler-provided 64-bit arithmetic versus specialized scaled multiply.
- Debug assertions removed versus retained outside the hot path.

### C++-specific

- Wrapper structs versus raw integers.
- Operator overloads versus direct functions.
- Template scale parameters versus fixed functions.
- Compiler-provided wide arithmetic versus specialized scaled multiply.

Generated code should be inspected to confirm whether abstractions disappear.

## Correctness gates

A candidate is disqualified until it:

- Passes the same arithmetic vectors.
- Matches the declared rounding and overflow contract.
- Produces the expected lookup interpolation.
- Matches its native exact-arithmetic build for the integration workload.
- Produces a stable result across repeated emulator runs.

Performance comparisons are meaningless before these gates pass.

## Measurements

Record for each target and variant:

| Measurement | Purpose |
|---|---|
| Cycles or timer ticks per dynamics step | Determines feasible simulation throughput |
| Total timed-loop duration | Reduces sensitivity to measurement noise |
| Program and code size | Reveals compiler and runtime cost |
| Static RAM use | Protects room for tables, UI, and telemetry |
| Zero-page use | Exposes pressure on the fastest scarce storage |
| Stack use | Detects unsafe or surprising call costs |
| Temporary storage | Reveals wide-arithmetic overhead |
| Native-to-C64 state agreement | Confirms portable exact behavior |
| Error against high precision | Quantifies fixed-point and integration error |
| Source complexity | Captures long-term maintenance cost |
| Toolchain reproducibility | Captures setup and build risk |

Store compiler versions, flags, emulator version, target configuration, and hardware details with results.

## Timing method

Use at least two forms of timing:

1. Emulator profiling or cycle counts for detailed comparison.
2. CIA timer measurement inside the C64 executable for a target-visible result.

Run a smaller confirmation on real hardware when available. Real-hardware results validate the procedure; the emulator remains the convenient development instrument.

## High-precision comparison

A small host-only calculation may use floating point to quantify numeric error. It should implement only the experiment equations and emit expected checkpoints.

This is not a second product simulator. Its purposes are:

- Detect scaling mistakes.
- Estimate accumulated fixed-point error.
- Compare integrator and timestep combinations.
- Produce human-readable expected results.

Because it is not independent if it mechanically shares every equation, analytic cases remain necessary.

## Decision rubric

Rust remains the project language when:

- It passes all correctness gates.
- Its resource use leaves credible room for later simulation, UI, and telemetry.
- Its performance is within roughly 25 percent of Oscar64 on the representative full-step workload, or the absolute rate is already comfortably sufficient.
- Native and C64 workflows remain reproducible.

Oscar64 C++ becomes the leading choice when:

- It passes the same correctness gates.
- It produces a sustained, material advantage on the full workload, not merely one isolated primitive.
- That advantage changes project feasibility or fidelity.
- Its host/C64 source discipline and toolchain are maintainable.

A small difference should favor Rust because existing rust-mos experience lowers project risk. A large kernel-specific difference may justify an assembly or foreign-function helper rather than a language rewrite.

If neither candidate is viable, add a plain-C baseline and revisit numeric design before testing more languages.

## Required deliverable

The experiment concludes with one short report containing:

- Toolchain versions and reproduction steps.
- Source and generated tables used.
- Raw results.
- Correctness and error results.
- Generated-code observations.
- Selected language.
- Selected arithmetic approach.
- Known risks and the next experiment, if any.

No production simulator source tree should be committed until this decision is recorded.

