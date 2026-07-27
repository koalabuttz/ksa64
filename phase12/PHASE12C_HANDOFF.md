# Phase 12C handoff — complete global engineering viewer

Status: ready to plan. Phase 12B is complete and accepted.

Date: 2026-07-27

## Accepted entry boundary

Phase 12C begins from an accepted 2-D, role-filtered live-operations product rather than an unfinished presentation prototype. Its frozen entry evidence is:

- source commit `423c116cf58632f344d4a48774a97a4487c34113`;
- bridge ABI major 1 and build identity `0x120B0001`;
- qualified bridge `ksa64_viewer_bridge-423c116cf586-120b0001.dll` with SHA-256 `da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565`;
- the unchanged 13-entry catalog with SHA-256 `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`;
- the 21,591-release, 674.71875-second four-action GNSS-loss session;
- the exact 2,911,464-byte KSB11 archive with SHA-256 `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`;
- 17/17 accepted Unreal operations tests;
- a standalone packaged D3D12 command desk; and
- accepted bounded bridge/presentation service timing of 258,900 ns p99 and 460,000 ns maximum.

The Phase 12B command desk, action flow, outcome axes, role filtering, exact/smooth behavior, and evidence finalization become regression fixtures. Phase 12C does not redesign them.

## Phase 12C owns

Phase 12C adds the passive global engineering viewer for the complete accepted Phase 10 mission:

- Earth-scale 3-D presentation;
- explicit local ENU, Earth-fixed ECEF, and inertial GCRF display domains;
- large-world/origin management that does not alter authoritative coordinates;
- vehicle position, attitude, angular-rate, and frame-owner presentation;
- launch, ascent, coast, entry, recovery, and landing visualization;
- planned, onboard-estimated, ground-estimated, and role-permitted truth trajectories;
- exact visual snapping at frame transitions and other discontinuities;
- smooth interpolation only between compatible exact samples;
- cameras and engineering overlays;
- replay and live observation through the same accepted application/bridge boundary; and
- renderer-specific performance and packaged-product evidence.

The 2-D operations desk remains available as an overlay or companion surface. The 3-D viewer is not a replacement for procedures or commanding.

## Authority rules

Rust remains the sole owner of simulation, avionics, mission operations, role filtering, actions, canonical evidence, and final dispositions.

Unreal may:

- consume typed role-filtered live streams;
- consume typed presentation paths and recordings supplied by Rust;
- convert accepted coordinates into renderer display coordinates;
- interpolate presentation between compatible exact samples;
- choose cameras, materials, labels, and quality settings; and
- submit existing high-level operator actions through the accepted bridge.

Unreal may not:

- propagate or correct authoritative physics;
- derive authoritative events from scene or collision state;
- let Chaos, animation, particles, or frame rate own vehicle motion;
- parse KSB11 or nested canonical K records;
- expose SIM Director truth to operational roles;
- infer absent telemetry or silently repair incomplete history;
- reclassify Phase 12B outcomes; or
- alter release, action, checksum, or evidence order.

Only one model owns an entity state at any interval. Renderer state is always disposable presentation state.

## Exact and smooth display contract

Exact samples own event epochs, frame transitions, action receipts, procedure state, prediction identity, and evidence checksums. Visual smoothing is allowed only inside a continuous interval whose endpoints share compatible frame and model identities.

At a discontinuity the viewer must snap to the exact successor sample. It must never interpolate across:

- ENU/ECEF/GCRF ownership changes;
- deployment or recovery-mode transitions;
- explicit state resets or invalidity boundaries;
- prediction-model or source-estimate changes; or
- missing/incomplete history.

Both exact and smooth views must remain visibly labelled. Smooth mode is a presentation aid, never an alternate trajectory.

## Roles and trajectory sources

The viewer must distinguish every path by source and validity:

- planned reference;
- onboard estimate / ground-propagated projection;
- ground estimate / ground projection;
- physical truth, available only to SIM Director; and
- stale, invalid, or terminal prediction state.

Role filtering happens in Rust before these products cross the bridge. Hiding a truth actor in Unreal is not an acceptable security or authority boundary.

## Entry acceptance expectations

A Phase 12C plan should include:

1. typed global display samples and path products over the existing additive ABI;
2. independent coordinate-conversion fixtures at equator, poles, dateline, altitude, and every accepted frame transition;
3. exact-versus-smooth replay parity;
4. live/replay identity for the same source evidence;
5. large-world continuity and origin-shift tests;
6. role-filtering and source-label tests;
7. event-snap and incomplete-history failure cases;
8. packaged D3D12 presentation and performance evidence; and
9. no regression to the accepted Phase 12B 2-D desk or KSB11 output.

## Deliberately deferred beyond 12C

Phase 12C does not own:

- vehicle or mission authoring and compiler parity, which remain Phase 12D;
- production art, NASA-derived assets, Niagara effects, broad quality tiers, and final packaged performance, which remain Phase 12E;
- new physics, atmosphere, gravity, avionics, or canonical formats;
- live C64 hardware-link acceptance, C64-world work, a 6502 rewrite, or Ultimate acceleration;
- certification, launch approval, regulatory acceptance, or safety authority.

See [PHASE12B_COMPLETION.md](PHASE12B_COMPLETION.md) and [phase12b-completion-audit.json](phase12b-completion-audit.json) for the accepted entry evidence.
