# Phase 7 published-data reference

Retrieved 2026-07-24.

## Giant Leap Firestorm 54

Source: https://giantleaprocketry.com/products/firestorm-54-rocket-kit

The source supplies the dual-deployment length (74.5 in), outside diameter
(2.27 in), dry weight (74.5 oz), 36 in main, 18 in drogue, and I211 among the
recommended 38 mm motors. SI conversions are recorded in
`../source-data/firestorm54.json`.

Body Cd 0.60, canopy Cd 1.5, reference area, and resulting canopy CdA values
are KSA64 modeling assumptions, not manufacturer claims.

## AeroTech I211W

Source: https://www.thrustcurve.org/simfiles/5f4294d20002e90000000035/

Contributor: John Coker. The page marks the RASP data as public domain and
describes it as converted from 1999 Tripoli Motor Testing test-stand data. The
checked-in RASP text preserves the published knots, loaded mass, and propellant
mass. The canonical JSON adds the RASP-implied zero-thrust point at time zero.

No current manufacturer mass values are mixed into this motor pack.

## SHA-256 source evidence

- `aerotech-i211w.rasp`: `116bb74c400f3bba055f3ac24c783536ada32a05d6a525e2d200e3d71c404b0e`
- `aerotech-i211w.json`: `83e13f05d8dfd1c2882c83f319b331edc01cca7d65851e2f019e161795483eff`
- `firestorm54.json`: `d27df80844e199e60e881d4fbd957df7d75b3de20d993f446ebdc380fff0489f`
- `firestorm-i211-mission.json`: `5e71c3c30e06770472686a6534a7eade584f4b0c41982fff7188cc47583f0e64`

This fixture is suitable for repeatable software evaluation. It is not
flight-correlated, certification-grade, or a substitute for a validated launch
simulation and applicable safety review.
