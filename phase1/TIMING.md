# Phase 1 production timing

Date: 2026-07-22

Status: raw physics-budget and telemetry-attribution gates passed. Exact checked dynamics runs at 8.57 Hz; checksum plus canonical telemetry runs at 5.72 Hz, with telemetry itself adding 7,475.28 cycles per physics step over checksum mode.

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

## Canonical telemetry cost

A separate three-path diagnostic PRG measures raw dynamics, per-successor checksumming, and checksum plus canonical telemetry in the same binary. The telemetry path emits one 32-byte header and 257 40-byte frames through a volatile discard sink. The sink overwrites fixed buffers so every byte must be materialized, but it performs no display, disk, REU, or serial transport. Final truth, frame count, byte count, terminal events, state checksum, and final frame CRC are checked after timing.

All three PAL VICE runs were identical:

| Path | Total cycles | Cycles/physics step | Maximum rate | 8 Hz margin |
|---|---:|---:|---:|---:|
| Checked dynamics | 235,477,621 | 114,979.31 | 8.57 Hz | +8,176.69 |
| Dynamics + rolling checksum | 337,259,130 | 164,677.31 | 5.98 Hz | -41,521.31 |
| Checksum + canonical telemetry | 352,568,502 | 172,152.59 | 5.72 Hz | -48,996.59 |

Canonical scheduling, record serialization, record CRCs, and the volatile discard copies add 15,309,372 cycles per mission: 7,475.28 cycles per physics step, 59,569.54 cycles per emitted frame, or 4.54 percent over checksum mode. The full path does not fit 8 Hz, but telemetry is not the dominant reason; per-successor exact-state hashing costs roughly 6.6 times the telemetry increment.

The diagnostic telemetry PRG is 29,444 bytes and the accepted machine-readable evidence is `telemetry-timing-v1.json`. Small raw/checksum differences from `production-timing-v3.json` are whole-program code-layout effects from adding the third executor instantiation, so attribution uses differences between paths in this same binary.

This closes the timing question without forcing one runtime policy. Recorded validation may preserve every successor checksum and run at 5.72 Hz. An interactive mode can keep the 8.57 Hz dynamics path and use a cheaper or less frequent validation policy later, while canonical telemetry format and scheduling remain unchanged.
## Finding

The raw Phase 1 physics loop fits its provisional cadence without weakening the numeric contract or narrowing accepted scenarios. All 34 native tests, rust-mos exact execution, the C64 correctness build, and both three-run common-clock timing gates pass with the unchanged `0x72bf6e0e` mission checksum.

Arithmetic optimization remains paused: the new measurement shows serialization is a modest increment, while validation cadence is the larger policy decision. The next Phase 1 slice is a host capture and strict inspection adapter using the accepted sink and binary format, followed by C64 presentation.

## Reproduce

From the project root in PowerShell:

    .\phase1\check.ps1
    .\phase1\timing.ps1
    .\phase1\telemetry_timing.ps1

The first script verifies generated inputs and exact execution across native Rust, rust-mos, and the C64 build. The second preserves the accepted raw/checksum measurement. The third builds the telemetry timing PRG, requires three stable runs by default, and prints the checksum and serialization deltas, 8 Hz margins, frame/byte counts, and artifact size.