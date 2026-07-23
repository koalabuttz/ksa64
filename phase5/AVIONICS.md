# Phase 5 avionics boundary

Gate 7 adds the spatial sensor, navigation, guidance/control, and actuator path without modifying any Phase 3 or Phase 4 message or artifact.

## Wire contracts

`SpatialSensorFrame` is a strict 128-byte little-endian record. It carries:

- sequence and drifted onboard time;
- three-axis body proper acceleration and angular rate;
- delayed barometric altitude, GPS ECI position/velocity, and star-tracker attitude;
- applied two-axis gimbal feedback and remaining RCS propellant;
- stage state, validity flags, events, reserved bytes, and IEEE CRC-32.

`SpatialActuatorCommand` is a strict 32-byte record containing two-axis gimbal demand, three-axis RCS demand, engine action, separation, abort safeing, reserved space, and CRC. Unknown flags, invalid booleans/enums, non-zero reserved bytes, length errors, and corruption fail closed.

The pinned rust-mos optimizer required the long CRC reduction and stored tail load to be assigned to named temporaries. Without that expression split, the independently correct values compared incorrectly in the combined target parser. Native behavior was unchanged; the finite target probe proves the specialized form.

## Sensor suite

The deterministic sensor suite uses one seeded PRNG and fixed draw order:

| Channel | Rate | Transport latency | Model |
|---|---:|---:|---|
| IMU | four 32 Hz samples aggregated into each 8 Hz frame | none | body proper acceleration, rigid rate, bending/slosh contamination, bias, triangular noise, quantization |
| Clock | 8 Hz | none | bounded ppm drift |
| Barometer | 4 Hz | one mission frame | altitude, 80 km ceiling, bias/noise/quantization |
| GPS | 1 Hz | two mission frames | ECI position and velocity, independent axis bias/noise/quantization |
| Star tracker | 2 Hz | one mission frame | normalized scalar-first body-to-ECI quaternion |
| Actuator feedback | 8 Hz | none | applied gimbal and remaining RCS propellant |

Barometer, GPS, and star-tracker outages are independent. Storage and recording remain observational and cannot change draws or physics.

## Flight software

The `ksa64-flight` crate still depends only on `ksa64-interface`; it cannot access simulator truth. Spatial navigation:

- propagates ECI position and velocity from body IMU measurements and spherical gravity;
- propagates and normalizes attitude from the gyro;
- applies bounded delayed-GPS, radial barometer, and star-attitude aiding;
- rejects sequence gaps, missing inertial channels, and numeric escape;
- maintains an exact rolling checksum.

The attitude controller uses the shortest quaternion error, rate damping, two-axis gimbal authority, and bounded three-axis RCS authority. Sequencing owns ignition, cutoff, and separation. Missing actuator feedback, persistent gimbal tracking error, corrupt transport, or navigation failure latches an irreversible cutoff/safeing response.

Gate 8 will supply and tune the complete launch-plane/ascent/insertion guidance target generator. Gate 7 deliberately validates the truth boundary, estimator, controller, sequencer, and wire loop against explicit attitude targets first.

## Evidence

Independent Python generation freezes:

- a complete sensor wire image and CRC;
- a complete actuator wire image and CRC;
- hold, pitch, roll/rate, and yaw controller cases;
- exact avionics signature `0xaa0a0b0e`.

Native tests cover cadence, latency, source-specific outages, navigation aiding, sequence rejection, fail-closed corruption, persistent tracking abort, and repeated closed-loop exactness. The pinned rust-mos target matches the frozen signature in a 7,367-byte size-optimized probe, well below the 48 KiB stock-profile gate.