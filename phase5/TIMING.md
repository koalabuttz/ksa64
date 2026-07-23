# Phase 5 PAL C64 timing

Gate 11 measures three naturally terminating kernels under the pinned PAL x64sc
3.10 emulator and rust-mos image. It does not start a complete target mission.
Each result is checked against a native exact-arithmetic execution before timing
is accepted.

## Final measurements

Three repeated runs are cycle-identical. CIA boundary cost is 24 cycles and is
subtracted from every region.

| Region | Cycles per 0.125 s mission step | Share |
|---|---:|---:|
| Four-substep spatial vehicle | 15,565,702 | 76.80% |
| Sensor, navigation, guidance, command codecs | 2,579,033 | 12.72% |
| Canonical KST5 encode, rolling hash, volatile copy | 2,124,185 | 10.48% |
| Conservative composed path | 20,268,920 | 100% |

All three executables fit below `$c000` on a stock C64. Their file sizes are
37,970, 46,943, and 10,290 bytes respectively. They are deliberately separate:
a monolithic probe overflowed the stock link region by 33,780 bytes, while the
split probes measure the same additive boundaries without making an REU a code
requirement.

## Projection and run decision

The nominal mission has 3,133 successor steps. At 985,248 PAL cycles per second,
the measured sum projects to 17.90 hours. Applying the required ten-percent
margin gives 19.69 hours, well beyond the 30-minute automatic-run threshold.
The full target mission is therefore ineligible and was not started.

At the same conservative rate, a 32-run routine campaign projects to 26.26 days
and the 256-run reference campaign to 210.07 days. Native execution remains the
campaign breadth path; finite target probes remain the exactness path.

## Measured optimization

The first timing pass measured 20,299,184 cycles per composed step. Campaign
parameterization had introduced general million-scale arithmetic even when a
scale delta was exactly zero. A zero-delta vehicle fast path preserves the
frozen operation result and reduces the composed cost to 20,268,920 cycles,
saving 30,264 cycles or 0.149 percent. It costs 267 bytes in the vehicle probe
and remains well inside stock RAM.

A second candidate replaced stage-inertia division with the existing reduced
16-bit-denominator primitive. Native missions and tests remained exact, but the
rust-mos telemetry probe exposed a different initial inertia state. The change
was rejected and reverted. This is recorded as evidence that an optimization
is not accepted merely because native tests pass.

Vehicle dynamics remain the dominant cost. Further optimization should profile
its quaternion normalization, spatial environment, aerodynamic, and repeated
force/mass divisions individually. It must retain the rigid-body multi-call-site
specialization constraint and pass native, rust-mos, KST5, and campaign evidence
before acceptance.