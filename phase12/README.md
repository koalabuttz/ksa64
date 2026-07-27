# Phase 12: Unreal feasibility, live operations, and Mission Foundry

Status: active. Phase 12A is complete and accepted; Phase 12B is in
implementation under its accepted contract.

Phase 12 adds a native-Windows Unreal presentation and authoring surface above
the frozen KSA64 Rust application. It does not move simulation authority,
role filtering, action validation, or evidence construction into Unreal.

The work is divided so that each slice proves one coherent boundary:

| Subphase | Result |
|---|---|
| 12A | **Complete:** pinned UE 5.8 toolchain, versioned live bridge, native harness, minimal runtime plugin, packaged smoke test, and optional MCP feasibility |
| 12B | Exact 674.71875-second scripted GNSS-loss reference, human-scale operations, multi-axis outcomes, 2-D command desk, and exact evidence finalization |
| 12C | Complete Phase 10 global engineering replay, coordinate/display domains, events, Earth, and packaged performance |
| 12D | Mission Foundry vehicle/mission authoring and GUI/headless compiler parity |
| 12E | Production visual assets, NASA-derived reference material, effects, quality tiers, and visual performance |

## Phase 12 records

- [PLAN.md](PLAN.md) — accepted implementation and acceptance contract.
- [COMPLETION.md](COMPLETION.md) — accepted outcome, measurements, and limitations.
- [completion-audit.json](completion-audit.json) — machine-readable accepted evidence.
- [complete.ps1](complete.ps1) — bounded, non-live-by-default completion audit.
- [PHASE12B_HANDOFF.md](PHASE12B_HANDOFF.md) — frozen boundary and next-slice handoff.
- [PHASE12B_PLAN.md](PHASE12B_PLAN.md) — accepted live-operations implementation and acceptance contract.
- [PHASE12B_COMPLETION.md](PHASE12B_COMPLETION.md) — measured Rust evidence and the explicit Unreal acceptance remainder.
- [phase12b-completion-audit.json](phase12b-completion-audit.json) — machine-readable full-reference hashes, outcome axes, authority boundary, and pending gates.
- [complete-phase12b.ps1](complete-phase12b.ps1) — composed Phase 12A, Rust, C++ harness, and explicit optional Unreal audit.
- [ENGINE_DECISION.md](ENGINE_DECISION.md) — accepted engine, authority, and
  rollout decision, including deliberate changes to the supplied Unreal guide.
- [TOOLCHAIN.md](TOOLCHAIN.md) — native-Windows setup, verification, and lock
  procedure.
- [toolchain-lock.example.toml](toolchain-lock.example.toml) — noncanonical
  machine/toolchain manifest template.
- [toolchain-lock.toml](toolchain-lock.toml) — sanitized accepted workstation and
  toolchain evidence.
- [package.ps1](package.ps1) — reproducible Development cook, package, and
  headless bridge smoke gate.
- [unreal-mcp.codex.toml.example](unreal-mcp.codex.toml.example) — sanitized,
  loopback-only optional editor-tool configuration.
- [BRIDGE.md](BRIDGE.md) — versioned C ABI, ownership, threading, role, and
  failure-containment contract.
- [ASSETS.md](ASSETS.md) — source, LFS, provenance, generated-asset, and rights
  policy.

## Non-negotiable authority rule

`Ksa64Application` is the application facade and `LiveMissionSession` is the
only accepted live mission boundary. Unreal may request work and display typed
results. It may not parse CLI text, spawn phase executables as an integration
mechanism, run substitute physics, infer hidden truth, mutate canonical
evidence, or recreate the mission loop.

MCP and Unreal Python are supervised editor-development aids. A normal build,
automation run, cook, package, and shipped Mission Foundry session must not
need either one.

## Existing handoff

The authoritative entry conditions remain
[the Phase 11.5 handoff](../phase11_5/PHASE12_HANDOFF.md) and
[hardening amendment](../phase11_5/HARDENING.md). Phase 12 introduces no new
K-format and does not revise any Phase 0–11.5 accepted identity.

## Phase 12B direction

The player-facing slice runs the complete accepted KSA-G10R mission rather than presenting Phase 11's compressed nine-release fixture as realistic human operations. Persistent coast-phase GNSS loss drives a human-scale Review -> Stage -> Validate -> Commit workflow. Continuous realtime is the default, and autonomous flight continues if the operator chooses another valid route or does nothing.

Mission objective, vehicle, procedure, operator, avionics, and evidence outcomes are evaluated independently. Following the nominal checklist is not the sole definition of success: a recovered mission may be degraded or contingency success while separately recording a delayed, skipped, or failed procedure.

The accepted scripted Rust reference now lands at release 21,591 (674.71875 seconds), records four actions, and seals a 2,911,464-byte KSB11 with SHA-256 `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`. It is classified Degraded Success because the mission objective, vehicle, procedure, operator, and evidence are accepted while avionics remain degraded operational. Final Unreal build, automation, packaged full-session, accessibility, screenshot/semantic, and presentation-performance evidence is still pending.

Live Guided Operator views are truth-filtered before crossing the C ABI. The sealed KSB11 is role-neutral post-run evidence and crosses the bridge only as opaque bytes for custody and Rust-side verification; Unreal does not gain authority to parse its canonical internals.

The typed Unreal bridge opens Rust in `Fast` execution-capacity mode solely so explicit bounded `Advance(n)` calls are honored. Unreal alone schedules realtime, pause, single-step, 4x, 16x, and maximum-fast wall-clock presentation. That internal setting is noncanonical, records no pace evidence, and preserves exact KSB11 whenever the release and action transcripts are identical.

The presentation is a modern NASA-inspired, C64-accented 2-D operations desk. Phase 12C retains Earth-scale 3-D display domains, vehicle pose, entry, recovery, and cameras.
