# Phase 8 implementation contract

Status: accepted implementation plan.

Phase 8 adds a separately versioned `HobbySpatialV1` profile while preserving
every accepted Phase 0-7 entry point and artifact. `HobbyVerticalV1` remains the
small, inexpensive hobby evaluation profile.

## Reference mission

The reference remains the Giant Leap Firestorm 54 with an AeroTech I211W.
Source values distinguish published, measured, declared-assumption, and
derived provenance. The current manufacturer dimensions, recovery sizes,
instructions, fin-can information, and RockSim data are preferred. Missing
values may be explicit assumptions but may not be disguised as derived truth.

## Spatial contract

- Launch-site local ENU: +X east, +Y north, +Z up.
- Body +X points through the nose; +Y is vehicle-right; +Z completes the
  right-handed frame.
- Scalar-first Hamilton quaternions actively rotate body vectors into ENU.
- Full six-degree-of-freedom propagation runs through first recovery
  deployment. Recovery then retains continuous ENU position and velocity while
  retiring attitude and using a three-dimensional point-mass canopy model.
- Powered translation and attitude use 0.01-second steps. Coast translation
  uses 0.02-second steps with two 0.01-second attitude substeps. Recovery uses
  0.05-second steps.
- Layered mean wind is linearly interpolated in altitude. Bounded deterministic
  gust targets are keyed by wind identity, case seed, gust epoch, and axis.
- The reviewed aerodynamic envelope is Mach <= 0.8 and angle of attack <= 15
  degrees. Escape is an explicit model-envelope outcome, never extrapolation.

## Models

The host compiler accepts reviewable component geometry and emits bounded
packs. It derives wet/dry centre of gravity, diagonal inertia, reference area,
Barrowman-compatible centre of pressure and normal-force slopes, damping,
static margin, rail-guide geometry, and reviewed axial Mach/Cd tables.

Rail motion is constrained to the configured rail axis until the aftmost guide
clears. After release, thrust acts along body +X; drag opposes air-relative
velocity; normal force acts at centre of pressure about the moving centre of
gravity. Drogue deploys at the first descending state after apogee and main
deploys at the configured descending AGL threshold.

## Versioned evidence

Phase 8 owns strict KVP8, KMP8, KMC8, KWP8, KST8, KSR8, KPH8, KSC8, and KRA8
families. All use fixed bounded fields, little-endian encoding, reserved-byte
enforcement, identity binding, and CRC protection. Phase 7 formats remain
unchanged.

The accepted reference evidence consists of exact portable fixed-point
execution, an independent float64 implementation, aligned OpenRocket 24.12
calm and steady-wind comparisons, a qualified public-flight-data investigation,
a 64-run routine campaign, and a frozen 1,024-run campaign using seed
`0x4b534138`.

OpenRocket or historical-flight disagreement may not be hidden by coefficient
tuning. It must result in a source/model correction or documented rescoping.

## Target policy

No REU is required. More storage may retain more summaries and histories but
cannot alter physics. A full C64 mission starts only after a fresh finite PAL
projection of at most 30 minutes and explicit user confirmation. At most one
VICE instance may run, and it is closed after success or proven failure.

## Exclusions

Phase 8 excludes staging, clustering, active control, deployment avionics,
suspended-body recovery, flexible dynamics, fin flutter, CFD, FEA,
