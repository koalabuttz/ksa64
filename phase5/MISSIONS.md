# Phase 5 integrated missions

Gate 8 composes the multirate KSA-5A vehicle, strict spatial sensor transport,
truth-isolated navigation, body-frame quaternion control, sequencing, and
launch-plane guidance. The guidance follows a host-generated local-horizontal
reference instead of holding a fixed inertial heading; this distinction was
required to remove a false late-ascent radial component.

The reviewed launch azimuth is 42.4 degrees east of north. It compensates for
the launch site's inertial eastward velocity and produces a nominal 51.618
degree orbit. The stage-two cutoff command is issued at mission step 3132.
The target path uses stage-specific gains and projects the one-frame-late star
tracker attitude to the current gyro epoch. Gate 7 retains its legacy exact
controller path and signature unchanged.

## Reviewed missions

| Mission | Outcome | Terminal step | Independent float64 result |
|---|---|---:|---|
| nominal | stable orbit | 3133 | 181.450 x 207.246 km, 51.618 deg |
| gust plus slosh | stable orbit | 3133 | 181.204 x 208.443 km, 51.606 deg |
| star outage plus gyro bias | stable degraded orbit | 3133 | 167.657 x 247.225 km, 51.658 deg |
| two-axis gimbal jam | irreversible abort | 1103 | no insertion claim |
| damping loss | irreversible abort | 958 | no insertion claim |
| RCS leak/depletion | stable degraded orbit | 3133 | 160.726 x 267.106 km, 51.517 deg |

The nominal and gust missions meet the scenario's 180-220 km apsis envelope
and 0.2-degree inclination tolerance. The sensor-outage and RCS-depletion
missions remain safely above the 120 km stable-orbit threshold but are
intentionally treated as degraded cases rather than nominal targeting evidence.
The jam and damping-loss missions latch safeing and cannot reignite.

The sampled nominal peak dynamic pressure is 43.655 kPa, peak angle of attack
is 11.645 degrees, maximum retained flexible state is 0.00312 in its declared
Q8.24 representation, and maximum navigation position error is 0.679 km.

`mission-reference-v1.json` is produced by an independent Python float64 audit
of the frozen raw terminal vectors. Rust acceptance separately freezes every
mission's outcome, terminal step, event mask, and checksum. The guidance-point
signature is `0xada003ef`; the finite rust-mos probe checks it without starting
a complete C64 mission.