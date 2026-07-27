# Phase 12: Unreal feasibility, live operations, and Mission Foundry

Status: active. Phase 12A is complete and accepted; Phase 12B is the next
implementation slice.

Phase 12 adds a native-Windows Unreal presentation and authoring surface above
the frozen KSA64 Rust application. It does not move simulation authority,
role filtering, action validation, or evidence construction into Unreal.

The work is divided so that each slice proves one coherent boundary:

| Subphase | Result |
|---|---|
| 12A | **Complete:** pinned UE 5.8 toolchain, versioned live bridge, native harness, minimal runtime plugin, packaged smoke test, and optional MCP feasibility |
| 12B | Live role-filtered GNSS-loss operations, procedures, actions, and exact evidence finalization |
| 12C | Complete Phase 10 global engineering replay, coordinate/display domains, events, Earth, and packaged performance |
| 12D | Mission Foundry vehicle/mission authoring and GUI/headless compiler parity |
| 12E | Production visual assets, NASA-derived reference material, effects, quality tiers, and visual performance |

## Phase 12A documents

- [PLAN.md](PLAN.md) — accepted implementation and acceptance contract.
- [COMPLETION.md](COMPLETION.md) — accepted outcome, measurements, and limitations.
- [completion-audit.json](completion-audit.json) — machine-readable accepted evidence.
- [complete.ps1](complete.ps1) — bounded, non-live-by-default completion audit.
- [PHASE12B_HANDOFF.md](PHASE12B_HANDOFF.md) — frozen boundary and next-slice handoff.
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