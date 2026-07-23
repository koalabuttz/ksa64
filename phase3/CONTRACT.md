# Phase 3 contract

Phase 3 introduces closed-loop avionics while retaining the physical KSA-2A
vehicle and all Phase 2 artifacts byte-for-byte.

## Dependency boundary

```text
ksa64-interface <- ksa64-flight
        ^                 ^
        |                 |
ksa64-core ----------> ksa64-sim
```

`ksa64-flight` may depend only on `ksa64-interface`. It cannot import
`PlanarTruthState`, atmosphere, vehicle, or simulator code. `ksa64-sim` is the
composition root and the only new crate allowed to see both worlds.

## Step order

1. Apply the previously accepted actuator command.
2. Advance stage machinery and vehicle truth.
3. Generate imperfect sensors from successor truth.
4. Advance flight software.
5. Validate and latch the next actuator command.
6. Emit telemetry.

Step zero supplies a bootstrap sensor frame. Normal command latency is one
simulation step (0.125 seconds).

## Stable transports

All messages are fixed width, little endian, allocation free, and protected by
CRC-32/IEEE. Parsers reject unknown flags, nonzero reserved bytes, invalid enum
values, inconsistent sequence numbers, and invalid checksums.

| Message | Size | Purpose |
|---|---:|---|
| `SensorFrame` | 56 bytes | Imperfect measurements and discrete feedback |
| `ActuatorCommand` | 16 bytes | Pitch, engine, separation, and safeing requests |
| `FlightOutput` | 52 bytes | Navigation, mode, alarms, command, checksums |

The `recovery_requested` command bit is transport-safe. Phase 3 does not model
recovery physics.

## Explicit exclusions

Phase 3 does not add rigid-body dynamics, throttle, an EKF, recovery physics,
REU dependence, or a Monte Carlo campaign.
