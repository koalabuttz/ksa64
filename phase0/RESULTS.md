# Phase 0 arithmetic-slice results

Date: 2026-07-22

Status: arithmetic correctness gate complete; timing and vertical-flight workload remain.

## Scope

This result covers signed scaled multiplication, scaled division, atmosphere-table interpolation, and generated-vector handling. It does not select the final KSA64 language and does not benchmark the vertical dynamics loop.

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

Oscar64's profile attributes 298,078,218 cycles, about 65 percent of the total, to the restoring 64-by-32 divider. The next performance slice should therefore focus on division and workload-specific reciprocal strategies before broader source-level optimization.

This gate also confirms the expected flight events:

- Engine cutoff occurs once at step 1,216, or T+152 seconds.
- Propellant reaches zero and mass reaches the 120-tonne dry mass.
- The coast phase continues through T+256 seconds.
- No arithmetic operation saturates in the frozen workload.

## Reproduce

From the project root in PowerShell:

    .\phase0\check.ps1

The check verifies generated artifacts, runs native Rust tests, executes both Rust variants with the simulator bundled in the pinned rust-mos image, builds the C64 Rust artifacts, builds and runs the native C++ candidate, executes the Oscar64 C64 candidate, and reports artifact sizes.

The failing Rust baseline is considered reproduced only when it returns exactly two failures. A changed result forces a fresh investigation rather than turning a known failure into an ignored test.

## Next gate

Turn the correctness workload into a controlled performance experiment:

1. Separate dynamics-only timing from checkpoints and checksum work.
2. Isolate multiplication, division, interpolation, and full-step cycle costs.
3. Inspect generated assembly and map-file contributions.
4. Compare general restoring division with range-specific reciprocal approaches.
5. Preserve exact checkpoints and checksum for every eligible optimization.

The language decision remains open until the sustained workload is measured after obvious arithmetic bottlenecks are addressed.
