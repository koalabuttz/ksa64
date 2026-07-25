# Phase 9.5 handoff — advanced control effectors

Phase 9.5 may begin from the accepted Phase 9 evaluator and optimization workbench. It should add new physical capabilities without changing the Phase 8.5 scheduler, navigation, truth boundary, or existing monitor/gimbal identities.

## Stable boundaries

- Guidance emits an effector-neutral desired body torque.
- A capability-bound allocator maps demand to installed actuators.
- Vehicle, physical profile, frame, mission, environment, avionics, capability, uncertainty, evaluator, and optimizer identities remain orthogonal.
- Search and presentation stay host-side; exact candidate evaluation remains in the portable implementation.
- Existing gimbal-only evaluations and all Phase 0–9 artifacts remain frozen.

## Intended additions

- Aerodynamic canards with geometry, dynamic-pressure/Mach/AoA authority, drag, hinge-load, lag, slew, saturation, envelope, and failure models.
- Cold-gas RCS thruster sets with placement, valve/minimum-impulse behavior, deterministic pulse allocation, consumable depletion, and changing mass properties.
- Phase-aware mixed allocation and deterministic authority handoff among gimbal, canards, and RCS.
- New optimization variables and constraints for effector sizing, placement, mass, consumables, controller gains, handoff points, and robust margins.
- Independent analytic/float64 force, torque, depletion, and transition evidence plus bounded stock-C64 exact probes.

Phase 9.5 must not smuggle unsupported aerodynamics into the accepted Firestorm envelope or present optimized model output as launch or safety approval. Phase 10 remains responsible for global ECEF atmospheric/suborbital dynamics and ECI handoff.
