# Phase 5: three-dimensional rigid-body dynamics

Status: Gate 1 complete; Gate 2 spatial numeric work is next.

The Phase 5 numeric/scenario contract and KSA-5A configuration are now frozen
under `phase5/`. Generated Rust constants bind the new work to the accepted
Phase 2/3 checksums without changing any inherited format or entry point.

Phase 4 completed the maturity prerequisites for 3-D work: the 2-D world is deterministic and independently checked, flight software is isolated from truth, campaign variation is reproducible, storage is observational, and stock/REU evidence paths are strict.

## Inherited contracts

Phase 5 must preserve the complete Phase 3/4 planar path and its frozen artifacts. A 3-D scenario should be additive, carry a new explicit contract identity, and reduce to the accepted planar equations when out-of-plane state, moments, and torques are zero. REU support remains optional and cannot become a physics dependency.

The existing target evidence also sets expectations:

- correctness and bounded arithmetic come before real-time execution;
- host-native campaigns provide statistical breadth;
- finite MOS/VICE probes provide instruction-level exactness;
- a long C64 run requires a fresh projection and explicit confirmation;
- optimization begins only after representative 3-D kernels are measured.

## Recommended first planning gates

1. Freeze frames, axes, handedness, units, quaternion convention, and state ownership.
2. Perform range analysis for quaternion, angular rate, inertia, torque, and cross products.
3. Build exact fixed-point vector/quaternion kernels with analytic identity and overflow tests.
4. Add independent float64 torque-free, constant-torque, and spherical-inertia references.
5. Prove a zero-torque planar reduction reproduces accepted Phase 4 translational checksums.
6. Measure representative attitude and coupled 6-DOF steps on host, mos-sim, and PAL VICE before choosing cadence or optimization work.

Candidate later gates are rigid-body propagation, mass/inertia schedules, gimbal torque, sensor extensions, attitude navigation/control, failure cases, 3-D telemetry, and campaign integration. Those choices remain deliberately unfrozen until Phase 5 is planned with explicit model scope.