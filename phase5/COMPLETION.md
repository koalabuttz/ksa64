# Phase 5 completion record

Status: complete. KSA64 now has a deterministic three-dimensional launch-vehicle simulator with ECI translation, rigid-body attitude, bounded flexible dynamics, spatial avionics, truth-isolated guidance, canonical telemetry, deterministic campaigns, optional capacity-scaled history, and stock-C64 replay. Phase 3 and Phase 4 contracts remain unchanged.

## Exit criteria

| Criterion | Evidence | Result |
|---|---|---:|
| Standard spatial and rigid-body cases pass | Independent exact vectors cover vector/quaternion operations, torque-free motion, diagonal Euler coupling, driven and undamped flexible modes, symmetry, and fail-closed numeric cases. Native and bounded rust-mos probes agree. | Pass |
| Quaternion normalization and drift are controlled | Scalar-first Hamilton quaternions use checked deterministic normalization at the 32 Hz fast cadence; long rigid-body reference cases remain bounded and match the independent model. | Pass |
| Reduced and complete 6-DOF cases are independently checked | Independent Python models verify world, vehicle, avionics, guidance, six integrated missions, KST5 bytes, KSC5/KSR5 campaigns, and KPH5 history. | Pass |
| Flight software remains isolated from truth | Spatial navigation and control depend on fixed-width sensor and actuator transports, not simulator truth types. One-step command latency and world authority remain explicit. | Pass |
| Recording and storage are observational | Recorded and unrecorded nominal summaries are exact; stock and every REU allocation use the same run identities and cannot affect mission state or later seeds. | Pass |
| Stock hardware has useful presentation | A 1,664-byte KPH5 tape drives a 6,252-byte PETSCII/SID replay whose complete screen and cue schedule pass PAL VICE validation. | Pass |
| Target feasibility is measured without an unbounded run | Separate naturally terminating vehicle, avionics, and telemetry probes agree with native exact results and project a conservative complete mission duration before any long run is considered. | Pass |

## Frozen mission evidence

The reviewed KSA-5A nominal mission completes 3,133 command steps. Its independent float64 orbit has 181.450 km perigee, 207.246 km apogee, 0.001962 eccentricity, and 51.6175 degree inclination. Maximum dynamic pressure is 43.655 kPa, maximum sampled angle of attack is 11.646 degrees, and maximum navigation position error is 0.679 km.

The gust/slosh case also remains inside the reviewed 180-220 km apsis and 51.6 plus or minus 0.2 degree inclination envelope. Star-tracker outage with gyro bias and RCS leak/depletion remain stable degraded orbits. Gimbal jam and damping loss latch irreversible abort behavior. All six mission summaries and their exact checksums are frozen in `mission-reference-v1.json`.

## Canonical telemetry and campaigns

| Evidence | Accepted result |
|---|---:|
| KST5 nominal frames | 3,134 |
| KST5 bytes | 1,328,912 |
| KST5 stream CRC-32 | `0xa9b3b94c` |
| KST5 observation checksum | `0x5b7b2419` |
| KSC5/KSR5 master seed | `0x4b534135` |
| Routine campaign | 32 runs, ordered chain `0xde13cb6f` |
| Reference campaign | 256 runs, ordered chain `0x3103d833` |
| Reference outcomes | 180 stable orbit, 28 complete non-orbit, 48 safe abort |
| Numeric or step-limit failures | 0 |

Serial and eight-worker reference execution produce byte-identical KSC5 and KSR5 artifacts. The independent analyzer reconstructs every variation and computes orbital results from raw terminal vectors rather than trusting the compact fixed-point classifier.

## Stock and REU history capability

Stock mode retains the streaming campaign aggregate, five deterministic KSR5 summaries (`[0, 1, 4, 53, 2]`), and one 99-point KPH5 nominal history.

| Hardware | Retained summaries | Full KST5 histories | Compact KPH5 histories |
|---:|---:|---:|---:|
| Stock | 5 | 0 | 1 |
| 128 KiB REU | 204 | 0 | 15 |
| 256 KiB REU | 256 | 0 | 34 |
| 512 KiB REU | 256 | 0 | 75 |
| 1 MiB REU | 256 | 0 | 157 |
| 2 MiB REU | 256 | 1 | 113 |
| 4 MiB REU | 256 | 3 | 25 |
| 8 MiB REU | 256 | 6 | 58 |
| 16 MiB REU | 256 | 12 | 123 |

Counts are derived from observed exact KST5/KPH5 byte sizes. The preserving PAL VICE matrix passes no-REU and every supported capacity. The accepted bounded-loop planner replaced a division-based version after PAL target evidence exposed a native/MOS-divergent quotient. Gate 14 also made the probe publish its completion magic only after all result fields, eliminating a monitor race without changing any allocation result.

## Target timing and memory decision

Three stable PAL runs measured 15,565,702 vehicle cycles, 2,579,033 avionics cycles, and 2,124,185 telemetry cycles per mission step. Their conservative sum projects:

- one nominal target mission: 70,898.7 seconds, or 19.69 hours;
- 32 target campaign runs: approximately 26.26 days;
- 256 target campaign runs: approximately 210.07 days.

These exceed the locked 30-minute automatic-run threshold. No complete target mission or campaign was started or canceled. Native missions provide duration and campaign breadth; naturally terminating MOS/VICE programs provide exact target evidence.

The measured target programs all fit the stock `$0801`-to-`$BFFF` load window:

| Program | PRG bytes | Loaded end exclusive |
|---|---:|---:|
| Vehicle timing | 37,970 | `$9C51` |
| Avionics timing | 46,943 | `$BF5E` |
| Telemetry timing | 10,290 | `$3031` |
| History exactness/allocation probe | 5,917 | bounded MOS image |
| REU capacity probe | 4,491 | stock-compatible |
| Mission-control replay | 6,252 | `$206B` |

## Presentation evidence

KPH5 remains presentation-only; KST5 and independent analysis retain physical authority. The accepted KPH5 tape has 99 points, CRC-32 `0xf2b3b81f`, and SHA-256 `2b84eae5871dee6967ea63061aefa1c232159e1803507072344866174adacf5f`.

PAL VICE verifies all 1,000 screen bytes of the stock replay, 85 populated trajectory cells, and cue hash `0x3b2fb64b`. Replay does not execute physics and cannot alter a mission result.

## Claims and limitations

Phase 5 verifies that the declared equations, contracts, and deterministic workflows are implemented consistently. It does not certify KSA-5A as a real vehicle or correlate it against flight, wind-tunnel, engine, structural, or sensor hardware data.

The accepted model intentionally uses a spherical Earth gravity/environment abstraction, bounded table-driven aerodynamics, diagonal rigid-body inertia, reduced bending/slosh modes, simplified actuators and sensors, and a reviewed launch reference rather than a production guidance optimizer. Campaign distributions explore declared parameter ranges; they do not establish real-world probabilities. Compact orbit classifiers and KPH5 histories are selection/presentation aids, not authoritative physical evidence.

## Completion audit

The final bounded audit is:

```powershell
powershell -File phase5/complete.ps1
```

It checks inherited Phase 4 distribution/campaign evidence, every Phase 5 generator and independent analyzer, formatting, compilation, lints, native tests, finite rust-mos probes, the complete PAL REU matrix, stock replay, stable timing, and every checked-in Phase 5 SHA-256 sidecar. It does not start a complete C64 mission or campaign.

Status: accepted. Phase 6 is ready for planning; see `PHASE6_HANDOFF.md`.
