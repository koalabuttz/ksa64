# Phase 1 production timing

Date: 2026-07-22

Status: measurement gate complete; exact execution is stable, but the current production core does not fit the raw PAL 8 Hz budget.

## Measurement boundary

The dedicated timing PRG parses the golden scenario before timing, disables VIC display DMA, sprites, and CIA interrupts, and measures two production paths with the same target-visible cascaded CIA1 timer used in Phase 0:

1. Checked dynamics through `run_vertical_dynamics`, including environment sampling, force evaluation, numeric-status checks, immutable successor construction, cutoff detection, and loop control.
2. The same checked executor through `run_vertical_mission`, with the rolling exact-state FNV-1a checksum enabled.

The parser, timer setup, final-state validation, and result publication are outside the measured regions. An empty start/stop boundary measurement is subtracted from both totals. Both paths use the same const-generic executor, so the difference isolates rolling checksum work without duplicating the dynamics implementation.

## PAL common-clock result

The pinned rust-mos build ran under the pinned cycle-accurate PAL VICE 3.10 `x64sc`. All three sequential runs were identical and produced the accepted final truth and checksum.

| Path | Net cycles | Cycles/step | Maximum rate | Margin at 8 Hz |
|---|---:|---:|---:|---:|
| Checked dynamics | 329,532,711 | 160,904.64 | 6.12 Hz | -37,748.64 |
| Dynamics plus rolling checksum | 430,920,997 | 210,410.64 | 4.68 Hz | -87,254.64 |

At 985,248 processor clocks per second, the 0.125-second timestep permits 123,156 cycles per step. Checked dynamics exceeds that raw budget by 30.65 percent. Per-successor checksum validation adds 101,388,286 cycles, or 49,506.00 cycles per step, increasing checked-dynamics cost by 30.77 percent.

The diagnostic timing PRG is 20,952 bytes. Its size includes both specialized executor instantiations, the scenario parser, golden-result checks, timer support, and result publisher; it is not the size of a deployable single-path simulator.

The machine-readable evidence is in `production-timing-v1.json`.

## Finding

The gate passes as a reproducible measurement and correctness check, but fails the provisional raw 8 Hz performance target. This does not violate the project goal—real-time execution is not required—but it leaves no budget for telemetry or presentation at that cadence.

The highest-value next optimization is production environment interpolation. Each successful dynamics step samples density and gravity, and the current production `interpolate_clamped` performs two general 64-by-32 scaled divisions. Phase 0 measured that Rust primitive at 40,428.10 cycles per call and already proved an algebraically exact 32-by-16 interpolation-fraction path at 20,535.13 cycles per call for these integral Q12 kilometre knot widths. Applying that proven specialization twice per step has a measured savings potential of about 39,786 cycles per step—enough to put checked dynamics near the 8 Hz boundary before fresh common-clock confirmation.

The rolling checksum is a separate policy cost, not physics. It should remain available for deterministic validation, but telemetry work should not assume that hashing every truth word after every step is free or required during an interactive run.

## Reproduce

From the project root in PowerShell:

    .\phase1\timing.ps1

The script verifies the pinned VICE executable, builds the timing PRG with the pinned rust-mos image, requires stable results across three runs by default, and prints the 8 Hz margins and artifact size.
