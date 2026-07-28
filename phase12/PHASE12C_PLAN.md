# Phase 12C implementation contract

Status: implementation in progress.

Entry commit: `eb666cbaf3b8950218656a7ad7fe135b05385813`

Date: 2026-07-28

## Objective

Phase 12C adds a passive global engineering viewer for the complete accepted
KSA-G10R mission. One Rust-owned, renderer-neutral display model supplies
role-filtered ENU, ECEF, and GCRF state to Unreal and Babylon.js. Renderers own
only cameras, camera-relative origin shifts, visual interpolation, materials,
layouts, and interaction.

The frozen Phase 10 nominal mission is the coordinate and full-flight
reference. The Phase 11 GNSS-loss operations session remains the live
commanding reference. No physics, avionics, optimizer, campaign, canonical
format, C64 target, or accepted mission result changes in this phase.

## Deliberate Phase 12B.5 decoupling

The remaining physical Duet, Vita3K, and physical Vita qualification gates are
tracked as an independent Phase 12B.5 workstream. They do not gate Phase 12C
implementation or completion. Phase 12C may record opportunistic physical Duet
performance, but device product qualification remains Phase 12C.5.

This supersedes the sequencing sentence in `PHASE12C_HANDOFF.md`; it does not
reclassify Phase 12B.5 as complete or alter any Phase 12B.5 evidence.

## Frozen entry evidence

Phase 12C preserves:

- the 13-entry product catalog and SHA-256
  `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`;
- bridge ABI major 1, KPS1 version 1.0, and every accepted ABI/KPS1 layout;
- the 21,591-release, four-action GNSS-loss session;
- the exact 2,911,464-byte KSB11 and SHA-256
  `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`;
- the accepted Phase 10 KTT10, KPH10, and KSR10 nominal evidence;
- Phase 12B command-desk behavior, actions, role filtering, dispositions,
  exact/smooth modes, accessibility, and evidence finalization; and
- every accepted Phase 0-12B.5 regression artifact.

### Phase 10 nominal lineage audit

The frozen Phase 10 nominal artifacts and the current portable re-execution
have a small, pre-existing post-recovery fixed-point disagreement. Phase 12C
changes neither. Before nominal replay is exposed, a fail-closed audit strictly
validates the frozen hashes and records, requires the reviewed current hashes,
and checks exact event identity plus bounded physical deltas. The frozen
lineage supplies the labelled planned path; current re-execution supplies
separately identified exact-release SIM truth. See
PHASE12C_NOMINAL_COMPATIBILITY.md.

## Global display boundary

Add a noncanonical `GlobalDisplayV1` product containing:

- `GlobalDisplayDefinitionV1`;
- `GlobalDisplaySampleV1`;
- `GlobalDisplaySourcePoseV1`;
- `GlobalDisplayPathChunkV1`;
- `GlobalDisplayTransitionV1`; and
- `GlobalReplayIndexV1`.

The products carry exact release/time, segment, active frame, source and model
identity, validity, continuity and discontinuity state, geodetic state,
role-permitted source poses, frame transitions, path chunks, and replay
bookmarks. They never expose canonical K-record internals.

KPS1 remains version 1.0. New message kinds are sent only after negotiating a
`GLOBAL_DISPLAY_V1` capability. Existing clients receive no new records.
Bridge ABI-v1 remains unchanged; a size-tagged optional Global Display
function table is additive.

Rust owns WGS 84, ENU/ECEF/GCRF conversion, source identity, role filtering,
events, and final dispositions. Clients receive absolute fixed-point state and
perform only a tested renderer-axis mapping and camera-relative origin
subtraction.

## Source and role policy

- Onboard position and attitude form the solid operational vehicle pose.
- Ground estimates remain separate ghost markers and paths.
- Planned paths are labelled model products.
- Sources are never combined to fabricate a state.
- Truth records are absent from every non-SIM-Director transport.
- Nominal replay defaults to read-only SIM Director with truth hidden.
- GNSS-loss operations defaults to Guided Operator.
- Plan deviation is informational; only Rust dispositions classify outcomes.

## Temporal and replay policy

Exact state exists at every 32 Hz release. Smooth presentation may interpolate
only between samples with matching source, model, frame, segment, continuity,
and validity identities.

Frame/segment changes, deployment, attitude retirement, resets, invalidity,
source replacement, replay seeks, history gaps, completion, and abort all force
an exact snap.

Verified replay supports exact release seeking, stepping, event jumps, and
0.25x, 0.5x, 1x, 2x, 4x, 8x, 16x, and unpaced playback. Live sessions remain
forward-only.

Paths use deterministic event-preserving levels: an exact active window,
one-second whole-mission paths, and four-second overview paths.

## Presentation

Both Unreal and Babylon provide:

- hybrid mission-director, engineering split, and cinematic layouts;
- automatic, launch/local, chase, Earth-fixed, inertial, recovery, free, and
  true-scale inspection cameras;
- a procedural WGS 84 ellipsoid, grids, frame axes, anchors, atmosphere shell,
  schematic KSA-G10R, component/recovery indicators, and labelled paths;
- physically scaled geometry with a labelled locator and true-scale inset; and
- separate vehicle, mission, avionics, procedure, operator, and evidence
  dispositions.

The hybrid layout and active-frame auto-director are defaults. Manual camera
input suspends automatic direction until explicitly resumed.

Production imagery, terrain, vehicle art, Niagara, Lumen, Nanite, and final
quality-tier polish remain Phase 12E.

## Implementation gates

1. Freeze and document the baseline and Phase 12B.5 decoupling.
2. Implement portable display DTOs, capability negotiation, codecs, and the
   additive bridge interface.
3. Implement the Rust display publisher, frame products, roles, and fixtures.
4. Implement accepted nominal/GNSS sources, replay indexing, and path LODs.
5. Implement interpolation, discontinuities, axis mappings, and origin policy.
6. Implement and package the Unreal procedural global viewer.
7. Implement the Babylon WebGPU/WebGL2/2-D global viewer.
8. Integrate layouts, operations, actions, outcomes, and replay UX.
9. Prove cross-renderer semantic parity and bounded performance.
10. Complete frozen audits, documentation, completion evidence, and handoffs.

Each accepted gate receives its own commit.

## Completion boundary

Phase 12C is complete only when:

- all frozen artifacts remain exact;
- old KPS1 and ABI-v1 clients remain compatible;
- live and verified replay normalize to the same global-display trace;
- Unreal and Babylon agree on sources, frames, events, paths, selected release,
  and dispositions at every important event;
- no unauthorized truth bytes cross a boundary;
- no discontinuity is interpolated;
- renderer choices cannot alter authority or action ordering;
- packaged Win64/D3D12 works without Editor, MCP, or Python;
- Babylon works through WebGPU, WebGL2, and complete 2-D fallback; and
- the viewer never equates plan deviation with mission failure.
