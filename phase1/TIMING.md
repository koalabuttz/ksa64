# Phase 1 production timing

Date: 2026-07-22

Status: final Phase 1 linked-layout gates passed. Exact checked dynamics runs at 8.34 Hz; checksum plus canonical telemetry runs at 5.62 Hz, with telemetry itself adding 7,504.00 cycles per physics step over checksum mode.

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
| v3: reduced acceleration division checkpoint | 114,981.59 | 8.57 Hz | +8,174.41 | -10.12% from v2 |
| v4: final Phase 1 linked layout | 118,111.48 | 8.34 Hz | +5,044.52 | +2.72% from v3 |

The final binary remains 42,793.16 cycles per step, or 26.60 percent, faster than v1. Its margin is 4.10 percent of the raw 8 Hz budget. The v3-to-v4 change is whole-program code layout from the final adapters and acceptance surfaces, not a dynamics-model change; the exact final state and checksum are unchanged.

Machine-readable evidence is retained separately:

- `production-timing-v1.json`: general production arithmetic.
- `production-timing-v2.json`: specialized environment interpolation.
- production-timing-v3.json: specialized acceleration division checkpoint.
- production-timing-v4.json: final Phase 1 linked layout.

## Exact fast paths

The environment verifies that every altitude-knot span is a positive integral Q20.12 kilometre count. Its interpolation fraction computes the same rounded Q0.16 result with a checked 32-by-16 divider instead of the general 64-by-32 divider. The generic interpolation primitive remains available for other tables.

Acceleration division attempts a narrower path only when all of these facts hold:

- force magnitude fits the accepted 21-bit Phase 1 envelope;
- mass raw units contain the declared factor of 128;
- the reduced mass denominator fits 16 bits.

It removes the same factor from the denominator and Q28 numerator shift, processes only the provably occupied numerator bits, and preserves nearest rounding with exact halves away from zero. Any input outside that envelope automatically uses the original general divider. Tests compare both routes across signed values, zero, out-of-envelope forces, non-aligned masses, and large valid masses.

The v3 diagnostic timing PRG is 22,973 bytes. This is 913 bytes larger than v2 because it carries both the exact fast path and general fallback, plus both executor instantiations, scenario parsing, result assertions, and timing support. It is not the size of a deployable single-path simulator.

## Validation-policy cost

The final v4 mission path with per-successor rolling FNV-1a costs 167,610.48 cycles per step, while checked dynamics alone costs 118,111.48. Full-state hashing therefore adds 49,499.00 cycles per step and does not fit 8 Hz.

This does not invalidate the physics-budget result. The checksum is deterministic validation policy rather than dynamics, and KSA64 does not require all modes to run in real time. The telemetry gate must preserve canonical checksums while keeping validation cadence and interactive scheduling explicit.

## Canonical telemetry cost

A separate three-path diagnostic PRG measures raw dynamics, per-successor checksumming, and checksum plus canonical telemetry in the same binary. The telemetry path emits one 32-byte header and 257 40-byte frames through a volatile discard sink. The sink overwrites fixed buffers so every byte must be materialized, but it performs no display, disk, REU, or serial transport. Final truth, frame count, byte count, terminal events, state checksum, and final frame CRC are checked after timing.

All three PAL VICE runs were identical:

| Path | Total cycles | Cycles/physics step | Maximum rate | 8 Hz margin |
|---|---:|---:|---:|---:|
| Checked dynamics | 241,892,732 | 118,111.69 | 8.34 Hz | +5,044.31 |
| Dynamics + rolling checksum | 343,661,953 | 167,803.69 | 5.87 Hz | -44,647.69 |
| Checksum + canonical telemetry | 359,030,136 | 175,307.68 | 5.62 Hz | -52,151.68 |

Canonical scheduling, record serialization, record CRCs, and the volatile discard copies add 15,368,183 cycles per mission: 7,504.00 cycles per physics step, 59,798.38 cycles per emitted frame, or 4.47 percent over checksum mode. The full path does not fit 8 Hz, but telemetry is not the dominant reason; per-successor exact-state hashing costs roughly 6.6 times the telemetry increment.

The final diagnostic telemetry PRG is 29,165 bytes and the accepted machine-readable evidence is `telemetry-timing-v2.json`. Small raw/checksum differences from `production-timing-v4.json` are whole-program code-layout effects from adding the third executor instantiation, so attribution uses differences between paths in this same binary.

This closes the timing question without forcing one runtime policy. Recorded validation may preserve every successor checksum and run at 5.62 Hz. An interactive mode can keep the 8.34 Hz dynamics path and use a cheaper or less frequent validation policy later, while canonical telemetry format and scheduling remain unchanged.
## Finding

The raw Phase 1 physics loop fits its provisional cadence without weakening the numeric contract or narrowing accepted scenarios. All 44 native tests, exhaustive rust-mos exact execution, target-practical C64 acceptance, and both final three-run common-clock timing gates pass with the unchanged `0x72bf6e0e` mission checksum.

Arithmetic optimization remains paused: serialization is a modest increment, while validation cadence is the larger policy decision. Host capture, C64 presentation, memory reporting, and high-precision accumulated-error evidence now pass; Phase 1 is complete.

## Reproduce

From the project root in PowerShell:

    .\phase1\complete.ps1
    .\phase1\check.ps1
    .\phase1\timing.ps1
    .\phase1\telemetry_timing.ps1

The first script reruns the complete Phase 1 matrix. The second verifies generated inputs and exact execution across native Rust and rust-mos while building the C64 artifacts. The third preserves the accepted raw/checksum measurement. The fourth builds the telemetry timing PRG, requires three stable runs by default, and prints the checksum and serialization deltas, 8 Hz margins, frame/byte counts, and artifact size.