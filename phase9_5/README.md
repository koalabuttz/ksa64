# Phase 9.5 — advanced control effectors

Phase 9.5 adds aerodynamic canards, cold-gas RCS, and deterministic mixed
control allocation to the accepted local-ENU vehicle and avionics system.

The phase is additive. Every Phase 0–9 format, evaluator, reference vehicle,
search, and frozen artifact remains unchanged.

## Accepted scope

- `Firestorm-C9`: four independently actuated aerodynamic canards.
- `Firestorm-R9`: twelve cold-gas jets with an ideal-isothermal blowdown
  supply.
- `Firestorm-M9`: motor gimbal, canards, and RCS under one priority-residual
  allocator.
- `KSA-X1`: a separately identified experimental vehicle using a regulated
  supply table. It demonstrates reuse but cannot enter an accepted physical
  Pareto front.
- Host/host, host-world/C64-flight, and C64-world/host-flight execution.
- Stock-C64 flight and world endpoints. No REU is required.

## Frozen boundaries

- The Phase 8.5 `LocalFlightComputer` remains byte-for-byte compatible.
- KLR8/KAT8/KAS8 and the Phase 9 optimization formats remain unchanged.
- The advanced wrapper consumes truth-blind measurements, invokes the frozen
  local kernel in monitor-only mode, adds roll demand, and allocates physical
  body torque.
- KSA64 remains the only runtime physical authority. Analytic and independent
  float64 models provide primary validation evidence.
- Full six-axis force-and-torque guidance is deferred as
  `SixAxisWrenchV1`.

See [PLAN.md](PLAN.md) for the implementation gates and
[CONTRACT.md](CONTRACT.md) for the frozen public behavior.

## Implemented gates

- Gate 1 froze the additive contract and proved the complete Phase 0–9 audit.
- Gate 2 froze the numeric and binary contracts and matched native and MOS vectors.
- Gate 3 adds the offline provenance-bearing compiler and four reconstructible reference pack sets under `phase9_5/examples/`. Static hardware is compiled into each derivative KVP8; KPE9 carries only active effector and supply behavior. `reference/verify_reference_packs.py` independently checks identities, CRCs, mass moments, hinge limits, and pack links.

Regenerate Gate 3 outputs with:

```powershell
cargo run -p ksa64-host --bin phase9_5_compile -- phase8/examples/firestorm54.kvp8 phase9_5/source-data/advanced-effectors-v1.json phase9_5/examples
python phase9_5/reference/verify_reference_packs.py
```
