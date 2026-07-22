# Phase 0 results

Date: 2026-07-22

Status: correctness and first dynamics-performance gates complete; common CIA timing remains.

## Scope

This result covers exact fixed-point arithmetic, the complete vertical workload, dynamics-only timing, and the first exact arithmetic optimizations. It does not yet select the final KSA64 language because both candidates still require a common CIA timing method.

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

Rust is measured with `mos-sim --cycles`; Oscar64 uses its integrated profiling emulator. Reductions within each toolchain are controlled comparisons. The absolute Rust/Oscar totals are still preliminary because they do not yet come from one common C64 timer and emulator.

At the frozen 0.125-second timestep, a nominal 1 MHz machine provides about 125,000 cycles per step. Both optimized physics kernels are below that raw budget, which makes an 8 Hz Phase 0 model plausible before display, sensor, guidance, and scheduling costs are added.

### Exact specializations

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
## Reproduce

From the project root in PowerShell:

    .\phase0\check.ps1
    .\phase0\benchmark.ps1

The check verifies generated artifacts, runs native Rust tests, executes both Rust variants with the simulator bundled in the pinned rust-mos image, builds the C64 Rust artifacts, builds and runs the native C++ candidate, executes the Oscar64 C64 candidate, and reports artifact sizes. The benchmark runs the baseline and optimized dynamics-only kernels, captures both emulator profiles, and prints cycle, size, and reduction summaries.

The failing Rust baseline is considered reproduced only when it returns exactly two failures. A changed result forces a fresh investigation rather than turning a known failure into an ignored test.

## Next gate

Put both candidates on one target-visible timing method:

1. Add identical CIA timer boundaries around the 2,048-step kernel.
2. Execute both PRGs in one cycle-accurate C64 environment.
3. Measure primitive multiply, general divide, and fast interpolation-divide runners.
4. Confirm that the remaining acceleration division is the next material bottleneck.
5. Repeat a smaller confirmation on real hardware when available.

The language decision remains open. The reported optimized totals differ by less than five percent, well inside the range where the common-timer requirement matters more than the apparent lead.
