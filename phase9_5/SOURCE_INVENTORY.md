# Phase 9.5 source inventory

## Frozen implementation boundaries

- `core/src/phase8_5_contract.rs`: KAP8/KAC8/KLE8/KAS8 identities and codecs.
- `interface/src/phase8_5.rs`: KLR8 and KAT8 cells and telemetry.
- `flight/src/phase8_5.rs`: truth-blind `LocalFlightComputer`.
- `sim/src/phase8_5.rs`: exact event clock and accepted local evaluator.
- `host/src/phase9*.rs`: deterministic search, archive, protocol, reports, and TUI.
- `core/src/phase8_pack.rs` and `core/src/phase8_mission/`: accepted Firestorm spatial physical model.

These files may receive additive imports or compatibility aliases only where the
new API requires them. Existing encodings, identities, reference fixtures, and
execution entry points must not change.

## Additive implementation homes

- `core/src/phase9_5_contract.rs`: identities, numeric wrappers, KPE9/KPA9/KLE9/KAS9/KSC9.
- `interface/src/phase9_5.rs`: KLR9 and KAT9.
- `core/src/phase9_5_finalist.rs`: allocation-free KFE9 presentation reader.
- `flight/src/phase9_5.rs`: advanced truth-blind wrapper, roll demand, pitot fallback, allocator.
- `sim/src/phase9_5.rs`: canard/RCS physics, exact valve edges, evaluator, telemetry.
- `sim/src/phase9_5_bootstrap.rs` and `sim/src/bin/phase9_5_finalist_flight_endpoint_c64.rs`: strict KFB9 startup and separate stock selected-finalist flight endpoint.
- `host/src/phase9_5*.rs`: compiler, references, campaign/search integration, Mission Control.
- `phase9_5/reference/`: independent analytic/float64 evidence.
- `phase9_5/evidence/`: frozen generated packs, vectors, campaigns, searches, and reports.

## Source policy

Canard reference geometry and actuator values are declared assumptions. RCS
installation, tank, and supply curves carry explicit source/provenance identity.
Basilisk is optional secondary fixture generation only; normal tests and CI stay
offline and do not depend on it.


## Completion gate

- `complete.ps1`: bounded Phase 0–9 compatibility, native, independent, accepted-workbench, MOS, and optional sequential VICE audit.
- `completion-audit.json`: machine-readable accepted measurements, hashes, target boundaries, and limitations.
- `COMPLETION.md`: human-readable Phase 9.5 closure.
- `PHASE10_HANDOFF.md`: authoritative global-frame/time and validator boundary for the next phase.

The completion gate introduces no runtime dependency and starts no complete target mission.
