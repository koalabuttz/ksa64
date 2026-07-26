# Phase 12 application handoff

Phase 11.5 freezes the host product boundary that Mission Foundry should consume directly.

## Rust entrypoints

Phase 12 should depend on the host application modules, not spawn the CLI:

- `product::ProductCatalog` for deterministic experience, target, and historical discovery.
- `application::Ksa64Application` for orchestration.
- `application::ApplicationRequest` for mission, campaign, optimization, and evidence actions.
- `application::ApplicationOutcome` and `ApplicationDiagnostic` for structured results.
- Existing Phase 11 authoring, session, procedure, prediction, role, and debrief services for editor workflows.

The checked snapshot is `product-catalog-v1.json` with schema `ksa64.product-catalog.v1`. It is host product metadata, not canonical simulation evidence.

## Authority boundary

Mission Foundry may:

- enumerate and filter catalog entries;
- validate capability and placement choices;
- compile reviewable sources through existing compilers;
- launch accepted missions and workbenches through the facade;
- observe progress and canonical telemetry;
- open, replay, verify, and debrief evidence;
- construct explicit target build or live-probe requests.

It may not:

- parse console output as an API;
- spawn phase-numbered binaries for ordinary product actions;
- become a second simulator, flight computer, optimizer, or evidence parser;
- alter physics through rendering, editor state, role filtering, or pacing;
- start VICE, hardware, or a long target run without the same explicit boundary as the CLI.

## Stable identities

Use catalog IDs such as `ksa-g10r.operations` and `firestorm.spatial` in new user-facing state. Preserve serialized profile names, phase modules, K-format identities, artifact filenames, and historical hashes exactly.

Compatibility wrappers remain supported through at least Phase 13. Their existence is a migration aid, not the Phase 12 integration mechanism.

## Phase 12 starting point

The flagship open action is equivalent to:

```text
ApplicationRequest::Mission(
    id = "ksa-g10r.operations",
    scenario = "gnss-loss",
    role = "guided-operator",
    display = TUI,
    pace = Fast
)
```

Mission Foundry can then add visual source editing, maturity/provenance views, role-filtered operations, and passive 3-D playback while the accepted application services retain authority.
