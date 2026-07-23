# Phase 3 integrated missions

The six-step closed-loop runner composes the unchanged KSA-2A physical vehicle,
kinematic steering actuator, imperfect sensors, isolated flight software, and
ballistic post-cutoff world.

| Case | Result | Independent float64 orbit (perigee/apogee) | Cutoff | Abort |
|---|---|---:|---:|---:|
| nominal | complete | 180.627 / 190.752 km | step 3171 | none |
| altimeter dropout T+45-60 s | complete | 180.625 / 190.744 km | step 3171 | none |
| GPS outage T+260-320 s | complete | 181.070 / 190.479 km | step 3171 | none |
| steering jam at T+260 s | abort/ballistic | impact | step 2097 | step 2096 |

The successful cases have a sampled peak dynamic pressure of 41.798 kPa and a
sampled peak proper acceleration of 54.273 m/s2 in KST3. The full-rate runner
measures 41.801 kPa and 55.161 m/s2. All three meet the 180-220 km, eccentricity,
load, cutoff-navigation, and deterministic-checksum requirements.

The independent Python float64 audit is authoritative for the acceptance orbit.
It exposed that the low-resolution fixed-point orbit reporter collapses small
eccentricities toward zero and overstates perigee; that reporter remains useful
on the C64 display but is not used as the high-precision acceptance oracle.

At cutoff, total planar navigation position error is at most 0.016 km and
velocity-vector error at most 0.009 km/s. During the GPS outage, the maximum
position error is 0.255 km and velocity error 0.0035 km/s. Settled nominal
actuator RMS is at most 0.5 degrees and peak error at most two degrees.

The jam is injected at an off-nominal 80-degree position. The monitor observes
16 consecutive steps above two degrees, latches abort at step 2096, commands
cutoff for step 2097, never reignites, requests recovery, and continues the
vehicle ballistically. Recovery physics remains deferred.

## KSC3

The four 96-byte KSC3 images bind to the exact KSA-2A KSC2 scenario identity and
the CRC of its content bytes (excluding the image's own trailing CRC). They
freeze the seed, rates, latency-related constants, actuator limits, monitor
thresholds, and fault windows. Their parser rejects corruption, unknown values,
nonzero reserved bytes, or a different KSC2 base.