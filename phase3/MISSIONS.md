# Phase 3 integrated missions

The six-step closed-loop runner composes the unchanged KSA-2A physical vehicle,
kinematic steering actuator, imperfect sensors, isolated flight software, and
ballistic post-cutoff world.

| Case | Result | Orbit (perigee/apogee) | Cutoff | Abort |
|---|---|---:|---:|---:|
| nominal | complete | 183.788 / 183.788 km | step 3171 | none |
| altimeter dropout T+45-60 s | complete | 183.786 / 183.786 km | step 3171 | none |
| GPS outage T+260-320 s | complete | 183.789 / 183.789 km | step 3171 | none |
| steering jam at T+260 s | abort/ballistic | impact | step 2097 | step 2096 |

The nominal/recoverable peak dynamic pressure is 41.801 kPa and peak proper
acceleration is 55.161 m/s2. The three successful cases meet the 180-220 km,
eccentricity, load, cutoff-navigation, and deterministic-checksum requirements.
The GPS outage bridge remains inside 5 km and 30 m/s before reacquisition.
Settled nominal actuator RMS is at most 0.5 degrees and peak error at most two
degrees.

The jam is injected at an off-nominal 80-degree position. The monitor observes
16 consecutive steps above two degrees, latches abort at step 2096, commands
cutoff for step 2097, never reignites, requests recovery, and continues the
vehicle ballistically. Recovery physics remains deferred.

## KSC3

The four 96-byte KSC3 images bind to the exact KSA2A KSC2 scenario identity and
full-image CRC. They freeze the seed, rates, latency-related constants, actuator
limits, monitor thresholds, and fault windows. Their parser rejects corruption,
unknown values, nonzero reserved bytes, or a different KSC2 base.
