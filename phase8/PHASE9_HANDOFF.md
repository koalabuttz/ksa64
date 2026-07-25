# Phase 9 handoff: design optimization and robustness workbench

Phase 8 leaves a stable physical evaluator, but Phase 8.5 now deliberately precedes optimization. Phase 9 should select candidates only after the shared avionics, frame-aware identity, and execution-placement boundary in [PHASE8_5_HANDOFF.md](PHASE8_5_HANDOFF.md) is accepted. It must not introduce a second production simulator or move search logic into the portable core.

## Phase 8 inputs already frozen

- Frozen profile identities 3 and 4, historically exposed as `HobbyVerticalV1` and `HobbySpatialV1`; Phase 8.5 will add the canonical public names `VerticalPointMassV1` and `LocalEnu6DofV1` without changing bytes.
- Compiled, identity-bound vehicle, motor, mission, and wind packs.
- `EvaluationSummary` with objective/constraint-ready metrics and explicit validity.
- Deterministic keyed uncertainty independent of worker count or catalog ordering.
- Ordered KSC8/KRA8 evidence, strict corruption rejection, stock/REU retention, and replay.
- Independent float64 and OpenRocket evidence for the Firestorm reference.

## Phase 8.5 prerequisites for optimization

Before freezing Phase 9 candidate identities, accept a versioned avionics identity, sensor/command contract, actuator-capability binding, coordinate-frame identity, host/VICE exactness evidence, and monolithic loopback placement. Phase 9 identities must include those inputs so later insertion of realistic avionics cannot silently change an already optimized design.

Phase 9 must treat actuator capabilities and control-allocation profiles as versioned evaluator inputs rather than baking the initial two-axis gimbal into optimizer logic. Its search, checkpoint, archive, and Pareto contracts must therefore accept future canard, RCS, and mixed-effector identities without reinterpretation. Phase 9.5 will add those physical models and use this workbench to size, tune, and robustly compare them.

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
- A candidate identity binds source, compiler, model profile, mission, environment, avionics, actuator capabilities, control allocation, uncertainty, and evaluator versions.
- Unsupported geometry or model conditions fail closed; search must not reward numerical faults.
- Large searches remain host-native. C64 execution is for bounded finalists, replay, and small demonstrations.
