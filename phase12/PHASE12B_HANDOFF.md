# Phase 12B handoff: live GNSS-loss operations presentation

Phase 12A accepts the engine, toolchain, bridge, packaging, and authority seam.
Phase 12B may now build the first real Mission Foundry experience without
reopening those decisions.

## Accepted starting point

- Unreal Engine consumes KSA64 only through the versioned C ABI in
  `viewer-bridge`.
- Rust owns `Ksa64Application`, `LiveMissionSession`, role filtering,
  high-level action validation, exact release ordering, and KSB11
  finalization.
- One dedicated Rust worker owns each session handle.
- Game-thread bridge calls are bounded enqueue or poll operations.
- The guided `ksa-g10r.operations` GNSS-loss session is the first experience.
- The packaged runtime does not require Unreal Editor, Python, or MCP.
- The sidecar remains a fallback, not parallel Phase 12B work.

The accepted bridge and package identities are recorded in
[completion-audit.json](completion-audit.json). Phase 12B should consume the
manifest rather than naming an unqualified DLL.

## Phase 12B objective

Present the existing live GNSS-loss operations scenario as an attractive,
truth-isolated, deterministic operator experience:

```text
Ksa64Application / LiveMissionSession
                  |
          versioned viewer bridge
                  |
       presentation-owned interpolation
                  |
       Mission Foundry operations UI
```

The vertical slice should let a guided operator:

1. start the accepted scenario;
2. observe role-filtered telemetry and events;
3. follow the active deterministic procedure;
4. stage, inspect, and commit the permitted ground update and branch action;
5. pause, single-step, run fast, or pace against wall time without changing
   simulation ordering;
6. finish the mission and retrieve the exact KSB11 evidence; and
7. replay the same transcript with byte-identical final evidence.

## Required presentation work

Phase 12B owns:

- a live session lifecycle and connection-status surface;
- Guided Operator procedure prompts and observable step guards;
- typed forms for the existing stage, commit, and cancel actions;
- explicit staged/validated/committed/rejected receipts;
- a mission timeline and event journal;
- compact navigation, communications, plan, prediction, and recovery panels;
- smooth rendering interpolation alongside an exact-release inspection mode;
- pause, resume, single-release, fast, and realtime presentation pacing;
- clean completion, bridge-failure, invalid-action, and session-shutdown UX;
- deterministic screenshot and functional automation for the accepted slice.

No Phase 12B widget may construct a direct effector command or gain access to
SIM Director truth while operating as Guided Operator.

## Data and authority rules

- Poll snapshots and events; never run simulation work in an Unreal callback.
- Treat validity masks and result codes as mandatory, not advisory.
- Keep role immutable for one bridge session.
- Submit actions only through the accepted Phase 11 stage–validate–commit
  payloads.
- Keep wall-clock animation, frame interpolation, camera state, widget state,
  sound, and visual hints outside experiment identity.
- Do not infer missing telemetry from scene state.
- Do not parse KSB11 or other canonical evidence in Unreal when an owning Rust
  service already exists.
- A display prediction is labelled by its accepted source identity; it is not
  world truth.
- Identical ordered action transcripts must produce identical session evidence
  regardless of frame rate, pause pattern, interpolation mode, or page layout.

## Recommended first layout

Keep the first slice operational rather than cinematic:

- **Header:** mission time, phase, package, procedure, communications, and
  bridge health.
- **Trajectory/prediction:** compact 2-D planned, onboard-estimate, and
  ground-estimate traces.
- **Procedure:** current step, observable guard, timeout, permitted action,
  caution, and hint.
- **Navigation:** GNSS validity, onboard/ground estimates, residual, and update
  status.
- **Uplink:** load editor, validation receipt, commit countdown, and
  acknowledgment.
- **Timeline:** fault, journal, procedure, action, and mission events.
- **Transport diagnostics:** queue depth/pressure and last typed bridge error,
  visible in an engineering/debug panel rather than the normal operator view.

Phase 12C will add the complete global engineering viewer, Earth-scale display
domains, vehicle pose, entry, and recovery. Phase 12B should not quietly absorb
that scope.

## Acceptance evidence

At minimum, Phase 12B should prove:

- the complete scripted transcript and equivalent interactive transcript yield
  byte-identical KSB11 evidence;
- Guided Operator data contains no truth-only field;
- no frame-rate, pause, polling-frequency, or interpolation choice changes
  release or event order;
- stage, reject, commit, cancel, blackout, and reacquisition states are
  represented without inventing authority;
- bridge queue pressure fails visibly and deterministically;
- destroying a running or paused session shuts down its worker cleanly;
- the packaged Development build completes the experience without editor,
  Python, or MCP;
- normal automation remains headless and does not require network access; and
- Unreal presentation failures cannot mutate or masquerade as completed
  canonical evidence.

## Deferred boundaries

- Phase 12C: full global replay, ENU/ECEF/GCRF display conversion, Earth, pose,
  entry, recovery, and performance.
- Phase 12D: source editors and GUI/headless compiler parity.
- Phase 12E: production art, NASA-derived assets, Niagara, quality tiers, and
  visual polish/performance.
- Later review: sidecar process only if the accepted in-process containment
  stops meeting its failure or replacement requirements.

The core rule remains delightfully simple: Unreal makes KSA64 legible and
beautiful; it never becomes KSA64's second brain.
