# Phase 9.5 contract

## Identity and format rules

Phase 9.5 adds, without reinterpreting any earlier identity:

- `AdvancedEffectorSetId`
- `RcsSupplySourceId`
- `ControlAllocatorId::PriorityResidualV1`
- `KPE9` (2,048 bytes), including four Q24 hinge-load limits in the fixed header
- `KPA9` (512 bytes)
- `KLE9` (256 bytes)
- `KLR9` (64-byte fast sensor, 64-byte command, 64-byte aid, 80-byte status)
- `KAT9` (128-byte header, 320-byte frames)
- `KAS9` (512 bytes)
- `KSC9` (512 bytes)
- segmented `KAE9`
- bounded `KFE9`

All records are fixed-capacity, little-endian, identity-bound, CRC-protected,
strict about reserved-zero bytes, and fail closed on truncation or corruption.
KLF6 remains the outer split-transport frame.

## Time and command semantics

- The avionics release clock remains exactly 32 Hz in Q18 mission time.
- An RCS pulse quantum is exactly 1/256 second, or 1,024 Q18 raw units.
- A jet request contains zero through eight pulse quanta.
- Commands first affect the successor release interval.
- Continuous commands may be held for two missing epochs and safe on the
  third. RCS pulse and recovery commands are one-shot and are never replayed.
- Valve opening, closing, and exact propellant depletion are world split
  points. Simultaneous equal-time edges introduce one split.

## Physical authority

Canard forces, hinge loads, drag, and moments are incremental to the neutral
vehicle pack. RCS forces are applied per jet; nominal pairing never suppresses
residual translation caused by mismatch or failure. Remaining propellant
updates mass, centre of gravity, and diagonal inertia continuously.

The accepted canard envelope is:

- Mach at most 0.8
- vehicle angle of attack at most 15 degrees
- local canard incidence at most 15 degrees
- dynamic pressure at most 20 kPa

Operation outside a declared physical or numeric envelope fails closed.

## Allocation

`PriorityResidualV1` consumes a physical Q12 roll/pitch/yaw torque demand.
It applies vehicle-compiled authority and mixing tables in declared group
order, predicts achieved torque after quantization and saturation, and passes
the exact residual to the next group.

Reference ordering is:

- rail: all attitude effectors inhibited
- powered: gimbal, canards, RCS
- post-burnout with aerodynamic authority: canards, RCS
- low dynamic pressure: RCS
- recovery: all attitude effectors safe before attitude retirement

Canard authority begins at 300 Pa, reaches full authority at 2,000 Pa, and
disables below 200 Pa with hysteresis. Normal RCS allocation stops at the
declared 20 percent reserve.

## Execution and evidence

Execution placement, recording, presentation, worker count, storage, and REU
capacity are excluded from evaluation identity. Host/host and both split
directions must produce identical sensor, navigation, demand, allocation,
actuator, command, and physical checksum chains.

The stock advanced flight endpoint must fit below `$C000` and its worst release
must remain within 24,631 PAL cycles. The separate stock world endpoint must
fit below `$C000` but may run slower than simulated real time.

The impossible combined stock world-plus-avionics image remains closed.

## Deferred contract

`SixAxisWrenchV1` will add deliberate translational-force guidance for docking,
station keeping, rendezvous, and propulsive landing. Phase 9.5 applies every
individual RCS force physically, but guidance commands attitude torque only.
