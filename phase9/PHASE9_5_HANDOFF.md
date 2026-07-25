# Phase 9.5 handoff — advanced control effectors

Phase 9.5 may begin from the accepted Phase 9 evaluator and optimization workbench. It should add new physical capabilities without changing the Phase 8.5 scheduler, navigation, truth boundary, or existing monitor/gimbal identities.

## Stable boundaries

- Guidance emits an effector-neutral desired body torque.
- A capability-bound allocator maps demand to installed actuators.
- Vehicle, physical profile, frame, mission, environment, avionics, capability, uncertainty, evaluator, and optimizer identities remain orthogonal.
- Search and presentation stay host-side; exact candidate evaluation remains in the portable implementation.
- Existing gimbal-only evaluations and all Phase 0–9 artifacts remain frozen.
- During any interval, exactly one KSA64 world model owns and advances each entity's state.

## Intended additions

- Aerodynamic canards with geometry, dynamic-pressure/Mach/AoA authority, drag, hinge-load, lag, slew, saturation, envelope, and failure models.
- Cold-gas RCS thruster sets with placement, valve/minimum-impulse behavior, deterministic pulse allocation, consumable depletion, and changing mass properties.
- Phase-aware mixed allocation and deterministic authority handoff among gimbal, canards, and RCS.
- New optimization variables and constraints for effector sizing, placement, mass, consumables, controller gains, handoff points, and robust margins.
- Independent analytic/float64 force, torque, depletion, mass-property, actuator, allocation, and transition evidence plus bounded stock-C64 exact probes.

## Validation policy

KSA64 owns the production canard, RCS, depletion, changing-mass, actuator, allocator, and handoff models. Primary evidence comes from analytic fixtures and a small independent float64 implementation.

Basilisk may be used only as optional secondary evidence for selected fixed-step spacecraft-attitude/RCS force, torque, pulse, depletion, and mass-property cases. It is not an oracle for canard aerodynamics, the exact 32 Hz event clock, mixed-effector allocation, or authority handoff. It cannot become a runtime, build, or CI dependency.

Any external comparison must be frozen as a provenance-complete fixture. Normal Phase 9.5 acceptance remains offline. Do not expand Phase 9.5 merely to implement SatKit, Orekit, GMAT, global frames, or other Phase 10 validation infrastructure.

## Phase boundary

Phase 9.5 must not smuggle unsupported aerodynamics into the accepted Firestorm envelope or present optimized model output as launch or safety approval. Phase 10 remains responsible for global ECEF atmospheric/suborbital dynamics, ECI handoff, Earth/time/frame contracts, and specialized external-reference fixtures.
