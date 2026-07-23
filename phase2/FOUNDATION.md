# Phase 2 numeric and data foundation

Status: accepted for the Phase 2 planar laboratory.

Phase 2 uses a rotating spherical Earth in the equatorial plane. The canonical truth representation is polar: radius, Earth-relative downrange angle, radial velocity, and inertial specific angular momentum. Tangential velocity is derived as `h / r`. This keeps the central-force coast compact and makes torque-free angular momentum an exact state invariant.

The generated `contract-v1.json` is the machine-readable source of truth for field formats, declared envelopes, widened intermediates, Earth constants, fixed-capacity limits, integer-square-root vectors, and the Q1.15 quarter-wave sine table. The contract retains Phase 1's explicit two-word arithmetic policy; compiler-provided MOS `u64` is not accepted.

## Locked model limits

- One rotating spherical Earth, equatorial prograde motion, zero wind, and a co-rotating atmosphere.
- Up to four attached stages, sixteen pitch knots, four aerodynamic tables, and sixteen Mach knots per table.
- A 0.125-second baseline step; every pitch, ignition, cutoff, and separation time is step-aligned.
- Radius from 6,376 through 8,379 km, component speeds through 16 km/s, and specific angular momentum through 120,000 km²/s.
- Point-mass forces only. Lift, attitude, sensors, throttling, and partial-step events are outside this contract.

## Source and target boundaries

Humans author exact decimal JSON under `phase2/examples/`. The host generator validates and packs it; the C64 never parses JSON. The future `KSC2` image and `KST2` stream are new versions. Phase 1 `KSC1` and `KST1` remain byte-identical regression artifacts.

Regenerate:

    python -B phase2/reference/generate_contract.py

Verify without writes:

    python -B phase2/reference/generate_contract.py --check
