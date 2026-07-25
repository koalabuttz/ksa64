# Phase 8.5 handoff: unified avionics and execution profiles

Status: planned next, before Phase 9.

Phase 8.5 makes vehicle category, model profile, coordinate frame, avionics profile, and execution placement independent. It adds the avionics-aware evaluation boundary that Phase 9 optimization must consume while preserving every accepted Phase 0-8 artifact.

## Canonical terminology

- `VerticalPointMassV1`: canonical public name for frozen identity 3 (`HobbyVerticalV1`).
- `LocalEnu6DofV1`: canonical public name for frozen identity 4 (`HobbySpatialV1`).
- Legacy names remain compatibility aliases; no existing format, discriminant, checksum, fixture, or parser changes.
- Future `GlobalEcef6DofV1` and orbital ECI profiles are selected by model envelope, not by who owns or classifies the vehicle.

## Required boundary

An evaluation identity binds:

```text
vehicle + physical model + coordinate frame + mission + environment
        + avionics + actuator capabilities + uncertainty + evaluator
```

The shared avionics kernel retains the 32/8/1 Hz schedule, sensor validation, navigation framework, guidance, control, sequencing, health monitoring, safeing, and evidence. Profile data supplies local or orbital navigation semantics, guidance programs, mission phases, sensors, and available actuators.

The local avionics-aware executor uses an exact event clock. Existing Phase 8 physical timesteps become maximum steps and are split at exact 31.25 ms releases and mission-event boundaries. Sensors are sampled at release N, command N is produced from those measurements, and that command becomes effective at release N+1; continuous commands are held between releases. This separately identified executor may produce a slightly different trajectory, so the frozen Phase 8 executor and artifacts remain intact.

Guidance produces a body-control demand that a statically selected, capability-bound allocator maps to physical actuator commands. Phase 8.5 implements monitor-only allocation for the original Firestorm and a bounded two-axis motor-gimbal allocator for an explicitly fictional derivative. The solid motor cannot be cut off, the rail remains authoritative before release, gimbal authority exists only during powered flight, and the derivative becomes passive after burnout. Canard, RCS, and mixed-effector allocators are identified as future capabilities but fail closed until Phase 9.5 implements their physics.

## Deployment targets

1. Host world plus host avionics for rapid development and the exact reference.
2. Host world plus one VICE/C64 avionics endpoint with passive host Mission Control and observational native shadow validation.
3. Combined C64 world and avionics using an in-memory loopback with the identical sensor-N/command-N/effective-N+1 contract.
4. The accepted standalone Phase 8 C64 world remains supported for deliberately long runs regardless of combined-image feasibility.

The original Firestorm receives full navigation, telemetry, health monitoring, and autonomous dual-deploy sequencing, with attitude control in monitor-only mode because the real reference declares no steering actuator. A separately identified fictional derivative uses the two-axis motor gimbal to provide explicit powered-flight control authority.

## Stock target and run policy

Measure the combined incremental avionics kernel rather than adding the complete standalone transport endpoint to the nearly full Phase 8 image. Link one profile, exclude host-only compilers/evidence/UI, and optimize only measured size or cycle hotspots. Banking, RAM under ROM, overlays, or optional REU storage may extend capability, but stock execution remains the baseline and storage cannot affect physics.

No long target run begins merely to discover its duration. Measure finite kernels, publish a projection, require explicit confirmation under the existing threshold policy, and close the single VICE instance after success or proven failure.

## Exit criteria

- Frozen Phase 0-8 evidence remains exact and the terminology migration is non-breaking.
- Avionics-aware evaluation and capability identities are strict and reproducible.
- Host/host and host/VICE commands, navigation, status, alarms, and terminal checksums agree exactly.
- Host, VICE, and monolithic loopback agree on every exact release time, split step, held-command interval, and sensor-N/command-N/effective-N+1 transition.
- Truth-triggered Phase 8 recovery remains available; avionics-commanded recovery is separately identified and truth-blind.
- The original Firestorm remains monitor-only for attitude, while the fictional gimbal derivative respects rail constraint, actuator limits, powered-flight availability, and post-burnout loss of authority.
- Monolithic loopback and split execution have identical endpoint ordering and bounded failure behavior.
- Combined stock-C64 packaging/timing evidence is recorded without removing the standalone long-run image.
- Frame identities and transform contracts are extensible to ECEF/ECI without implementing global propagation in this phase.

## Deferred

Phase 9 owns optimization. Phase 9.5 owns aerodynamic canards, cold-gas RCS, and mixed-effector control allocation. Phase 10 owns global ECEF atmospheric/suborbital dynamics and ENU-to-ECEF-to-ECI handoff. Physical user-port, ACIA, and Ultimate acceptance remains a separate Phase 6 hardware boundary.
