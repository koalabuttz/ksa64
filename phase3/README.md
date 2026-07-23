# Phase 3: simulated avionics and closed-loop flight

Status: complete.

Phase 3 turns KSA64 from a trajectory program into a closed-loop aerospace simulation architecture. The physical KSA-2A vehicle remains unchanged from Phase 2; new fixed-width transports isolate vehicle truth from simulated sensors, aided navigation, guidance/control, sequencing, and the kinematic steering actuator.

## What is implemented

- `ksa64-interface`: allocation-free sensor, actuator, and flight-output transports with strict CRC-checked parsers.
- `ksa64-flight`: truth-blind aided inertial navigation, flight phases, insertion guidance, sequencing, alarms, and fail-closed abort behavior.
- `ksa64-sim`: the composition root for world truth, deterministic sensors and faults, actuator enforcement, closed-loop missions, and KST3 telemetry.
- `ksa64-host`: strict whole-stream KST3 inspection and validated KRP3 derivation.
- Independent Python float64 validation of orbit, coast, loads, and navigation.
- Finite PAL C64 probes and a strict PETSCII/SID KRP3 replay.

## Frozen cases

| Case | Expected result |
|---|---|
| nominal | closed-loop insertion and stable orbit |
| altimeter dropout, T+45 to T+60 s | navigation bridges the recoverable outage and reaches orbit |
| GPS outage, T+260 to T+320 s | inertial navigation bridges the outage and reaches orbit |
| steering stuck at T+260 s | monitor detects loss of control, latches abort, safes propulsion, and continues ballistically |

Canonical KSC3 configuration, KST3 telemetry, and KRP3 presentation artifacts are in `phase3/examples/`. Every artifact has a reviewed SHA-256 sidecar.

## Reproduce the accepted result

Native evidence and regression audit:

    ./phase3/check.ps1

Full Phase 3 completion audit, including finite PAL C64 timing and replay:

    ./phase3/complete.ps1

The target timing gate never starts a full C64 mission unless the pre-run projection is at most 30 minutes and the program fits stock RAM. The accepted projection is 243.7 minutes, so the full target mission is correctly ineligible; no run is canceled to obtain the result.

## Design records

- `CONTRACT.md`: dependency and step-order contract.
- `SENSORS.md`: measurement, delay, noise, and fault model.
- `NAVIGATION.md`: aided inertial navigation and tuning evidence.
- `GNC.md`: phases, insertion guidance, monitoring, and abort behavior.
- `MISSIONS.md`: frozen scenarios, acceptance limits, and results.
- `TELEMETRY.md`: KST3 and KRP3 contracts.
- `TIMING.md`: target probes, memory, full-run decision, and replay evidence.
- `COMPLETION.md`: exit-criterion audit and Phase 4 handoff.
