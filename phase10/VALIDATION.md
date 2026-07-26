# Phase 10 validation

Status: accepted.

## Authority split

`GlobalEcef6DofV1` is the sole production model. External tools and the
independent float64 implementation produce evidence; they never own, correct,
or co-propagate production state.

The validation layers are:

1. exact numeric/time vectors;
2. frame-only and round-trip fixtures;
3. fixed-state gravity, atmosphere, and rotating-frame snapshots;
4. one-step and frame-transition tests;
5. a complete uninstrumented fixed-point versus independent-float64 mission;
6. separate integrated truth-blind avionics evidence;
7. KSA-5A orbital-coast corroboration;
8. deterministic campaigns and bounded stock-C64 probes.

This separation makes a frame/time convention error distinguishable from a
force-model difference or integration accumulation.

## Earth and time

The accepted contract freezes WGS 84, `CentralJ2V1`,
`Iers2010CompiledV1`, the accepted epoch
`2024-01-01T00:00:00 UTC`, and pinned leap/EOP source hashes and coverage.
Elapsed TAI is continuous. Missing coverage and extrapolation requests fail
before propagation.

Fixtures cover:

- UTC/TAI/TT/UT1 conversion and a leap boundary;
- the accepted EOP interval and both out-of-coverage sides;
- the equator, both sides of the date line, high altitude, near poles, and
  exact poles with a declared reference meridian;
- ECEF/GCRF quaternion, angular velocity, angular acceleration, and state
  transport;
- ENU/ECEF/GCRF round trips for position, velocity, attitude, angular rate,
  and Q16 time.

The checked-in transform fixtures are provenance-bound to pinned standards
data and SatKit 0.16.x generation. Routine tests are offline.

## Physical reference

The exact production reference is
`phase10/generated/uninstrumented-exact-v1.json`. The independent model is
`phase10/reference/analyze_nominal.py`; it uses float64 equations and a
separately implemented mission loop.

| Metric | Exact fixed point | Independent float64 | Difference |
|---|---:|---:|---:|
| Apogee | 205.268066 km | 205.271479 km | 0.001663% |
| Downrange | 339.003662 km | 339.051281 km | 0.014047% |
| Landing position | — | — | 0.048907 km |
| Maximum transition time | — | — | 0.03125 s |
| Maximum flight-event time | — | — | 0.03125 s |
| Transition attitude | — | — | 0.0000348° |
| Landing time | 964.53125 s | 964.625 s | 0.09375 s |

Rail clearance, burnout, apogee, drogue, and main agree within one 32 Hz
step. The fixed-point recovery descent accumulates quantization over hundreds
of seconds, so terminal ground contact has a separately declared four-step
(0.125 s) bound. This is an explicit accepted deviation from applying the
one-step event criterion to terminal contact; it is not hidden coefficient
tuning.

The independent physical model deliberately excludes the closed-loop avionics
implementation. The latter has its own truth-blind analytic, link, fault,
checksum, and campaign evidence. Comparing a prescribed-attitude reference to
the controlled mission would conflate controller behavior with physical-model
error.

## Controlled mission

The accepted nominal controlled mission has evaluation identity `0x09fbd185`:

- duration: 687.9375 s;
- 22,015 avionics releases;
- apogee: 210.896973 km;
- downrange: 336.168945 km;
- crossrange: -6.818359 km;
- transitions: ENU→ECEF→GCRF→ECEF→ENU;
- complete recovery with no numeric, frame, time, or model-envelope fault.

The artifact manifest hashes the CSV, self-contained HTML report, KMR10
recording, KPH10 plot, KSR10 summary, and KTT10 telemetry.

## Orbital corroboration

The frozen KSA-5A insertion state remains a Phase 5 artifact. Phase 10 maps
that state into its declared GCRF epoch and performs a 5,350-second
central-plus-J2 coast. The fixed-point and independent float64 terminal states
remain within the declared 5 km and 5 m/s tolerances. SatKit is the preferred
secondary fixture source; GMAT remains optional and non-gating.

This validates a cross-profile handoff and bounded near-orbital coast. It does
not claim powered KSA-5A flight under the research-scale Phase 10 formats.

## Campaigns

Seed `0x4b5341a0` drives both accepted campaigns. Earth/time/transform inputs
are fixed experiment identities rather than sampled uncertainty.

- 64/64 routine cases physically recover.
- 256/256 completion cases physically recover.
- No case records numeric, frame, time, or model-envelope faults.
- One-, four-, and eight-worker ordered archives are byte-identical.

Storage, reporting, presentation, and worker count remain outside physical
identity.

## Stock-C64 evidence

The stock endpoint receives KLR10 sensor/aid/frame cells and returns exact
command/status cells through KLF6. The host world remains authoritative.

- 33-release release-class probe:
  sensor `bc1909a0`, command `f20dafa0`, status `93441006`,
  navigation `e6d2eebe`, flight `9956643c`.
- Five-release transition probe:
  transition mask `0f`, sensor `f0b1d614`, command `28ec2ebc`,
  status `fd0b240a`, navigation `e33ffaa4`, flight `4729a6b9`.
- Stock replay:
  identity `09fbd185`, 128 points, transition mask `0f`,
  cue hash `dc2aadfe`.

Warp was disabled. Each probe used one VICE process, which was closed before
the next began.

The endpoint is not realtime. Measured PAL costs are 1,689,887 cycles for a
fast release, 2,136,880 for an aided release, 2,237,024 for a GNSS release,
and 3,512,697 for a frame-transition release. These are respectively 54.9,
69.4, 72.7, and 114.1 nominal 32 Hz PAL release slots.

## Limits

- KSA-G10R is fictional and assumption-backed.
- U.S. Standard Atmosphere 1976 is an idealized compiled profile.
- Gravity stops at central plus J2.
- No thermal protection, ablation, lifting entry, precision landing, live
  space weather, or runtime empirical atmosphere is modeled.
- No complete Phase 10 C64 mission was run.
- The evidence validates implementation behavior and declared numerical
  tolerances, not a real vehicle or flight-safety case.
