# Phase 10 source inventory

Status: frozen contract inventory; normalized data snapshots and generated
fixture hashes are added at their implementation gates.

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
