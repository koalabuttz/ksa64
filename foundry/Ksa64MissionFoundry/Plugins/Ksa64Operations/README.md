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
