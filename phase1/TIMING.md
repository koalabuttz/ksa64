# Phase 1 production timing

Date: 2026-07-22

Status: raw physics-budget gate passed. Exact checked dynamics runs at 8.57 Hz on the PAL common clock with 8,174.41 cycles per step of headroom.

## Measurement boundary

The dedicated timing PRG parses the golden scenario before timing, disables VIC display DMA, sprites, and CIA interrupts, and measures two production paths with the same target-visible cascaded CIA1 timer used in Phase 0:

1. Checked dynamics through `run_vertical_dynamics`, including environment sampling, force evaluation, numeric-status checks, immutable successor construction, cutoff detection, and loop control.
2. The same checked executor through `run_vertical_mission`, with the rolling exact-state FNV-1a checksum enabled.

The parser, timer setup, final-state validation, and result publication are outside the measured regions. An empty start/stop boundary measurement is subtracted from both totals. Both paths use the same const-generic executor, so their difference isolates rolling checksum work without duplicating dynamics.

Every recorded version uses the pinned rust-mos image and cycle-accurate PAL VICE 3.10 `x64sc`. Three sequential runs were identical at each gate and produced the accepted final truth and checksum.

## Optimization progression

At 985,248 processor clocks per second, the 0.125-second timestep permits 123,156 cycles per step.

| Version | Checked dynamics/step | Maximum rate | 8 Hz margin | Change |
|---|---:|---:|---:|---:|
| v1: general production arithmetic | 160,904.64 | 6.12 Hz | -37,748.64 | baseline |
| v2: integral-Q12 environment interpolation | 127,932.69 | 7.70 Hz | -4,776.69 | -20.49% |
| v3: reduced acceleration division | 114,981.59 | 8.57 Hz | +8,174.41 | -10.12% from v2 |

Together, the two exact specializations remove 45,923.05 cycles per step, or 28.54 percent of the v1 checked-dynamics cost. The final margin is 6.64 percent of the raw 8 Hz budget.

Machine-readable evidence is retained separately:

- `production-timing-v1.json`: general production arithmetic.
- `production-timing-v2.json`: specialized environment interpolation.
- `production-timing-v3.json`: specialized acceleration division.

## Exact fast paths

The environment verifies that every altitude-knot span is a positive integral Q20.12 kilometre count. Its interpolation fraction computes the same rounded Q0.16 result with a checked 32-by-16 divider instead of the general 64-by-32 divider. The generic interpolation primitive remains available for other tables.

Acceleration division attempts a narrower path only when all of these facts hold:

- force magnitude fits the accepted 21-bit Phase 1 envelope;
- mass raw units contain the declared factor of 128;
- the reduced mass denominator fits 16 bits.

It removes the same factor from the denominator and Q28 numerator shift, processes only the provably occupied numerator bits, and preserves nearest rounding with exact halves away from zero. Any input outside that envelope automatically uses the original general divider. Tests compare both routes across signed values, zero, out-of-envelope forces, non-aligned masses, and large valid masses.

The v3 diagnostic timing PRG is 22,973 bytes. This is 913 bytes larger than v2 because it carries both the exact fast path and general fallback, plus both executor instantiations, scenario parsing, result assertions, and timing support. It is not the size of a deployable single-path simulator.

## Validation-policy cost

The v3 mission path with per-successor rolling FNV-1a costs 164,489.59 cycles per step, while checked dynamics alone costs 114,981.59. Full-state hashing therefore adds 49,508.00 cycles per step and does not fit 8 Hz.

This does not invalidate the physics-budget result. The checksum is deterministic validation policy rather than dynamics, and KSA64 does not require all modes to run in real time. The telemetry gate must preserve canonical checksums while keeping validation cadence and interactive scheduling explicit.

## Finding

The raw Phase 1 physics loop now fits its provisional cadence without weakening the numeric contract or narrowing accepted scenarios. All 23 native tests, rust-mos exact execution, the C64 correctness build, and three common-clock timing runs pass with the unchanged `0x72bf6e0e` mission checksum.

Arithmetic optimization now stops unless a later measured subsystem needs more headroom. The next Phase 1 slice is canonical binary telemetry serialization: first exact 32-byte headers and 40-byte frames against the independent golden fixture, then scheduled stream emission and its separate timing cost.

## Reproduce

From the project root in PowerShell:

    .\phase1\check.ps1
    .\phase1\timing.ps1

The first script verifies generated inputs and exact execution across native Rust, rust-mos, and the C64 build. The second verifies the pinned VICE executable, builds the timing PRG with the pinned rust-mos image, requires stable results across three runs by default, and prints the 8 Hz margins and artifact size.