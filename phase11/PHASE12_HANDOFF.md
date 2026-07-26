# Phase 12 handoff: Mission Foundry

Phase 11 freezes the headless operational contracts that Phase 12 may present,
author, compile, and replay. Phase 12 should improve accessibility and visual
understanding without becoming a second simulator or silently changing
identity, authority, or evidence rules.

## Available foundations

- Strict vehicle, mission, environment, avionics, effector, flight-package,
  plan, procedure, fault, action, prediction, telemetry, and session records.
- A headless JSON compiler and
  `lint/compile/inspect/run/script/replay/debrief/verify` workflow.
- Explicit flight-software package/ABI compatibility and resource evidence.
- Separate world, onboard, ground-operations, and SIM Director authority lanes.
- Role-filtered operational views with truth structurally absent outside SIM
  Director.
- Deterministic session/action identities and exact replay.
- Passive host Mission Control and canonical trajectory/prediction products.
- Host-world/host-flight and externally paced host-world/C64-flight placements.

## Recommended Phase 12 scope

1. Build a host-only vehicle integration editor over the existing compiler,
   with component trees, attachment nodes, symmetry, procedural geometry,
   staging, internal placement, and cutaway views.
2. Show live mass, CG, inertia, CP, static margin, authority, propellant,
   connection, provenance, representability, and model-envelope evidence.
3. Provide separate mission lanes for world events, flight-software decisions,
   ground operations, and SIM Director faults.
4. Add an avionics lab that binds packages, sensors, capabilities, release
   budgets, targets, plans, procedures, and safe states.
5. Add Sketch, Evaluated, and Frozen Candidate maturity states. Only immutable
   compiled packs may identify accepted evidence.
6. Build a passive 3-D viewer consuming canonical telemetry and prediction
   products, with continuous local-to-global scale and selectable truth,
   onboard-estimate, ground-estimate, planned, and predicted paths according to
   the active role.
7. Prove GUI and headless compilation of the same source produce byte-identical
   packs and derivation ledgers.

## Boundaries Phase 12 must preserve

- The portable evaluator remains the only simulation authority.
- GUI state, camera, render rate, hints, and role presentation cannot alter a
  run except through explicit recorded public actions.
- Editable source is not canonical evidence; compilation creates a new
  immutable identity.
- The C64 consumes compiled packs and replay products, not GUI project files.
- No hidden automatic wiring of mechanical, propellant, power, data, or control
  connections.
- No arbitrary plugin execution, universal CAD/CFD/FEA, or unrecorded manual
  control path.
- Any visual flight-program authoring must compile into a declared Phase 11 ABI
  with deterministic target resource evidence.

## Parallel target-engineering track

Do not let Mission Foundry block the separately documented C64 priorities:

- physical link and bank-loader acceptance;
- measured 6502-specific flight-package work;
- C64 Ultimate RAM/acceleration integration;
- restoration of the portable C64-world role; and
- later REU overlay/handover research without pretending one CPU is redundant
  hardware.

## Following physical mission

After Mission Foundry proves these authoring and replay boundaries, select one
concrete sustained Earth-orbital spacecraft mission. Let that mission—not a
generic framework ambition—decide whether Phase 13 needs bounded electrical,
thermal, communications, propulsion, degradation, rendezvous, docking,
deorbit, or `SixAxisWrenchV1` work.
