# Phase 12: Unreal feasibility, live operations, and Mission Foundry

Status: Phase 12A and Phase 12B are complete and accepted. Phase 12B.5 software implementation and local acceptance are complete from entry commit `b9f2c79`; hosted portable-runtime qualification is complete; full acceptance remains pending the required physical Duet, Vita3K emulator, and physical Vita gates.

Phase 12 began with an accepted native-Windows Unreal presentation and now
expands through portable Rust authority, desktop Unreal, web/PWA, SDL2/Vita,
Android, and iOS presentation lanes. It does not move simulation authority,
role filtering, action validation, or evidence construction into a renderer or
client.

The work is divided so that each slice proves one coherent boundary:

| Subphase | Result |
|---|---|
| 12A | **Complete:** pinned UE 5.8 toolchain, versioned live bridge, native harness, minimal runtime plugin, packaged smoke test, and optional MCP feasibility |
| 12B | **Complete:** accepted 674.71875-second GNSS-loss mission, human-scale operations, multi-axis outcomes, packaged 2-D command desk, 17/17 automation, exact evidence, and bounded presentation-service timing |
| 12B.5 | **Software- and hosted-portable-runtime-qualified; device acceptance pending:** portable session/KPS1 authority, bridge/platform packaging, secure loopback and paired LAN, compact React/Babylon PWA, exact Rust/WASM authority, and Vita/SDL2; device and emulator gates remain open |
| 12C | Renderer-neutral Phase 10 global engineering viewer through desktop Unreal and Babylon.js WebGPU/WebGL2 |
| 12C.5 | Portable web/PWA, Vita/SDL2, Android, and iOS operations clients |
| 12D | Mission Foundry vehicle/mission authoring and GUI/headless compiler parity |
| 12E | Production visual assets, NASA-derived reference material, effects, quality tiers, and visual performance |

## Phase 12 records

- [PLAN.md](PLAN.md) — accepted implementation and acceptance contract.
- [COMPLETION.md](COMPLETION.md) — accepted outcome, measurements, and limitations.
- [completion-audit.json](completion-audit.json) — machine-readable accepted evidence.
- [complete.ps1](complete.ps1) — bounded, non-live-by-default completion audit.
- [PHASE12B_HANDOFF.md](PHASE12B_HANDOFF.md) — historical frozen Phase 12A-to-12B handoff.
- [PHASE12B_PLAN.md](PHASE12B_PLAN.md) — accepted live-operations implementation and acceptance contract.
- [PHASE12B_COMPLETION.md](PHASE12B_COMPLETION.md) — accepted live-operations outcome, Unreal evidence, measurements, and limitations.
- [phase12b-completion-audit.json](phase12b-completion-audit.json) — machine-readable full-reference hashes, outcome axes, authority boundary, and accepted product gates.
- [PHASE12B5_HANDOFF.md](PHASE12B5_HANDOFF.md) — historical cross-platform runtime, browser, mobile, and Vita planning boundary.
- [PHASE12B5_PLAN.md](PHASE12B5_PLAN.md) — implementation and acceptance contract.
- [PHASE12B5_COMPLETION.md](PHASE12B5_COMPLETION.md) — software-complete checkpoint, local evidence, and explicit pending qualification gates.
- [CI.md](CI.md) — hosted native/WASM checks and the explicit physical-device exclusions.
- [WEB_PLATFORM.md](WEB_PLATFORM.md) — accepted Babylon.js, React/TypeScript, Rust/WASM, browser-authority, lifecycle, and Duet decision.
- [PHASE12C_HANDOFF.md](PHASE12C_HANDOFF.md) — frozen Phase 12B global-viewer boundary, consumed after 12B.5.
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
only accepted live mission boundary. Unreal, Babylon, web/PWA, SDL2, and mobile
clients may request work and display typed results. They may not parse CLI
text, spawn phase executables as an integration mechanism, run substitute
physics, infer hidden truth, mutate canonical evidence, or recreate the mission
loop.

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

The accepted four-action reference lands at release 21,591 (674.71875 seconds) and seals a 2,911,464-byte KSB11 with SHA-256 `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`. It is Degraded Success because the primary mission objective, vehicle/recovery, procedure, operator, and evidence are accepted while persistent GNSS loss leaves avionics degraded operational. Unreal automation passed 17/17; the standalone packaged mission reproduced the exact KSB11; and the 1920x1080 D3D12 presentation recorded zero overflow with 258,900 ns p99 and 460,000 ns maximum bridge/presentation service time. The latency result is not total GPU frame time, and the 30/60/144-Hz fixtures prove scheduling/checksum invariance rather than universal rendered frame rates.

Live Guided Operator views are truth-filtered before crossing the C ABI. The sealed KSB11 is role-neutral post-run evidence and crosses the bridge only as opaque bytes for custody and Rust-side verification; Unreal does not gain authority to parse its canonical internals.

The typed Unreal bridge opens Rust in `Fast` execution-capacity mode solely so explicit bounded `Advance(n)` calls are honored. Unreal alone schedules realtime, pause, single-step, 4x, 16x, and maximum-fast wall-clock presentation. That internal setting is noncanonical, records no pace evidence, and preserves exact KSB11 whenever the release and action transcripts are identical.

The presentation is a modern NASA-inspired, C64-accented 2-D operations desk. Phase 12C retains Earth-scale 3-D display domains, vehicle pose, entry, recovery, and cameras.

## Cross-platform direction

Windows/D3D12 remains the frozen first accepted graphical lane, not the permanent product boundary. Phase 12B.5 qualifies native Rust execution on Linux x86-64, Linux ARM64, and macOS ARM64; generalizes the bridge; and freezes a role-filtered presentation-session contract for Unreal, web, SDL2/Vita, Android, and iOS clients.

The 8 GB Lenovo Chromebook Duet 11 is the reference ARM64 Linux and ChromeOS device. Crostini may own the Rust world and flight computer while a ChromeOS PWA renders Mission Control and Babylon.js WebGPU/WebGL2 3-D locally. Android is a second native graphical path; Crostini-hosted Unreal remains experimental.

The web stack is React plus TypeScript and Vite for the PWA, Babylon.js for 3-D, and a dedicated Rust/WASM Web Worker for optional local authority. The browser may ultimately run World, flight computer, operations, evidence, Mission Control, and 3-D together, but only after its complete KSB11 is byte-identical to native Rust. Babylon remains presentation-only and its physics integration is disabled.

PlayStation Vita is a first-class constrained SDL2 target. Its initial product is Mission Control, replay, and high-level operations; bounded host-world/Vita-flight and selected Vita-world placements follow exact compile, vector, and physical timing evidence.

The explicit native/Vita paired-LAN host lane is separate from the loopback browser broker. `ksa64-paired-lan` accepts only a user-selected private or link-local interface, requires a locally compared Noise XX code before storing a peer, binds the peer to immutable Guided Operator authority, supports local list/revocation, and uses Noise IK for reconnect. Its host identity and peer registry are local noncanonical configuration; physical Vita/Vita3K acceptance remains pending.
