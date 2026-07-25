# Phase 9 handoff — optimization over avionics-aware evaluations

Phase 9 can begin from the accepted Phase 8.5 host evaluator. It must use the avionics-aware identity and summary rather than optimizing only the frozen Phase 8 truth-triggered physical boundary.

## Stable input boundary

Each candidate binds vehicle, motor, mission, environment, physical profile, reference frame, avionics profile, actuator capability, uncertainty case, and evaluator version. Host/host versus host/VICE placement is not part of physical identity.

The optimizer remains outside the portable core:

```text
candidate generator -> compiled bounded packs -> evaluate_with_avionics
                    -> KAS8 objectives/constraints -> archive/Pareto analysis
```

## Available metrics and evidence

Phase 9 can consume physical outcomes and the 32-slot evaluation metrics plus navigation error, attitude error, saturation, deployment decisions/feedback, alarms, deadlines, link losses, and six checksum chains. The deterministic keyed campaign engine is independent of worker count and run order.

## Required preservation

- Keep Phase 0–8.5 artifacts and executors frozen.
- Candidate selection, storage, presentation, and worker count cannot change evaluation results.
- Do not make the C64 perform large production searches; use it for bounded finalist evaluation, browsing, and replay.
- Keep monitor-only and gimbal capabilities separately identified.
- Treat the combined-stock decision as orthogonal to host optimization; Phase 9 must not silently choose an overlay, REU requirement, feature cut, or rewrite.

Phase 9.5 remains the owner of canards, cold-gas RCS, mixed-effector allocation, and authority transitions. Phase 10 remains the owner of ECEF/ECI global flight.
