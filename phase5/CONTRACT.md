# Phase 5 contract

## Compatibility

Phase 5 is additive. KSC2, KSC3, KST3, KRP3, every Phase 4 family, and all existing entry points remain unchanged. The generated contract binds the accepted KSC2 source CRC `0x5d362512`. A dedicated planar-reduction gate reruns the accepted Phase 3 nominal path and requires its exact truth, sensor, navigation, flight, KST3, and Phase 2 compatibility checksums.

## Frames and cadence

ECI is right-handed with +Z through the north pole, +X through longitude zero at the reference epoch, and +Y completing the frame. ECEF coincides with ECI at the epoch and rotates about +Z. Body +X points through the nose, +Y points vehicle-right, and +Z completes the body frame. The scalar-first Hamilton quaternion actively maps body vectors into ECI.

The mission cadence remains 0.125 seconds. Attitude, actuators, IMU, bending, and slosh use four exact 0.03125-second substeps. Translation, aiding, guidance, sequencing, and canonical KST5 observations use the mission cadence.

## Reference mission

KSA-5A retains KSA-2A stage propulsion, propellant, dry masses, burn timing, and axial aerodynamic data. Its payload is 12 tonnes. It launches from 28.5 degrees geocentric north latitude at longitude zero and targets a 200 km, 51.6 degree orbit. The upper-stage 0.10-tonne RCS allocation is carved from its existing dry-mass allocation so initial stage mass remains source-bound.

## Numeric behavior

All new product math uses explicit checked fixed point and the existing two-word widening primitives. Quaternion is Q1.30; angular rate and modal state are Q24; inertia is Q12 t m2; torque is Q16 MN m; geometry is Q16 m. Saturation is never accepted as a valid successor. Quaternion zero norm, range escape, model-envelope violations requiring safeing, and arithmetic overflow are explicit outcomes.

## Scope boundary

The accepted model contains spherical rotating Earth, co-rotating atmosphere, axisymmetric rigid mass properties, one bending and one slosh mode per transverse axis, two-axis gimbal, powered roll authority, and fuel-accounted coast RCS. Higher structural modes, pogo, nonlinear finite elements, oblateness, third-body gravity, throttle optimization, and multi-C64 transport are excluded.
