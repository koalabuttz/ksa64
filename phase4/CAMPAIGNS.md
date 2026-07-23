# Phase 4 campaigns and summaries

## Parameter application

A `RunSpec` is converted into explicit world, sensor, and actuator parameters. Existing Phase 3 constructors and mission entry points remain unchanged.

- World parameters vary payload mass, each stage thrust independently, atmospheric density, and aerodynamic reference scaling.
- Sensor parameters add fixed biases to accelerometer, gyro, altimeter, GPS position, and GPS velocity channels, and scale every noise amplitude consistently.
- Actuator parameters select a two-to-six-step lag response and scale the frozen slew limit.
- Guidance gains, sequencing, vehicle topology, and fault topology are not parameterized.

Run zero uses the Phase 3 sensor seed and zero parameter deltas. Its terminal truth, state, and all four checksum chains are exactly the accepted Phase 3 nominal values.

## KSR4

Each completed run produces one fixed 128-byte KSR4 record. It carries:

- campaign, scenario, run, derived-seed, and variation identities;
- classified outcome;
- terminal and cutoff raw states;
- maximum dynamic pressure and proper acceleration;
- cutoff navigation position and velocity errors;
- truth, sensor, navigation, and flight checksum chains;
- terminal flight mode, alarm state, active stage, and CRC-32.

Reserved bytes must be zero. Unknown enums, wrong length/version/contract, and any corruption are rejected.

The orbit classification is useful for selection and display only. Phase 4 host analysis will recompute authoritative float64 orbital results from the raw cutoff state.

## Streaming aggregate

`CampaignAggregate` is allocation-free. It folds KSR4 summaries strictly in run-index order and retains:

- outcome counts;
- integer minima, maxima, Q16 means, and sample variance;
- a 16-bin insertion-altitude histogram;
- an ordered FNV summary chain over canonical KSR4 bytes.

Presentation statistics use the Phase 2 contract’s native units: cutoff altitude in kilometres, dynamic pressure in kilopascals, proper acceleration in metres per second squared, and navigation position error in metres. Canonical KSR4 values remain raw fixed point.

## Frozen 64-run smoke campaign

The reviewed smoke campaign uses master seed `0x4b534134`, 64 runs, and KSC4 CRC `0x3ad7ff88`. Two consecutive native executions produce exactly:

| Evidence | Frozen value |
|---|---:|
| stable / suborbital / impact / escape / abort / error | 55 / 9 / 0 / 0 / 0 / 0 |
| cutoff altitude range | 180–192 km |
| cutoff altitude Q16 mean | 12,321,787 |
| cutoff altitude sample variance | 6 km² |
| maximum dynamic-pressure range | 40–43 kPa |
| maximum proper-acceleration range | 54–55 m/s² |
| navigation position-error range | 1–47 m |
| ordered KSR4 summary chain | 586,068,286 |

The nine suborbital insertions are expected sampled outcomes, not acceptance failures. The campaign’s purpose is to expose sensitivity while preserving deterministic execution.

The pinned rust-mos acceptance probe validates the reviewed catalog, zero-run parameter mapping, streaming mean/variance, and KSR4 round-trip on `mos-sim-none`. It does not launch a long target campaign.