# Phase 11.5 pre-Phase 12 hardening

Status: accepted on 2026-07-26 after the Phase 11.5 completion baseline at commit `97cc5b1`.

This amendment incorporates the product-boundary review before Mission Foundry begins. It changes host organization and noncanonical application APIs only. No physical model, flight software, optimizer, K-format, catalog entry, accepted evidence, or C64 program changes.

## Accepted changes

- `Ksa64Application` remains the public facade, while project, mission, campaign, evidence, optimization, and target/audit adapters live in focused modules.
- `ApplicationRequest` now has nested Project, Mission, Campaign, Optimization, Evidence, Target, and Audit families.
- Every request exposes a conservative permission class, safe cancellation boundary, and explicit-confirmation requirement for GUI queueing and activity history.
- Typed facade methods remain available for compatibility; the unified CLI routes ordinary product actions through `execute`.
- `AcceptedProductCatalog`, `ProjectWorkspace`, and `RecentSessions` are separate types.
- User project validation states stop at `Reviewed`; they cannot acquire built-in `Accepted` maturity by referencing an accepted model profile.
- Authored project IDs cannot shadow accepted product IDs, and session origins are validated against their owning domain.
- Unknown binary evidence is described as opaque, sets `recognized_format: false`, and directs the caller to the owning strict parser.
- `LiveMissionSession` provides the Phase 11 GNSS-loss flagship with explicit Compiled, Ready, Running, Paused, Completed, and Aborted states; one-release and bounded advancement; typed operational snapshots, retained per-release telemetry, and events; pause/resume/single-step/real-time pacing; atomic operator actions; and deterministic KSB11 finalization.
- `Ksa64Application::live_mission_capability` and `start_mission` expose that live adapter directly. Synchronous evaluators fail closed instead of masquerading as live sessions.
- The Phase 11 operations console now consumes the live session. Guided actions stage and commit through the accepted uplink boundary; scripted operation uses the same public action path.

## Frozen behavior

- `ksa64.product-catalog.v1` remains byte-identical with SHA-256 `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`.
- The 13 current experiences, seven targets, historical tier, commands, compatibility wrappers, session evidence, and safety boundaries are unchanged.
- Project/source compilation, simulation execution, campaign ordering, optimization, evidence parsing, target automation, and C64 behavior remain owned by their accepted modules.

## Validation

The hardening gate requires:

- formatting and Clippy with warnings denied;
- the complete host suite and Phase 11.5 CLI parity tests;
- live-session lifecycle, pacing, telemetry/event snapshot, action-boundary, and facade-capability tests;
- byte-identical KSB11 finalization for scripted and manually submitted copies of the same action transcript;
- request-policy tests covering read-only, artifact-writing, external-process, and live-target work;
- type-boundary tests rejecting accepted-ID shadowing and unknown base products;
- session-origin validation for accepted products and authored projects;
- opaque evidence wording and strict-parser referral tests;
- unchanged catalog snapshot hash; and
- the Phase 11.5 completion audit without live VICE, since no target code changed.

Machine-readable results are recorded in `hardening-audit.json`.
