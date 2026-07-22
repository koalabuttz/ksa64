# Phase 1 production timing

Date: 2026-07-22

Status: interpolation optimization verified; exact execution is stable and checked dynamics is 20.49 percent faster, but remains 3.88 percent over the provisional raw PAL 8 Hz budget.

## Measurement boundary

The dedicated timing PRG parses the golden scenario before timing, disables VIC display DMA, sprites, and CIA interrupts, and measures two production paths with the same target-visible cascaded CIA1 timer used in Phase 0:

1. Checked dynamics through `run_vertical_dynamics`, including environment sampling, force evaluation, numeric-status checks, immutable successor construction, cutoff detection, and loop control.
2. The same checked executor through `run_vertical_mission`, with the rolling exact-state FNV-1a checksum enabled.

The parser, timer setup, final-state validation, and result publication are outside the measured regions. An empty start/stop boundary measurement is subtracted from both totals. Both paths use the same const-generic executor, so the difference isolates rolling checksum work without duplicating the dynamics implementation.

## Baseline common-clock result

The pinned rust-mos build ran under the pinned cycle-accurate PAL VICE 3.10 `x64sc`. All three sequential baseline runs were identical and produced the accepted final truth and checksum.

| Baseline path | Net cycles | Cycles/step | Maximum rate | Margin at 8 Hz |
|---|---:|---:|---:|---:|
| Checked dynamics | 329,532,711 | 160,904.64 | 6.12 Hz | -37,748.64 |
| Dynamics plus rolling checksum | 430,920,997 | 210,410.64 | 4.68 Hz | -87,254.64 |

The baseline machine-readable evidence is in `production-timing-v1.json`.

## Integral-Q12 interpolation result

The production environment now verifies that every altitude-knot span is a positive integral Q20.12 kilometre count. Its interpolation fraction computes the same rounded Q0.16 result with a checked 32-by-16 divider instead of the general 64-by-32 divider. The generic interpolation primitive remains available for tables that do not satisfy this contract.

All 22 native tests, the rust-mos exact self-test, the C64 build, and three fresh PAL timing runs preserve the accepted final truth and checksum.

| Optimized path | Net cycles | Cycles/step | Maximum rate | Margin at 8 Hz |
|---|---:|---:|---:|---:|
| Checked dynamics | 262,006,151 | 127,932.69 | 7.70 Hz | -4,776.69 |
| Dynamics plus rolling checksum | 363,838,853 | 177,655.69 | 5.55 Hz | -54,499.69 |

At 985,248 processor clocks per second, the 0.125-second timestep permits 123,156 cycles per step. The specialization saves 32,971.95 checked-dynamics cycles per step, a 20.49 percent reduction, and shrinks the budget miss from 30.65 percent to 3.88 percent. The measured saving is smaller than the isolated primitive estimate because production retains contract checks and whole-program code placement differs.

Per-successor checksum validation now measures 49,723.00 cycles per step. The small 217-cycle increase from the baseline checksum delta is a whole-program code-generation effect; the checksum algorithm and its exact result did not change.

The optimized diagnostic timing PRG is 22,060 bytes, 1,108 bytes larger than the baseline because it contains the specialized divider alongside the general acceleration divider and both executor instantiations. It is not the size of a deployable single-path simulator.

The optimized machine-readable evidence is in `production-timing-v2.json`.

## Finding

The optimization succeeds: it preserves the numeric contract and recovers most of the missing real-time budget. Checked dynamics is now about 7.70 Hz, only 4,776.69 cycles per step short of the provisional 8 Hz target.

The next measured target is the remaining general 64-by-32 acceleration division. It runs once per physics step and Phase 0 measured it as the largest individual arithmetic call. A specialization must remain exact, retain the general fallback for arbitrary validated scenarios, and earn acceptance through unchanged golden results plus another common-clock measurement.

The rolling checksum remains a separate validation-policy cost rather than physics. It should remain available for deterministic regression runs, but interactive scheduling should not assume per-successor full-state hashing is free.

## Reproduce

From the project root in PowerShell:

    .\phase1\check.ps1
    .\phase1\timing.ps1

The first script verifies generated inputs and exact execution across native Rust, rust-mos, and the C64 build. The second verifies the pinned VICE executable, builds the timing PRG with the pinned rust-mos image, requires stable results across three runs by default, and prints the 8 Hz margins and artifact size.