# Phase 9 implementation contract

Status: accepted for implementation on 2026-07-25.

Phase 9 adds a deterministic host-side optimization workbench around the
accepted Phase 8.5 avionics-aware evaluator. It does not add a second simulator
or alter any Phase 0-8.5 artifact.

## Frozen decisions

- Canonical engines are lexicographic grid search, NSGA-II, and
  `DE/rand/1/bin`.
- Built-in searches are byte-exact across worker counts and checkpoint/resume.
- Feasible candidates dominate infeasible candidates; constraints are never
  hidden inside weighted penalties.
- Candidate evidence advances through fixed nominal, eight-case, and 64-case
  uncertainty tiers.
- Human JSON manifests compile to bounded KSA64 records. Worker count,
  presentation, and output paths are execution settings, not experiment
  identity.
- The accepted seed is `0x4b534139`.
- The original Firestorm remains immutable. Accepted design work uses a
  separately identified research derivative near the validated geometry.
- Broad-airframe search is experimental and cannot enter the accepted physical
  Pareto front.
- Production optimization is host-only. A stock C64 browses results and reruns
  selected finalists through the existing split host/C64 endpoint.

## Public records

| Record | Contract |
|---|---|
| KOM9 | 2,048-byte compiled optimization manifest |
| KDV9 | 256-byte canonical design vector |
| KOE9 | 512-byte robust candidate aggregate |
| KRA9 | segmented, append-only search archive |
| KPF9 | ordered Pareto-front export |
| KSN9 | sensitivity evidence |
| KFP9 | bounded stock-C64 finalist package |

KAS8 remains the per-case avionics result. KAT8, KST8, and KPH8 remain the
trajectory/history contracts.

## Accepted studies

1. Monitor-only Firestorm-derived geometry and recovery design.
2. Fictional two-axis-gimbal hardware and controller design.
3. A smaller coupled NSGA-II demonstration over both variable families.
4. An explicitly experimental broad-airframe demonstration.

The accepted balanced preset uses 17 by 17 grids, populations of 48 for 32
generations, and 32 full-campaign finalists per primary study. Quick and
routine presets are smaller but use identical algorithms and ordering.

## Completion boundary

Phase 9 must provide exact search traces, resumable archives, a streaming JSONL
evaluator, passive TUI and HTML reports, strict corruption handling, stock-C64
finalist browsing, and selected host/C64 reruns. Canards, RCS, mixed allocation,
Bayesian optimization, CMA-ES, global ECEF/ECI flight, and physical-link
acceptance remain deferred.

KSA64 is an engineering simulation, not launch approval, certification,
regulatory evidence, or safety authority.
