# Phase 12D handoff — Mission Foundry authoring and compiler parity

Status: **active planning boundary; Phase 12C display/session contracts accepted**.

Date: 2026-07-28

Phase 12D turns the accepted host application, live-session, evidence, and global-display services into a graphical vehicle and mission authoring workflow. It does not create another simulator and does not promote a user project into the accepted product catalog.

Phase 12C accepted the renderer-neutral boundary against source commit
`64d72f2a4ee0848bf7ff73c345fcd1cf56579ba1`. Its strict
`ksa64.phase12c.cross-renderer-evidence.v2` record has SHA-256
`c869a5dbc341ea6b5272e901882fe803dd2e15f1ab49cbeff48788527c01e50e`
and proves exact nominal and guided replay, role-filtered source/path products,
all four frame transitions, nine nominal milestones, six guided action/fault
milestones, and passive Unreal/Babylon parity.

## Required entry contracts

Phase 12D consumes, without redefining:

- the 13-entry accepted `ProductCatalog` and the hard separation among accepted products, `ProjectWorkspace`, and `RecentSessions`;
- `Ksa64Application` and typed application requests rather than CLI text or phase binaries;
- `LiveMissionSession` for ready/running/paused/completed/aborted lifecycle, exact release advancement, actions, snapshots, and KSB11 finalization;
- the Phase 11 source schemas, provenance ledger, headless `lint`, `compile`, `run`, `script`, `replay`, `debrief`, and `verify` services;
- role-filtered KPS1 and `GlobalDisplayV1` presentation products, including exact replay, source labels, frame transitions, and multidimensional disposition; and
- the accepted Phase 12C renderer-authority rule: Rust owns frames, events, source validity, role filtering, mission outcomes, and evidence; Unreal and Babylon remain passive views.

The carried physical Duet, Vita3K, and physical Vita qualification work remains
open under Phase 12C.5 product acceptance. It does not block Phase 12D
planning, authoring architecture, compiler parity, or desktop authoring
evidence, and Phase 12D may not silently claim those portable-device lanes.

## Phase 12D owns

- a bounded component-tree and attachment editor for supported vehicle profiles;
- vehicle, mission, environment, avionics, flight-package, procedure, fault, and authority-lane binding through existing source schemas;
- explicit `Sketch`, `Evaluated`, and `Frozen Candidate` maturity states;
- live engineering overlays derived by the owning Rust compiler, including mass properties, stability, authority, representability, source provenance, and model-envelope warnings;
- structured validation diagnostics that map cleanly between the graphical editor and headless tools;
- deterministic project save/load, migration, identity, and provenance behavior;
- GUI access to the same compile, run, replay, evidence, and debrief services used by the headless SDK;
- preview sessions that are visibly noncanonical until compiled and run through the accepted authority; and
- byte-identical GUI/headless compilation and derivation-ledger evidence for the same source project.

## Required authority boundaries

- A graphical edit creates a new source identity; it never mutates a compiled pack, accepted artifact, completed session, or accepted catalog entry.
- Unreal widgets, Blueprint, Babylon, and editor scene objects never calculate authoritative geometry, mass, aerodynamics, frames, events, constraints, or mission disposition.
- A preview may display Rust-derived engineering results, but only strict compiled packs may enter an authoritative evaluator or mission session.
- User-authored projects remain distinct from accepted KSA64 experiences even when they reuse `GlobalEcef6DofV1`, KSA-G10R components, or an accepted flight package.
- GUI convenience defaults are explicit source values with provenance; no hidden auto-wiring may invent mechanical, propellant, power, data, or control connections.
- A renderer may use local origins and compatible-sample interpolation only for presentation. Those choices never enter project, pack, run, or evidence identity.

## Compiler-parity acceptance direction

For a reviewed fixture set, graphical and headless workflows must produce the same:

- normalized source project;
- compiler diagnostics and validation outcome;
- compiled pack bytes and identities;
- derivation/provenance ledger;
- mission-session definition;
- exact run evidence when given the same ordered actions; and
- replay-visible `GlobalDisplayV1` semantic state, including complete source
  and path identities, raw stale/incomplete/terminal/resynchronization flags,
  exact path checksums, and the normalized supported camera/display-frame view
  mode.

A project that is incomplete, unrepresentable, outside a model envelope, missing provenance, or incompatible with its selected flight package fails closed before execution. A saved draft cannot masquerade as a frozen candidate.

## Phase 12D does not own

- new physics, avionics, optimizers, canonical evidence formats, or C64 programs;
- arbitrary CAD, CFD, FEA, scripting, dynamic plug-ins, or unbounded component graphs;
- public marketplace/catalog publication or automatic promotion of user work;
- production meshes, NASA imagery, terrain, Niagara effects, cinematic lighting, installers, signing, or store distribution, which remain Phase 12E;
- browser/mobile/Vita product qualification, which remains Phase 12C.5; or
- launch approval, certification, regulatory acceptance, or safety authority.

## Planning note

The Phase 12D implementation plan should begin with project/compiler parity and a narrow accepted vehicle/mission editing slice before adding broader component libraries. It should use the accepted display model for preview and replay instead of creating editor-owned trajectory or frame logic.
