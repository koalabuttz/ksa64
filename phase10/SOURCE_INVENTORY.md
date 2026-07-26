# Phase 10 source inventory

Status: complete. Normalized source snapshots, generation metadata, and
fixture hashes are frozen under `source-data`, `generated`, and `evidence`.

## Normative model sources

| Subject | Source | Use |
|---|---|---|
| Ellipsoid and geodesy | NGA WGS 84 | Ellipsoid constants, axes, flattening, geodetic height |
| Earth orientation | IERS Conventions 2010, Technical Note 36 | Earth orientation and ITRF/GCRF conventions |
| Precession/nutation | IAU 2006/2000A as specified by IERS 2010 | Host transform compilation |
| Atmosphere | U.S. Standard Atmosphere 1976 | Compiled density, pressure, temperature, sound-speed profile |
| Leap seconds | Pinned IERS leap-second bulletin/list snapshot | UTC/TAI conversion and coverage |
| Earth orientation | Pinned IERS EOP snapshot | UT1 and compiled orientation knots |

## External fixture generators

| Tool | Role | Runtime dependency |
|---|---|---|
| SatKit 0.16.x | Preferred time, frame, state transport, gravity, and orbit fixtures | No |
| Orekit | Documented transform-rate or coverage gap only | No |
| GMAT | Occasional exoatmospheric/orbital corroboration | No |

Every retained fixture must identify tool/version, inputs, epoch and time
scales, transform direction, source-data versions and hashes, Earth/gravity
configuration, raw output, conversion procedure, tolerances, and fixture hash.
Normal tests consume checked-in artifacts and require neither network access
nor an installed external validator.

## KSA-G10R assumptions

KSA-G10R is fictional. Geometry, propulsion, high-speed aerodynamics, RCS, and
recovery data are engineering assumptions with explicit provenance in the
source packs. They are not presented as validated hardware data. The compiler
must reject any required source value without provenance.

## Primary links

- <https://earth-info.nga.mil/?action=wgs84&dir=wgs84>
- <https://www.iers.org/IERS/EN/Publications/TechnicalNotes/tn36.html>
- <https://ntrs.nasa.gov/citations/19770009539>
- <https://satkit.dev/>
- <https://satkit.dev/api/frametransform/>
- <https://www.orekit.org/site-orekit-12.2/apidocs/org/orekit/frames/package-summary.html>

## Frozen source identities

`source-data/source-manifest.json` binds:

- IERS Bulletin C 67, upstream SHA-256
  `bf064f784512a6e364b818435ae55439d1029e47e8d39812eedd04bf7da8131d`;
- IERS finals2000A final Bulletin B columns, upstream SHA-256
  `22feeac3f99572368c5e81c9702e151988050a636ef888aaf7e41ac0d2c9f85e`;
- the normalized EOP window from 2023-12-31 through 2024-01-02 UTC;
- forbidden extrapolation and elapsed TAI integration.

The frame fixture manifest records SatKit 0.16.0 and satkit-data 0.9.0 wheel
hashes. KSA-G10R source values are explicitly assumption-backed. Normal audits
consume these checked-in files and never retrieve live Earth data.
