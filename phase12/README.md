# Phase 12: Unreal feasibility, live operations, and Mission Foundry

Status: active. The Phase 12A contract and engine decision are accepted;
implementation and acceptance evidence are pending.

Phase 12 adds a native-Windows Unreal presentation and authoring surface above
the frozen KSA64 Rust application. It does not move simulation authority,
role filtering, action validation, or evidence construction into Unreal.

The work is divided so that each slice proves one coherent boundary:

| Subphase | Result |
|---|---|
| 12A | Pinned UE 5.8 toolchain, versioned live bridge, native harness, minimal runtime plugin, packaging smoke test, and optional MCP feasibility |
| 12B | Live role-filtered GNSS-loss operations, procedures, actions, and exact evidence finalization |
| 12C | Complete Phase 10 global engineering replay, coordinate/display domains, events, Earth, and packaged performance |
| 12D | Mission Foundry vehicle/mission authoring and GUI/headless compiler parity |
| 12E | Production visual assets, NASA-derived reference material, effects, quality tiers, and visual performance |

## Phase 12A documents

- [PLAN.md](PLAN.md) — implementation and acceptance contract.
- [ENGINE_DECISION.md](ENGINE_DECISION.md) — accepted engine, authority, and
  rollout decision, including deliberate changes to the supplied Unreal guide.
- [TOOLCHAIN.md](TOOLCHAIN.md) — native-Windows setup, verification, and lock
  procedure.
- [toolchain-lock.example.toml](toolchain-lock.example.toml) — noncanonical
  machine/toolchain manifest template.
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