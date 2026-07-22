# Phase 0 results

Date: 2026-07-22

Status: Phase 0 compiler and arithmetic experiment complete; Rust/rust-mos selected.

## Scope

This result covers exact fixed-point arithmetic, the complete vertical workload, dynamics-only timing, exact arithmetic optimizations, common target-visible timing, primitive attribution, and linker-map resource evidence. It selects Rust/rust-mos for the production core while retaining Oscar64 as an independent optimization reference.

## Correctness results

| Candidate | Native host | MOS/C64 execution | Result |
|---|---:|---:|---|
| Rust, compiler-provided `u64` intermediate | Pass | `mos-sim`: 2 failures | Retained as a failing baseline |
| Rust, explicit two-word arithmetic | Pass | `mos-sim`: pass | Eligible for the next slice |
| Oscar64-compatible C++, explicit two-word arithmetic | Pass | Oscar64 integrated C64 emulator: pass | Eligible for the next slice |

The rust-mos `u64` baseline produced one scaled-division mismatch at the representative-acceleration vector and one gravity-interpolation mismatch at the 119.999 km vector when vectors ran as a sequence. Minimal probes of the individual quotient produced the correct bytes, which points to repeated optimized wide-integer execution rather than a mistaken contract constant.

An unoptimized rust-mos build was not available as a control: LLVM failed while building `core` and `compiler_builtins` with instruction-legalization errors. This is recorded as a toolchain risk rather than worked around silently.

The specialized Rust implementation avoids general 64-bit operations. It uses the same algorithmic shape as the Oscar64 candidate:

- Four 16-by-16 partial products for 32-by-32 multiplication.
- Explicit two-word shifts and rounding.
- Bitwise 64-by-32 restoring division.
- Signed saturation after magnitude arithmetic.

## Initial artifact sizes

These are correctness-runner PRGs, not isolated kernel sizes:

| Artifact | Bytes |
|---|---:|
| Rust compiler-provided `u64` C64 runner | 14,576 |
| Rust explicit two-word C64 runner | 6,580 |
| Oscar64 explicit two-word C64 runner | 4,685 |

Replacing compiler-provided wide arithmetic reduced the Rust correctness artifact by 7,996 bytes. Oscar64 is 1,895 bytes smaller than the specialized Rust runner at this checkpoint. No language decision should be made from whole-runner size alone; generated assembly, isolated kernel size, cycles, and the vertical workload still matter.

## Vertical correctness gate

Both specialized candidates reproduce all 12 fixed checkpoints and the final FNV-1a checksum `0x3a014fa6` after 2,048 steps.

| Candidate | Correctness-runner cycles | Approximate 1 MHz time | C64 artifact |
|---|---:|---:|---:|
| Specialized Rust/rust-mos | 441,745,996 | 7 min 22 s | 11,567 bytes |
| Oscar64 C++ | 457,888,329 | 7 min 38 s | 6,097 bytes |

The cycle counts include the physics loop, checkpoint comparisons, and rolling checksum. Rust was measured with `mos-sim --cycles`; Oscar64 was measured with its integrated profiling emulator. The 3.5 percent difference is encouraging but preliminary because the two emulators are not yet a calibrated common timing source.

Oscar64's profile attributes 298,078,218 cycles, about 65 percent of the total, to the restoring 64-by-32 divider. This motivated the exact division specializations measured below.

This gate also confirms the expected flight events:

- Engine cutoff occurs once at step 1,216, or T+152 seconds.
- Propellant reaches zero and mass reaches the 120-tonne dry mass.
- The coast phase continues through T+256 seconds.
- No arithmetic operation saturates in the frozen workload.

## Dynamics performance gate

The timing runners execute the same 2,048 dynamics steps but exclude rolling FNV-1a, checkpoint comparisons, rendering, and output. They expose and verify the final state so whole-program optimization cannot discard the loop.

| Candidate | Variant | Total cycles | Cycles/step | Kernel artifact | Reduction |
|---|---|---:|---:|---:|---:|
| Rust/rust-mos | General divider baseline | 344,098,644 | 168,016.92 | 9,711 bytes | - |
| Rust/rust-mos | Exact specialized path | 225,070,332 | 109,897.62 | 8,546 bytes | 34.59% |
| Oscar64 C++ | General divider baseline | 410,417,915 | 200,399.37 | 4,921 bytes | - |
| Oscar64 C++ | Exact specialized path | 235,021,443 | 114,756.56 | 4,997 bytes | 42.74% |

Rust is measured with `mos-sim --cycles`; Oscar64 uses its integrated profiling emulator. Reductions within each toolchain are controlled comparisons. Their absolute totals use different accounting and are retained for optimization history; the common-clock comparison below supersedes them for cross-language conclusions.

## Common C64 timing gate

Both optimized C64 PRGs were measured in the same official PAL VICE 3.10 `x64sc` environment using a target-visible 32-bit CIA1 counter. Timer A counts processor clocks and Timer B counts Timer A underflows. Each executable blanks the VIC display, disables sprites and CIA interrupts, aligns both boundary measurements to the start of a video frame, subtracts its empty-boundary cost, and publishes its result and final state at `$c000`.

| Candidate | Net CIA cycles | Cycles/step | Boundary cost | Timed PRG | Three-run result |
|---|---:|---:|---:|---:|---|
| Rust/rust-mos | 223,772,332 | 109,263.83 | 4 | 9,026 bytes | Identical |
| Oscar64 C++ | 235,627,088 | 115,052.29 | 18 | 5,865 bytes | Identical |

Rust uses 11,854,756 fewer cycles, or 5.03 percent fewer than Oscar64. Expressed as throughput, the Rust kernel is 5.30 percent faster. This is a real but modest advantage, well inside the experiment's 25 percent threshold for retaining the incumbent language when the remaining gates pass.

Every timed run produced the same frozen final state:

- Altitude Q12: `1,555,457`.
- Velocity Q24: `31,437,297`.
- Acceleration Q28: `-2,346,189`.
- Mass Q12: `491,520`.
- Propellant Q12: `0`.
- Cutoff events: `1`.

At the frozen 0.125-second timestep, a PAL C64 provides roughly 123,000 processor cycles per step. Both common-clock kernels fit that raw 8 Hz budget before display, sensor, guidance, and scheduling costs are added. Rust leaves about 13,700 cycles per step and Oscar64 about 7,900; this makes optimization of the remaining acceleration division materially useful even though the physics kernel is already feasible.

## Exact specializations

The frozen run performs:

- 17,596 scaled multiplications in either variant.
- 8,188 general 64-by-32 divisions in the baseline.
- 2,048 general acceleration divisions after optimization.
- 4,092 specialized interpolation-fraction divisions after optimization.
- 2,048 rounded nonnegative halvings replacing general drag division.

Two algebraically exact substitutions produce the reduction:

1. Nonnegative drag division by two becomes `(value >> 1) + (value & 1)`, preserving round-half-away-from-zero.
2. Environment knot widths are integral Q12 kilometres, so `(delta_q12 << 16) / span_q12` becomes `(delta_q12 << 4) / span_km`. This uses a 32-by-16 restoring divider rather than the general 64-by-32 path.

Both optimized implementations still match every checkpoint and the final checksum `0x3a014fa6` natively and on their C64 targets.

### Oscar64 profile evidence

Oscar64 attributes 298,091,198 baseline cycles to the 64-by-32 restoring divider. After specialization:

- The general divider falls to 75,148,576 cycles.
- The fast interpolation fraction path costs 58,026,958 cycles.
- Scaled multiplication remains essentially unchanged at about 36.3 million exclusive cycles.
- The generated optimized PRG grows by 76 bytes, from 4,921 to 4,997 bytes.

The optimized Oscar64 map reports 4,838 bytes of code and data in the main region, no general heap use, 25 bytes for final state in the runner, and 116 bytes of environment tables. Rust's linked `mos-sim` kernel shrinks by 1,165 bytes because LTO removes much of the displaced general-division call path.

## Primitive attribution gate

The isolated runners execute 512 calls per primitive. Operands come from volatile C64 memory to prevent constant folding, while the Q-format shift remains the same compile-time constant used at the flight-dynamics call site. Each loop accumulates and validates its outputs on target. The reported total subtracts the same empty CIA boundary measurement used by the full kernel.

| Primitive | Rust cycles | Rust/call | Oscar64 cycles | Oscar64/call | Oscar64 advantage |
|---|---:|---:|---:|---:|---:|
| Scaled 32-by-32 multiply, shift 28 | 2,121,015 | 4,142.61 | 2,115,619 | 4,132.07 | 0.25% |
| General scaled 64-by-32 divide, shift 28 | 20,699,187 | 40,428.10 | 19,313,692 | 37,722.05 | 6.69% |
| Specialized Q16 fraction divide | 10,513,986 | 20,535.13 | 7,017,501 | 13,706.06 | 33.26% |

All values were identical across three PAL VICE runs. Both candidates produced the same accumulators for all three operations.

The general acceleration divide remains the highest-cost individual call: one call consumes about 40,428 Rust cycles or 37,722 Oscar64 cycles. Two specialized interpolation divisions occur per step, so their aggregate cost remains comparable in Rust and lower in Oscar64. The isolated totals include volatile loads, loop control, and accumulation and therefore are attribution evidence rather than numbers to add mechanically into the full-kernel total.

Oscar64's stronger isolated division code does not overturn the representative result. Rust's full kernel remains 5.03 percent lower in cycles, indicating that its surrounding state-update and call-site optimization offsets the primitive disadvantage. The next focused optimization target is the remaining acceleration division, not a language rewrite.

## Resource gate

The timed full-kernel builds were relinked with maps and parsed by `resources.ps1`.

| Candidate | Timed PRG | Code | Read-only data | BSS | Zero page | Static stack evidence |
|---|---:|---:|---:|---:|---:|---:|
| Rust/rust-mos | 9,026 | 8,784 | 228 | 0 | 17 | 66-byte `.noinit` static stack |
| Oscar64 C++ | 5,865 | 5,108 | 628 | 0 | 0 | 122-byte map envelope through `$a000` |

Oscar64's PRG is 3,161 bytes smaller. Rust spends 17 bytes of zero page but reserves 56 fewer bytes in the compiler-reported static-stack area. Neither runner allocates BSS or a heap object, and both leave ample conventional RAM for the Phase 1 vertical laboratory.

Generated-code structure explains why isolated primitives are not the whole result. Rust LTO places 6,345 bytes in the inlined `main` path and 2,249 bytes in its remaining interpolation routine. Oscar64 emits smaller, separately named routines, including a 1,047-byte vertical step, a 456-byte restoring divider, and a 378-byte scaled-division wrapper. Oscar64 optimizes for compact modular code; rust-mos spends code size on contextual specialization that wins the complete workload.

## Phase 0 language decision

Rust/rust-mos is selected for the portable KSA64 core.

It passes every correctness gate, is 5.03 percent lower in common-clock cycles on the representative workload, fits the raw 8 Hz physics budget, has acceptable memory use, and reuses an already-proven project workflow. This satisfies every condition in the experiment rubric and provides no material reason to abandon the incumbent.

Oscar64 remains valuable as a comparison oracle and optimization reference. Its compact output and division performance suggest that a future measured Rust hotspot may justify a small assembly or foreign-function helper. They do not justify maintaining the entire simulation in C++.

The fixed-point core will use the explicit two-word widening algorithms. Compiler-provided Rust `u64` remains a failing regression baseline until a future pinned toolchain passes the frozen contract.

## Reproduce

From the project root in PowerShell:

    .\phase0\check.ps1
    .\phase0\benchmark.ps1
    .\phase0\timing.ps1
    .\phase0\primitive_timing.ps1
    .\phase0\resources.ps1

The check verifies generated artifacts and all host/C64 builds. The benchmark captures tool-specific dynamics profiles. The timing script measures the representative full kernel, the primitive script measures three isolated arithmetic paths, and the resource script rebuilds both timed PRGs with linker maps. Both VICE timing scripts require identical results across three sequential PAL runs by default.

The failing Rust baseline is considered reproduced only when it returns exactly two failures. A changed result forces a fresh investigation rather than turning a known failure into an ignored test.

## Next gate

The compiler experiment is complete. Continue Phase 0's numeric foundation before creating the production simulator tree:

1. Perform explicit range analysis for the Phase 1 physical quantities.
2. Select product fixed-point formats rather than inheriting benchmark formats automatically.
3. Decide the production overflow policy and initial integrator/timestep.
4. Define deterministic scenario and telemetry formats.
5. Add analytic integration cases required by the Phase 0 roadmap exit criteria.

Real-hardware timing remains a valuable confirmation when a machine is available, but it is not blocking the toolchain decision because both PRGs use the same target-visible CIA method and stable cycle-accurate environment.
