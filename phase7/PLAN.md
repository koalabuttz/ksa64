# Phase 7 implementation contract

Status: accepted plan.

Phase 7 adds a typed multi-profile evaluation boundary and proves it with a
separately versioned hobby/high-power vertical-flight model. It does not
rewrite, rescale, or reinterpret the accepted KSA-2A, KSA-5A, KSA-6R, or
Phase 0-6 artifacts.

## Profiles

The public profile identities are:

- `LegacyKsa2PlanarV1`
- `LegacyKsa5SpatialV1`
- `HobbyVerticalV1`

Host builds may dispatch among profiles. A C64 program links only the selected
profile. Legacy facade branches call their existing executors and convert only
the already-produced results into an additive evaluation summary.

## Hobby numeric and model boundary

The new profile uses SI units and generated, strong fixed-point storage types.
The range generator selects the greatest fractional precision that leaves at
least 25 percent signed-raw headroom for these envelopes:

| Quantity | Envelope |
|---|---:|
| Mission time | 0 to 4,096 s |
| Altitude | -1,000 to 150,000 m |
| Velocity | +/-2,500 m/s |
| Acceleration | +/-2,000 m/s^2 |
| Mass | 0 to 512 kg |
| Thrust and force | +/-100,000 N |
| Mass flow | 0 to 100 kg/s |
| Dynamic pressure | 0 to 8 MPa |
| Density | 0 to 2 kg/m^3 |
| Vehicle reference area | 0 to 4 m^2 |
| Recovery CdA | 0 to 128 m^2 |
| Mach | 0 to 8 |

The fixed mission cadence is 0.01 seconds while constrained or powered, 0.02
seconds during ballistic coast, and 0.05 seconds during recovery descent.
Semi-implicit Euler remains the portable exact integrator. Events commit at
step boundaries under documented successor-state crossing rules.

The environment uses a checked-in 250 m table derived from the 1976 U.S.
Standard Atmosphere through 86 km. Density is zero and Mach unavailable above
the table. Gravity uses the accepted spherical-Earth altitude equation.

## Published-data reference

The canonical configuration is the Giant Leap Firestorm 54 dual-deploy rocket
with an AeroTech I211W motor:

- Firestorm dimensions, dry weight, recovery sizes, and the recommended I211
  pairing come from the manufacturer page retrieved on 2026-07-24:
  `https://giantleaprocketry.com/products/firestorm-54-rocket-kit`.
- The executable motor curve, total mass, and propellant mass come only from
  the public-domain, TRA-test-derived I211W RASP file:
  `https://www.thrustcurve.org/simfiles/5f4294d20002e90000000035/`.
- Normalized sources, attribution, license, retrieval date, and checksums are
  committed so builds never require the network.

Declared KSA64 assumptions are a two-metre vertical rail, sea-level calm
conditions, constant body Cd 0.60, impulse-proportional propellant depletion,
ideal apogee-triggered drogue deployment, ideal main deployment while
descending through 200 m AGL, 0.5/1.0 second linear drogue/main inflation, and
canopy Cd 1.5.

This is a published-data reference configuration, not a flight-correlated or
certification-grade prediction.

## Formats

All records are strict little-endian, bind their complete input identity, use
the existing CRC-32 contract, require zero reserved bytes, and reject unknown
versions or enums.

| Magic | Contract |
|---|---|
| KVP7 | 512-byte vertical vehicle pack |
| KMP7 | 896-byte motor pack, at most 64 sampled knots |
| KMC7 | 256-byte mission/environment/launch/recovery pack |
| KST7 | 96-byte header plus 96-byte canonical frames |
| KSR7 | 192-byte evaluation summary with metric-validity mask |
| KSC7 | 512-byte uncertainty campaign, at most 16 distributions |
| KCL7 | ordered candidate-list manifest |
| KPH7 | 64-byte header plus 16-byte sparse plot points |
| KRA7 | append-only CRC-protected evaluation archive |

Human-authored JSON keeps physical decimals as strings. The C64 parses only
bounded compiled packs.

## Evaluation and campaign boundary

The evaluator returns physical metrics and validity bits, not a hardcoded
score. The hobby summary includes rail exit, burnout, apogee, extrema,
deployment, ground-contact, terminal, identity, event, fault, and checksum
evidence. Spatial, stability, drift, and orbital fields remain invalid.

Intentional design values, mission configuration, and sampled uncertainty are
separate types. Candidate grids materialize complete packs before evaluation.
The portable core never mutates a nominal pack into a design candidate.

Phase 4 keyed draws and ordered aggregation are reused with a new hobby
parameter catalogue. Run zero is nominal. Routine campaigns use 64 runs; the
frozen 1,024-run campaign uses seed `0x4b534137`.

## Target and acceptance policy

Stock C64 execution requires no REU. The target retains one KSR7 summary and an
approximately 2 KiB KPH7 plot and renders bounded post-run pages. Target jobs
run sequentially with at most one VICE instance, and harnesses close it after
success or demonstrated failure.

A complete target mission is required only if a measured projection is at
most 30 minutes. Otherwise exact arithmetic, phase, event, telemetry, display,
and timing probes are the target completion evidence. No run is canceled
merely because it is taking a long time.

Phase 7 excludes 3-D motion, wind, weathercocking, CG/CP/inertia derivation,
stability, staging, sensor-driven recovery avionics, advanced optimization,
regulatory decisions, and claims of flight correlation.
