# Phase 5 spatial campaigns

Gate 10 extends the proven keyed Phase 4 sampling model to KSA-5A without changing any Phase 3/4 format. The reviewed catalog varies payload mass, each stage's thrust, atmospheric density, aerodynamic scale, body-frame IMU bias, barometer bias, spatial GPS bias, global sensor noise, and gimbal lag/slew. Guidance gains, event topology, vehicle topology, and the reviewed pitch/azimuth program remain frozen.

Run zero is the exact Gate 8 nominal mission: sensor seed `0x5a000000`, zero variation, and identical terminal state plus sensor/navigation/flight checksum chains. Later runs use draws keyed by master seed, run index, parameter ID, correlation group, and draw index. Execution order and host worker count therefore cannot affect a run.

## KSC5 and KSR5

KSC5 is a 704-byte strict campaign configuration with capacity for 24 distribution records. Its records use the five frozen Phase 4 distribution families but bind the Phase 5 numeric and scenario identities. KSR5 is a 160-byte strict run summary carrying campaign/run/variation identity, outcome, terminal spatial state, orbital proxies, load and navigation extrema, checksum chains, reserved bytes, and CRC-32.

Both codecs are allocation-free. The finite rust-mos probe validates configuration/sample/summary round trips with signature `0xc921a2d2`; its size-optimized image is 14,445 bytes and it starts no mission.

## Frozen campaigns

The master seed is `0x4b534135`. Routine evidence uses 32 runs; the reviewed reference uses 256.

| Evidence | 32 runs | 256 runs |
|---|---:|---:|
| Stable orbit | 24 | 180 |
| Complete, not orbit | 2 | 28 |
| Safe abort | 6 | 48 |
| Numeric fault / step limit | 0 / 0 | 0 / 0 |
| Ordered KSR5 chain | `0xde13cb6f` | `0x3103d833` |

The 256-run KSC5 CRC-32 is `0x5402172a`; its KSR5 SHA-256 is `4d8f3f03b8d2bcede65a7dab45245768e76f5d5488043ed22a902d247bef9ea9`. Serial and eight-worker executions are byte-identical.

The independent float64 analyzer finds 208 terminal states suitable for orbit propagation. Their inclination stays within 51.6022–51.6291 degrees. Perigee spans 35.9–187.9 km and apogee 184.4–396.6 km; 184 propagated cases retain at least 120 km perigee. Maximum dynamic pressure spans 42.0–45.7 kPa and maximum navigation position error remains below 0.691 km.

The 48 safe aborts are an engineering result: the frozen controller is sensitive to the reviewed actuator dispersion. Gate 10 records that evidence rather than tuning it away. Optimization may begin only after Gate 11 measures representative target kernels, and any tuning must create new reviewed evidence rather than overwrite this campaign.