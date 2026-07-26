# Phase 12 application handoff

Phase 11.5 freezes the host product boundary that Mission Foundry should consume directly.

## Rust entrypoints

Phase 12 should depend on the host application modules, not spawn the CLI:

- `workspace_model::AcceptedProductCatalog` and its unchanged `ProductCatalog` snapshot for deterministic built-in discovery.
- `workspace_model::ProjectWorkspace` for user-authored projects and their non-accepted validation lifecycle.
- `workspace_model::RecentSessions` for generated evidence and explicit accepted-product or authored-project origin.
- `application::Ksa64Application` for orchestration.
- the nested `application::ApplicationRequest` family for project, mission, campaign, optimization, evidence, target, and audit work.
- `ApplicationRequest::policy()` for conservative permission, cancellation, and explicit live-confirmation metadata.
- `application::ApplicationOutcome` and `ApplicationDiagnostic` for structured results.
- `phase11_live::LiveMissionSession` for release-by-release GNSS-loss operations, plus `MissionSessionSnapshot`, retained release telemetry, typed events/actions, lifecycle and pacing controls, and exact KSB11 finalization.
- `Ksa64Application::live_mission_capability` for discovering which scenarios expose a true incremental adapter without changing the frozen product-catalog bytes.
- Existing Phase 11 authoring, session, procedure, prediction, role, and debrief services for editor workflows.

The checked snapshot is `product-catalog-v1.json` with schema `ksa64.product-catalog.v1`. It is host product metadata, not canonical simulation evidence.

## Discovery domains

Mission Foundry may combine these domains visually, but it must not merge their identities:

```text
AcceptedProductCatalog   reviewed built-ins and accepted maturity
ProjectWorkspace         authored source and Draft-to-Reviewed validation
RecentSessions           derived operational evidence with explicit origin
```

A project may reference `GlobalEcef6DofV1` or an accepted product as a base. That reference does not grant accepted product maturity. Promotion into the accepted catalog requires a separately reviewed catalog change and evidence decision.

## Authority boundary

Mission Foundry may:

- enumerate and filter catalog entries;
- validate capability and placement choices;
- compile reviewable sources through existing compilers;
- launch accepted missions and workbenches through the facade;
- observe progress and canonical telemetry;
- open, replay, verify, and debrief evidence;
- construct explicit target build or live-probe requests after checking request policy.

It may not:

- parse console output as an API;
- spawn phase-numbered binaries for ordinary product actions;
- become a second simulator, flight computer, optimizer, or evidence parser;
- alter physics through rendering, editor state, role filtering, or pacing;
- start VICE, hardware, or a long target run without the same explicit boundary as the CLI;
- insert user-authored projects or generated sessions into the accepted product catalog.

## Stable identities

Use catalog IDs such as `ksa-g10r.operations` and `firestorm.spatial` in new user-facing state. Preserve serialized profile names, phase modules, K-format identities, artifact filenames, and historical hashes exactly.

Compatibility wrappers remain supported through at least Phase 13. Their existence is a migration aid, not the Phase 12 integration mechanism.

## Hard Phase 12 entry criterion

Mission Foundry must operate live missions through `LiveMissionSession`. It may not recreate an execution loop, advance flight packages directly, or present completed evidence as a live run. Synchronous experiences remain valid for evaluation and replay, but the GUI must label them accordingly.

The flagship startup is equivalent to:

```text
let capability = app.live_mission_capability("ksa-g10r.operations", "gnss-loss")?;
let mut session = app.start_mission(&request)?;

session.advance_one_release()?;
session.submit_operator_action(action)?;
session.pause()?;
session.resume()?;
let snapshot = session.snapshot();
let completed_ksb11 = session.finish()?;
```

Wall-clock timers, animation, maps, procedure forms, and role-specific presentation remain Phase 12 responsibilities. They schedule or display the typed session; they do not own simulation state. Given an identical ordered action transcript, scripted and interactive operation must finalize to byte-identical evidence.
