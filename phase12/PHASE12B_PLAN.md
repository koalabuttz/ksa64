# Phase 12B: live mission operations and human-scale GNSS-loss exercise

Status: accepted implementation contract.

## Purpose

Phase 12B turns the accepted Phase 12A Unreal bridge into KSA64's first polished, bidirectional Mission Foundry operations experience. It runs the complete accepted KSA-G10R mission, injects a persistent GNSS receiver failure, and lets a Guided Operator observe, review, stage, validate, commit, cancel, or decline high-level operational actions.

Phase 12B adds no authoritative physics, alternate flight computer, direct effector control, or canonical record family. Rust remains the sole authority for mission state, role filtering, actions, outcome interpretation, and KSB11 evidence. Unreal owns presentation and wall-clock scheduling only.

The accepted Phase 11 nine-release GNSS-loss fixture remains byte-exact as a compatibility oracle. Its deliberately compressed command timing is not presented as a human operations exercise.

## Locked mission

Add a separately identified `FullMissionGnssLossV1` viewer session over the accepted Phase 10 KSA-G10R world and Phase 11 reference operations package.

- Execute launch through recovery. The untouched/no-action Phase 10 path remains 22,015 exact 32 Hz releases (687.96875 seconds); the accepted reference command transcript changes the later guided path and lands at release 21,591 (674.71875 seconds).
- Inject persistent GNSS receiver loss at T+180.000 seconds during GCRF coast.
- Qualify the loss from transported observations; neither the flight package nor ground operations may read private truth.
- Keep ground tracking available so the independent ground estimator can construct bounded navigation-update proposals.
- Use human-scale review and activation windows. The frozen reference transcript stages its first update at T+190 seconds and activates it at T+200 seconds, then stages its continuation decision at T+205 seconds and activates it at T+215 seconds.
- Require at least five seconds of activation lead. Rust regenerates a stale proposal from current operational data rather than allowing Unreal to edit it.
- Preserve autonomous onboard navigation, guidance, control, prediction, entry, and recovery when ground communication or operator action is absent.

Accepted operator paths include applying the ground update and continuing the primary mission, continuing inertially within declared residual limits, selecting conservative safe recovery, acting late, taking no action, and submitting an invalid action that is rejected without changing active state. Safe recovery preserves entry and recovery sequencing; it does not invoke a low-level safe state that disables recovery actuation.

## Operational authority and outcomes

Generalize the accepted global runner so it hosts a `GlobalKlr10FlightPackage` without changing the Phase 10 compatibility path:

```text
Phase 10 world
      | KLR10 observations
      v
Phase 11 flight package
      | validated commands
      v
Phase 10 world
      | public tracking observations
      v
ground estimate -> procedures -> predictions -> uplink broker -> KSB11
```

The generalized runner reproduces Phase 10 artifacts exactly when operational features are inactive. The ground estimator consumes delayed, deterministically noisy tracking observations and public frame data, never world state.

Mission-plan conformance is not synonymous with mission success. Add a truth-blind, noncanonical `OperationalDispositionViewV1` with independent axes:

| Axis | Classifications |
|---|---|
| Mission objective | primary, alternate, contingency, not achieved, indeterminate |
| Vehicle | nominal, degraded, recovered, safe-state, lost, unknown |
| Procedure | completed, alternate branch, skipped, mistimed, overridden, failed |
| Operator | timely reference, timely alternate, delayed valid, no action, rejected action |
| Avionics | nominal, degraded operational, safe recovery, failed |
| Evidence | complete, observation-incomplete, aborted, invalid, unavailable |

Overall wording is derived in this order: invalid or incomplete evidence is indeterminate; vehicle loss or unmet safety criteria is failure; declared contingency recovery is contingency success; a completed primary objective with faults or off-nominal procedure is degraded success; all nominal criteria is nominal success. A failed or mistimed procedure alone cannot turn a physically successful mission into failure.

## Additive application and bridge interfaces

Extend `LiveMissionSession` with role-filtered views for procedure guards and deadlines, GNSS/inertial state, available operational-link status, onboard and ground estimates, action proposals and receipts, semantic events, source-labelled prediction paths, exact release history, and final disposition. Explicit ground-communications blackout and reacquisition presentation remains attached to the separately validated Phase 11 scenario.

These views remain noncanonical. They reuse the accepted KTT10, KPH10, KSR10, KUL11, KUA11, KAL11, KDR11, and KSB11 owners rather than parsing or redefining their semantics in Unreal.

Preserve every Phase 12A ABI-v1 symbol and layout. Advance only the bridge build identity to `0x120B0001`, advertise additive feature bits, and add fixed-layout v1 presentation structures and exports for session start options; operational, procedure, action, transport, timeline, path, and disposition views; cursor-based history; command tickets and receipts; queue/worker state; nonblocking shutdown; Rust-side evidence verification; and deterministic presentation-text identities.

The original `ksa64_viewer_start` remains the compressed compatibility entry and must return the accepted 22,369-byte KSB11. No existing structure is widened or reinterpreted.

## Unreal runtime and presentation

Keep `FKsa64BridgeModule` as the qualified DLL loader. Add one `UKsa64LiveMissionSubsystem` as the sole Unreal-side bridge consumer. It owns command serialization, integer wall-clock release scheduling, polling, immutable view models, visual interpolation, atomic evidence output, and asynchronous lifecycle handling. Widgets never poll the bridge independently. Open the Rust live session in `Fast` execution-capacity mode so explicit bounded `Advance(n)` calls are honored; Unreal alone schedules realtime, pause, single-step, 4x, 16x, and maximum-fast wall-clock presentation. The internal capacity setting is noncanonical, emits no pace evidence, and must produce the same KSB11 for an identical release/action transcript.

Default to continuous realtime at 1x. Provide pause, resume, exact single-release step, 4x, 16x, and maximum-fast. Never skip releases or use floating-point release accumulation. Rendering stalls cause bounded catch-up or slower-than-realtime execution. Permit one outstanding lifecycle/action command, and never pause, submit, cancel, or change speed automatically.

Build a source-controlled C++/Slate command desk with mission/frame/package status; 2-D altitude/time and ground-track plots for planned, onboard, ground, and observed paths; procedure guards and timeouts; navigation residuals; explicit **Review -> Stage -> Validate -> Commit** controls; semantic timeline; outcome matrix; evidence verification; and an engineering diagnostics drawer.

Use a modern NASA-inspired operations language: dark navy and charcoal surfaces, cyan data, amber attention, red alarms, green accepted states, clear typography, and restrained C64-inspired dividers and identity labels. Do not use NASA insignia or imply endorsement. Convey state by text and icon as well as color; support keyboard focus, reduced motion, high contrast, 100/125/150 percent text, and 1280x720 through 2560x1440 layouts.

Smooth mode interpolates only presentation markers. Exact numbers, guards, receipts, validity, checksums, actions, and timeline entries always use exact release data. Frame changes, gaps, faults, actions, pauses, and completion snap exactly.

Phase 12B remains a 2-D operational presentation. Earth-scale 3-D rendering, vehicle pose, and local/global display-domain conversion remain Phase 12C.

## Implementation gates

1. Freeze Phase 12A bridge layouts, catalog bytes, harness, compressed session, and KSB11; run the Phase 0-12A audit.
2. Generalize the global runner and prove inactive-operation Phase 10 exactness.
3. Add the persistent GNSS fault, public ground tracking, human-scale procedure, safe recovery, and reference/alternate/fault cases.
4. Add role-filtered operational views, semantic timeline, predictions, presentation metadata, and disposition matrix in Rust.
5. Extend ABI v1 additively while retaining the old harness; add a full-mission harness.
6. Build the Unreal lifecycle/pacing/polling/history/evidence subsystem.
7. Build the accessible 2-D command desk and functional noncanonical alerts.
8. Complete the full mission through the UI, verify KSB11 through Rust, and replay the same transcript through the application facade.
9. Cover 30/60/144 Hz rendering, sparse/burst polling, mixed pacing, resizing, exact/smooth display, queue full, overflow, worker failure, abort, and shutdown. Target 60 fps at 1920x1080 with bridge work below 1 ms p99 and 2 ms maximum.
10. Run native/frozen audits, both C++ harnesses, Unreal semantic/screenshot tests, cook, packaged execution, and record the Phase 12C handoff.

## Acceptance criteria

Phase 12B is complete when every Phase 0-12A artifact and catalog byte remains unchanged; the original harness and compressed KSB11 remain exact; inactive operations reproduce Phase 10; scripted and equivalent UI transcripts produce byte-identical full-mission KSB11; alternate paths receive correct independent outcome classifications; identical action epochs remain exact across all presentation choices; invalid actions fail closed; Guided Operator receives no truth; incomplete/failed sessions cannot masquerade as complete; the packaged experience runs without Editor, MCP, Python, network, Starter Content, or NASA assets; and the worker shuts down cleanly.

## Deferred boundaries

- Phase 12C owns Earth-scale 3-D replay, ENU/ECEF/GCRF display domains, vehicle pose, entry, recovery, and cameras.
- Phase 12D owns graphical project, vehicle, and mission authoring plus GUI/headless compiler parity.
- Phase 12E owns production art, NASA-derived visual material, Niagara, quality tiers, voiced content, and production optimization.
- Phase 12B introduces no new physics, K-format, direct effector control, alternate simulator, additional operational role, C64 live placement, sidecar bridge, or hardware integration.
