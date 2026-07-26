# Phase 10 implementation plan

Status: complete. All twelve gates are accepted; see
[COMPLETION.md](COMPLETION.md).

Master seed: `0x4B5341A0`.

Accepted epoch: `2024-01-01T00:00:00 UTC`.

## Purpose

Phase 10 adds the separately versioned `GlobalEcef6DofV1` profile. It joins
local launch and recovery to rotating-Earth atmospheric flight and an inertial
exoatmospheric coast without altering any accepted Phase 0-9.5 artifact.

Only one KSA64 world owns an entity during an interval:

```text
LocalLaunch -> EcefAscent -> EciCoast -> EcefEntry -> LocalRecovery
```

Frame changes are deterministic world events committed at exact 32 Hz releases.
External programs generate offline validation fixtures only; they never
co-propagate, correct, or replace the production state.

## Locked Earth and time contract

- WGS 84 reference ellipsoid and ellipsoidal geodetic height.
- Central gravity plus J2 under `CentralJ2V1`.
- IERS Conventions 2010 with IAU 2006/2000A orientation under
  `Iers2010CompiledV1`.
- UTC, TAI, TT, and UT1 host input/output; elapsed TAI is the continuous
  integration clock.
- Pinned leap-second and Earth-orientation source files, content hashes,
  coverage windows, and fail-closed no-extrapolation policy.
- Host-compiled 60-second ECEF-to-GCRF knots containing a scalar-first
  Hamilton quaternion, angular velocity, and angular acceleration.
- The portable runtime parses no EOP files and evaluates no
  precession/nutation series.

The primary specialized fixture generator is SatKit 0.16.x. Orekit is used
only for a documented SatKit gap or useful independent transform check. GMAT
is reserved for bounded exoatmospheric corroboration. Normal tests require no
network, live Earth data, or installed external validator.

## Numeric and execution contract

- Position: signed Q12 kilometres.
- Velocity: signed Q24 kilometres/second.
- Acceleration: signed Q28 kilometres/second squared.
- Quaternion: signed Q30, scalar first, active body-to-current-frame.
- Angular rate: signed Q24 radians/second, physical body-relative-inertial
  rate expressed in body axes.
- Mission time: unsigned Q16 seconds, at least four hours.
- Existing kilogram/newton body, force, torque, effector, and recovery types.
- Existing Q18 Phase 9.5 pulse arithmetic, converted exactly at the boundary.

The accepted profile is bounded to 800 kg wet mass, -1 to 2,000 km altitude,
12 km/s, four hours, Mach 10, 15 degrees angle of attack, and 100 kPa dynamic
pressure. No model or numeric envelope is silently extrapolated.

Translation and attitude use midpoint RK2 with exact maximum steps:

- 1/128 second on the rail, under power, and during entry.
- 1/32 second during exoatmospheric coast and recovery.
- 1/256-second RCS valve edges.
- 32 Hz avionics releases.

## Frame ownership and transition order

The accepted KSA-G10R mission uses:

1. launch-site ENU through rail clearance;
2. ENU to ECEF at the first 32 Hz release after rail clearance;
3. ECEF to GCRF after ascending through 120 km with dynamic pressure below
   1 Pa;
4. GCRF to ECEF when descending through 120 km;
5. ECEF to the fixed recovery-anchor ENU below 20 km, below Mach 0.8, and
   within 200 km of that anchor.

The world qualifies a condition but commits ownership only at the next exact
release, before its sensor sample. Each transition records both frame
identities, exact Q16 time, Earth/transform identity, pre/post position,
velocity, attitude, angular rate, continuity deltas, and checksum. A failed
transform atomically rejects the successor. Exact-pole ENU uses a
mission-declared reference meridian.

## Environment and reference missions

`CompiledProfileV1` contains bounded altitude knots for density, pressure,
temperature, speed of sound, and ENU wind. The accepted profile is compiled
from the U.S. Standard Atmosphere 1976 through 200 km. Air co-rotates with
Earth plus local wind. Zero beyond coverage is permitted only when declared in
the pack.

ECEF propagation includes Coriolis, centrifugal, and frame-angular-
acceleration terms. Central-plus-J2 gravity is evaluated consistently in an
Earth-aligned frame and transformed when GCRF owns the state. A bounded
fixed-iteration WGS 84 geodetic conversion supplies latitude, longitude, and
ellipsoidal height.

The primary accepted vehicle is the assumption-backed fictional KSA-G10R:

- approximately 500 kg wet, 340 kg main propellant, and 5 kg RCS propellant;
- approximately 14 kN pressure-fed liquid engine for 60 seconds;
- approximately 8 m long and 0.4 m diameter;
- two-axis motor gimbal during powered flight and twelve-jet cold-gas RCS
  during coast;
- no accepted supersonic canard authority;
- drogue/main recovery after subsonic return;
- eastward launch from 28.5 N, 80.6 W, approximately 3 m ellipsoidal height;
- 200-300 km nominal apogee, 300-700 km downrange, and less than 45 minutes.

The secondary check exports the frozen KSA-5A insertion state through a
separately identified handoff fixture and propagates approximately one 200 km
orbit. It does not replace or alter the Phase 5 ascent.

## Global avionics

`GlobalFlightComputer` is additive. It retains the 32/8/1 Hz schedule and
receives transported measurements only:

- 32 Hz IMU, air data, actuator feedback, and active-frame identity;
- 8 Hz barometer, attitude aid, recovery, health, and frame service;
- 1 Hz ECEF GNSS position/velocity and mission guidance.

The navigator transforms its own estimate at a frame change using public frame
service data; it is never reset to truth. Guidance supplies local-vertical
ascent scheduling, inertial coast hold, entry attitude scheduling, and
measured-state recovery. Phase 9.5 gimbal/RCS allocation remains behind its
coordinate-neutral torque boundary.

## Additive formats

All records are fixed-capacity, little-endian, identity-bound, CRC-protected,
reserved-zero strict, and fail closed:

- `KEM10`: Earth, ellipsoid, gravity, epoch, leap/EOP policy.
- `KFT10`: compiled frame-transform knots.
- `KAT10`: compiled atmosphere and wind.
- `KGV10`: vehicle, propulsion, high-speed aero, effectors, recovery.
- `KGM10`: mission, guidance, anchors, transition conditions.
- `KLR10`: transported sensor, aid/frame, transition, command, status cells.
- `KTT10`: canonical global telemetry.
- `KSR10`: 512-byte global evaluation summary.
- `KPH10`: compact ground-track/altitude history.
- `KSC10`: deterministic campaign configuration.
- `KRA10`: append-only campaign archive.
- `KMR10`: noncanonical Mission Control recording.

The public evaluation boundary is:

```text
evaluate_global(
    earth,
    transforms,
    atmosphere,
    vehicle,
    mission,
    avionics,
    uncertainty_case
) -> GlobalEvaluationSummary
```

Placement, workers, presentation, storage, and REU capacity are excluded from
evaluation identity.

## Gates

1. Freeze identities, conventions, sources, envelope, format layout, and the
   Phase 0-9.5 compatibility baseline.
2. Prove numeric ranges, Q16 time, leap/EOP coverage, and native/MOS vectors.
3. Compile KEM10/KFT10 and freeze equator, dateline, altitude, pole,
   leap-boundary, EOP-boundary, and failure fixtures.
4. Implement ENU/ECEF/GCRF state transforms, exact ownership events, round
   trips, and atomic failure.
5. Implement central+J2 gravity, rotating-frame terms, geodetic conversion,
   compiled atmosphere/wind, and independent force snapshots.
6. Compile KSA-G10R vehicle and mission packs with assumption provenance and
   representability checks.
7. Compose the deterministic uninstrumented global mission.
8. Add frame-aware truth-blind avionics, KLR10, and checksum chains.
9. Add nominal/fault evidence and the complete independent float64 mission.
10. Add frozen KSA-5A one-orbit SatKit and optional GMAT corroboration.
11. Add 64/256 campaigns, F1-F7 Mission Control, reports, stock replay, and
    the externally paced stock-C64 flight endpoint.
12. Run the complete audit, record measurements and limitations, and write the
    next-phase handoff.

Each gate receives a commit only after its compatibility, exactness,
corruption, and independent-evidence checks pass.

## Campaign and target policy

Routine campaigns use 64 cases; completion uses 256. Run zero is nominal and
the seed is `0x4B5341A0`. Ordered archives must be byte-identical at one, four,
and eight workers.

Complete missions are accepted on host-world/host-flight. Stock-C64 flight is
an externally paced, no-REU KLF6/KLR10 endpoint. Finite VICE probes cover every
release class and transition. The endpoint need not be realtime, but size and
cycle cost are recorded.

Use one VICE instance, never enable warp, close it after success or proven
failure, and preserve the existing cooldown. A complete target mission starts
only after a fresh projection of at most 30 minutes and explicit confirmation.

## Completion boundary

Phase 10 preserves every Phase 0-9.5 artifact; proves leap/EOP failure policy;
meets declared frame, round-trip, transition, float64 trajectory, and KSA-5A
coast tolerances; completes KSA-G10R recovery; produces deterministic 256-case
evidence with at least 95 percent physical recovery; proves bounded host/C64
cell equality; and clearly separates numerical validation from real-vehicle,
certification, regulatory, or safety claims.

Runtime empirical atmosphere, higher-order gravity, thermal/ablation physics,
sustained orbit and precision entry, portable C64 global world,
6502-specialized rewrites, C64 Ultimate acceleration, realtime physical links,
six-axis guidance, rendezvous/docking, and multi-body flight remain deferred.
