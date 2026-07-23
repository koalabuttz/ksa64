# Phase 3 completion record

Phase 3 is complete. KSA64 now executes deterministic closed-loop missions through an explicit truth/sensor/flight-software/actuator split while preserving the Phase 2 physical vehicle and exact terminal checksum `0xcc57612b` in the compatibility path.

## Exit criteria

| Criterion | Accepted evidence |
|---|---|
| Flight software cannot read private truth | `ksa64-flight` depends only on `ksa64-interface`; `ksa64-sim` is the sole composition root that sees truth and flight software. Compile-time crate boundaries and truth-isolation tests enforce this. |
| Sensor and actuator interfaces are transportable | SensorFrame (56 bytes), ActuatorCommand (16 bytes), and FlightOutput (52 bytes) are fixed-width, little-endian, allocation-free, CRC-protected records with strict fail-closed parsers. |
| Nominal and failure cases are repeatable | Four frozen KSC3/KST3/KRP3 case sets reproduce exact truth, sensor, navigation, and flight checksum chains. Reviewed SHA-256 sidecars bind every artifact. |
| Control performance is measured | Nominal and recoverable cases satisfy the declared orbit, load, navigation, outage-bridge, and steering limits. The stuck actuator is detected and safed deterministically. |

## Accepted mission evidence

Independent Python float64 analysis reads the canonical KST3 bytes without using the Rust parsers and reports:

| Case | Perigee (km) | Apogee (km) | Eccentricity | Outcome |
|---|---:|---:|---:|---|
| nominal | 180.627 | 190.752 | 0.000771 | orbit |
| altimeter dropout | 180.625 | 190.744 | within 0.01 | orbit |
| GPS outage | 181.070 | 190.479 | within 0.01 | orbit |
| steering stuck | n/a | n/a | n/a | latched abort and propulsion safeing |

Across the orbital cases, sampled Max-Q is at most 41.798 kPa and sampled proper acceleration is at most 54.273 m/s^2. Full-rate mission summaries remain below the 60 kPa and 60 m/s^2 limits. Cutoff navigation error is approximately 0.016 km position and 8.66 m/s velocity at worst. During the 60-second GPS outage, peak bridge error is 0.254 km and 3.50 m/s, well inside the 5 km and 30 m/s limits.

The nominal terminal checksum chains are truth `0xc86045a0`, sensor `0x47d11fb0`, navigation `0xc6f9da7b`, and flight `0x02ce28ef`. The independent post-cutoff float64 coast agrees with fixed-point terminal radius within 100 m and radial velocity within 1 m/s.

## Audit findings resolved

The independent audit found two real issues before acceptance:

1. The coarse fixed-point orbit classifier could overstate insertion quality. Acceptance now uses an independent float64 orbit calculation and post-cutoff coast propagation.
2. GPS latency produced about 1.8 km of downrange navigation lag. The measurement is now projected through its declared two-step delay, and a bounded deterministic shift-gain search froze the accepted correction gains.

These corrections changed only Phase 3 navigation/controller behavior. KSA-2A physical vehicle data and Phase 2 artifacts remain unchanged.

## C64 evidence

Three stable PAL VICE runs produce bit-exact native/MOS results for five naturally terminating 64-step probes. The representative composed path costs 1,787,662.2 cycles/step and the GPS-guidance path costs 213,117.0 cycles/step. The 37,830-byte probe PRG fits stock RAM, uses no REU, and projects a conservative full mission time of 243.7 minutes.

The locked full-run rule requires both stock-RAM fit and a projection no greater than 30 minutes. Memory passes and time does not, so a full nominal target run is not started. This is an accepted gate decision, not an interrupted run.

The 26,841-byte KRP3 replay PRG independently validates the compact tape on the C64, processes 906 records through step 7200, renders the accepted PETSCII page, and emits deterministic SID event cues. VICE verifies `PHASE 3 REPLAY PASS` directly from screen memory.

## Reproduction

    ./phase3/check.ps1
    ./phase3/timing.ps1 -Runs 3
    ./phase3/replay.ps1

Or run all Phase 3 and cross-phase completion gates:

    ./phase3/complete.ps1

## Phase 4 handoff

Phase 4 may add explicit REU-backed telemetry history and seeded statistical analysis. It must preserve the accepted Phase 3 mission behavior when recording is disabled, keep REU DMA explicit and bounded, and leave KST3 as the canonical truth/sensor/navigation/flight regression record. Optimization is now appropriate only for measured kernels and must not weaken strict transport validation or change frozen results without a recorded model decision.
