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
- Host/host and host-world/C64-flight execution, with externally paced stock-C64 flight accepted as the interim hardware baseline.
- The portable C64 world and realtime C64 flight remain priority follow-on tracks; neither is silently replaced by host physics or an REU requirement.

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
- Gate 4 implements four independently actuated canards with incremental force, torque, induced drag, hinge-load limiting, lag/slew/saturation, and fail-closed aerodynamic envelopes. Native exact vectors match an independent float64 model within 0.213%; the three-vector stock-C64 VICE probe passes without an REU.
- Gate 5 implements twelve individual cold-gas jets, exact 1/256-second valve edges, one-shot pulse scheduling and accumulation, shared regulated/blowdown supply interpolation, exact depletion, residual translation, and changing propellant mass properties. Native exact vectors agree with independent float64 torque and mass-flow results well within 0.5%; the three-vector stock-C64 VICE probe passes without an REU.
- Gate 6 adds a truth-blind advanced wrapper around the frozen local flight computer, three-axis torque demand with roll control, deterministic 32 Hz pitot sensing, conservative navigation/wind fallback, bounded link-loss behavior, and independent checksum chains. Native and stock-C64 probes produce the same `0x8c165977` signature; the C64 probe is 18,700 bytes and requires no REU.
- Gate 7 implements `PriorityResidualV1` with pack-bound full-scale gimbal, orthogonal cruciform-canard, and bidirectional RCS mixing. It records requested, achieved, and exact residual torque; performs deterministic dynamic-pressure, reserve, health, rail, recovery, and safe-state handoffs; and never replays one-shot pulses. Four independent exact vectors produce signature `0xc262ed33` natively and in a 30,438-byte stock-C64 VICE probe.
- Gate 8 composes the advanced effectors with the exact 32 Hz event clock and the unchanged Phase 8 world through a new additive execution path. C9, R9, and M9 complete full missions; the accepted layered 5 m/s C9 case passes the 3 degree check; R9 passes the three-axis disturbance and reserve gates; M9 survives deterministic pitot loss; and strict KAT9/KAS9 evidence is independently checked. See [INTEGRATED_EVIDENCE.md](INTEGRATED_EVIDENCE.md).
- Gate 9 generalizes the Phase 9 workbench over KAS9 evidence and freezes accepted canard, RCS, mixed-effector, and experimental research studies. Seven grid/NSGA-II studies perform 2,974 unique robust evaluations; accepted finalists pass their 64-case promotion tier, archives are byte-identical with one, four, and eight workers, and the separately labelled KSA-X1 study promotes no accepted physical finalist.
- Gate 10 records the stock-target boundary without weakening the contract. The portable world endpoint remains too large and the advanced flight release exceeds the PAL realtime budget, but the exact stock flight endpoint fits without an REU. The accepted interim baseline is host world plus externally paced C64 flight: an eight-release one-instance VICE probe shadow-verifies every KLR9 command/status cell and the truth, navigation, flight, and allocator chains.

Regenerate Gate 3 outputs with:

```powershell
cargo run -p ksa64-host --bin phase9_5_compile -- phase8/examples/firestorm54.kvp8 phase9_5/source-data/advanced-effectors-v1.json phase9_5/examples
python phase9_5/reference/verify_reference_packs.py
python phase9_5/reference/generate_canard_vectors.py --check --report
python phase9_5/reference/generate_rcs_vectors.py --check --report
python phase9_5/reference/generate_allocator_vectors.py --check --report
```
