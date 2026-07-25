# Phase 9 handoff: design optimization and robustness workbench

Phase 8 leaves a stable evaluator boundary for host-side optimization. Phase 9 should select candidates; it must not introduce a second production simulator or move search logic into the portable core.

## Frozen inputs available to Phase 9

- `ModelProfileId::{HobbyVerticalV1,HobbySpatialV1}` and unchanged legacy adapters.
- Compiled, identity-bound vehicle, motor, mission, and wind packs.
- `EvaluationSummary` with objective/constraint-ready metrics and explicit validity.
- Deterministic keyed uncertainty independent of worker count or catalog ordering.
- Ordered KSC8/KRA8 evidence, strict corruption rejection, stock/REU retention, and replay.
- Independent float64 and OpenRocket evidence for the Firestorm reference.

## Recommended Phase 9 sequence

1. Freeze design-variable, constraint-policy, optimizer-manifest, candidate-summary, Pareto-front, checkpoint, and archive contracts.
2. Add deterministic candidate lists and parameter grids over offline-compiled geometry and motor choices.
3. Add resumable host execution with ordered evaluation identities and content-addressed deduplication.
4. Add sensitivity analysis and robust objectives over nested deterministic uncertainty campaigns.
5. Add bounded evolutionary/Pareto search as a host-only strategy behind the same evaluator API.
6. Add rich host plots and stock/REU-scaled finalist browsing and replay.
7. Validate search determinism, resume behavior, archive corruption, worker-count independence, and finalist re-execution.

## Entry constraints

- Do not mutate KVP7–KRA8 or accepted Phase 0–8 artifacts.
- Geometry is compiled and validated before evaluation; no JSON or arbitrary allocation enters the portable path.
- Optimizers consume summaries and validity bits, never private truth or presentation state.
- A candidate identity binds source, compiler, model profile, mission, environment, uncertainty, and evaluator versions.
- Unsupported geometry or model conditions fail closed; search must not reward numerical faults.
- Large searches remain host-native. C64 execution is for bounded finalists, replay, and small demonstrations.
