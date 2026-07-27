# KSA64 Mission Operations

This runtime plugin is the source-controlled Phase 12B presentation shell.
It owns no simulation, flight software, mission semantics, canonical evidence,
or role filtering. Those remain in Rust and cross into Unreal through
`Ksa64Bridge`.

The plugin contains:

- one game-instance subsystem that is the sole bridge consumer;
- a compatibility adapter around the currently qualified bridge surface;
- a Slate command desk with trajectory, procedure, navigation, uplink,
  timeline, disposition, and engineering views;
- deterministic semantic-state output for automation;
- visual pacing controls that never skip authoritative releases.

When a qualified bridge does not advertise a richer operations view, the
corresponding action and disposition surfaces remain visibly unavailable.
Unreal does not infer mission state or parse canonical `K*` records.

## Accepted product slice

Phase 12B is accepted. The command desk runs the complete Guided Operator GNSS-loss scenario, submits only explicit high-level Review/Stage/Validate/Commit or Cancel actions, and preserves exact release/action evidence across realtime, pause, step, fast, sparse/burst polling, and 30/60/144-Hz scheduling fixtures.

The accepted scripted run reaches release 21,591 and returns the exact 2,911,464-byte KSB11 archive. It is Degraded Success: the primary objective and nominal vehicle recovery succeed with complete procedure/operator/evidence results, while persistent GNSS loss keeps avionics degraded. The sealed archive is role-neutral opaque post-run evidence; live Guided Operator views remain truth-filtered before crossing the ABI.

Unreal automation passes 17/17. The standalone D3D12 presentation is nonblank at 1920x1080, supports high contrast, reduced motion, 1.25 text scale, and optional sound cues, and records zero overflow. Its bounded bridge/presentation service sample measures 258,900 ns p99 and 460,000 ns maximum; this is not total GPU frame time.

Phase 12C adds the passive global 3-D engineering viewer. This plugin does not own ENU/ECEF/GCRF conversion, Earth/vehicle pose, entry/recovery rendering, cameras, simulation, role filtering, or canonical parsing. See [the accepted completion record](../../../../phase12/PHASE12B_COMPLETION.md) and [Phase 12C handoff](../../../../phase12/PHASE12C_HANDOFF.md).
