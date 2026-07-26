# Phase 10 handoff — global atmospheric and suborbital flight

Phase 9.5 hands Phase 10 a deterministic local-ENU vehicle world, exact event executor, truth-blind avionics boundary, advanced effectors, robust-evaluation workbench, strict telemetry, and split host/C64 placement. Phase 10 must preserve those contracts while adding global frame and time semantics.

## Authority and profile boundary

- Add a separately versioned `GlobalEcef6DofV1` profile. Do not mutate `LocalEnu6DofV1` or any Phase 0–9.5 artifact.
- Exactly one KSA64 world owns and advances an entity during an interval.
- ENU/ECEF/ECI transitions are explicit mission events with identity, epoch, and continuity evidence.
- Advanced gimbal/canard/RCS behavior remains coordinate-agnostic body physics. Frame conversion must not fork or rewrite the effector models.
- Host-world/C64-flight remains the initial target placement. Portable C64-world work stays a priority but is not allowed to weaken the global contract.

## Contract to freeze before dynamics

Phase 10 planning must declare and version:

1. Reference ellipsoid and gravity model.
2. Earth rotation/orientation and any precession/nutation model.
3. Supported input/output time scales and the continuous internal integration scale.
4. Leap-second and Earth-orientation datasets, hashes, validity windows, and out-of-range policy.
5. Axis, transform direction, quaternion, angular-rate, velocity-transport, and epoch conventions.
6. Permitted simplifications and the mission envelope in which they are accepted.
7. Fixed-point ranges, resolutions, transition tolerances, and fail-closed behavior.

UTC may be an input/output representation but cannot be a discontinuous integration clock across a leap second. Exact-pole ENU frames must declare a reference meridian.

## Validation layers

1. **Portable deterministic KSA64 model:** the authoritative `GlobalEcef6DofV1` transition.
2. **Independent float64 model:** complete global atmosphere, gravity, frames, rigid-body dynamics, and transitions.
3. **SatKit fixtures:** preferred specialist evidence for time scales, Earth orientation, frame transforms, gravity, and selected ballistic/orbital coasts.
4. **Orekit fixtures:** used only for a documented SatKit gap or valuable independent comparison.
5. **GMAT fixtures:** occasional exoatmospheric and near-orbital corroboration.

External tools generate frozen offline fixtures only. Normal tests and CI require no network, live EOP/leap-second data, or installed external validator. Validators never co-propagate, correct, or replace production state.

## Required transition evidence

- Multiple epochs, including leap-second/EOP boundaries and explicit out-of-coverage failure.
- Equator, both sides of the date line, high altitude, near both poles, and exact poles.
- ENU/ECEF/ECI round trips for position, velocity, attitude, angular rate, and simulation time.
- Transition continuity for the same quantities; quaternion comparison uses physical rotation equivalence so `q` and `-q` agree.
- Separate reports for frame/time error, force/environment error, and integration accumulation.
- Fixture provenance containing tool/data versions and hashes, inputs, frame direction, epoch/time scales, Earth models, raw output, conversion procedure, tolerance rationale, and regeneration instructions.

## Deferred tracks carried forward

- Measured 6502-specific advanced-flight rewrite.
- C64 Ultimate acceleration and physical transport integration.
- Portable C64-world endpoint and deliberately long target runs.
- `SixAxisWrenchV1` for intentional translation in docking, station keeping, rendezvous, and propulsive landing.
- Dual-channel recovery avionics and physical user-port/ACIA/Ultimate Ethernet acceptance.

Phase 10 should begin with range/accuracy analysis and transform fixtures, not an integrated trajectory. A frame or time convention error must be impossible to mislabel as a force-model defect.
