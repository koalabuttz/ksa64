# Phase 11.5 completion record

Status: complete and accepted on 2026-07-26.

Phase 11.5 consolidates KSA64 into one discoverable host product without changing authoritative simulation, avionics, campaign, optimization, evidence, or C64 behavior.

## Accepted outcome

- `ksa64` is the default host executable and `ksa64-host` is an alias over the same application facade.
- Running `ksa64` with no arguments prints a useful non-mutating quick start.
- The deterministic current catalog contains 13 domain-oriented experiences, seven C64 targets, and an opt-in historical tier covering Phase 0 through Phase 11.
- The checked `ksa64.product-catalog.v1` snapshot has SHA-256 `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`.
- Project, mission, campaign, optimization, evidence, target, and audit actions route through one public Rust facade intended for direct Phase 12 use.
- The guided KSA-G10R GNSS-loss operations scenario is the flagship quick start.
- The unified scripted session and the Phase 11 compatibility wrapper produce the same 22,369-byte KSB11 evidence, identity `0x6d4122a0`, and SHA-256 `38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8`.
- The hidden `capture`, `inspect`, `phase2-capture`, and `phase2-inspect` aliases preserve the original `ksa64-host` interface. The tested Phase 1 capture remains 10,312 bytes with stream CRC `0xcf56fe65` and state checksum `0x72bf6e0e`.
- Stored target verification never launches VICE. A live probe without `--live` fails before process creation.
- Clap 4.6.1 and its required runtime crates are pinned host-only and vendored with upstream license and provenance records; portable and C64 crates do not depend on them.

No new canonical `K*` format was introduced. The product catalog and application outcomes are host metadata.

## Validation

The repeatable audit is [complete.ps1](complete.ps1). It passed in three bounded forms:

1. Native and stored-evidence completion with Phase 11 compatibility.
2. rust-mos target-only packaging and stock-memory checks.
3. Explicit sequential warp-disabled VICE probes.

The audit passed:

- repository formatting and Clippy with warnings denied;
- the complete Phase 11 native workspace regression and Phase 11.5 host suite;
- catalog validation, asset validation, deterministic ordering, JSON snapshot, and quick-start stability;
- unified/legacy session and telemetry byte parity;
- Phase 11 prediction, authoring SDK, replay, debrief, and corruption gates;
- all seven stored target descriptors without emulator startup;
- the unchanged 32,857-byte flat safehold endpoint and 55,423-byte banked reference-operations bundle;
- a 16-release safehold VICE probe with zero failures; and
- a 13-record banked-operations VICE probe with exact navigation `c73060d2`, flight `6e07595c`, and command `6ab926f2` checksums.

Both VICE runs used one instance at a time, warp disabled, the required cooldown, and successful process cleanup. No complete target mission was started.

Machine-readable results are in [completion-audit.json](completion-audit.json). The command migration table is [COMMANDS.md](COMMANDS.md), target policy is [TARGETS.md](TARGETS.md), and the Phase 12 API boundary is [PHASE12_HANDOFF.md](PHASE12_HANDOFF.md).

## Product boundary

The facade performs discovery, validation, and orchestration. Physics remains in the accepted core/simulation implementations; flight decisions remain in profile-specific flight packages; search remains in the accepted workbenches; strict parsers remain the evidence authorities; and C64 programs remain unchanged.

The specialized phase tools remain available where their full engineering surfaces exceed the deliberately smaller product command. Compatibility wrappers remain supported through at least Phase 13.

## Remaining boundaries

- Phase 12 Mission Foundry and passive 3-D operations are not part of this phase.
- Physical C64 loader, user-port, ACIA, and Ultimate Ethernet acceptance remain open.
- Realtime C64 flight, a 6502-specific rewrite, C64 Ultimate acceleration, REU package overlays, and a portable C64 global world remain separate tracks.
- Stored target verification proves catalog/evidence integrity; deep semantic acceptance remains owned by each phase audit.
- KSA64 remains engineering simulation evidence, not launch approval, certification, regulatory evidence, or safety authority.


## Pre-Phase 12 hardening amendment

The accepted follow-up in [HARDENING.md](HARDENING.md) preserves every completion result while preparing the host seam for Mission Foundry. Domain adapters are separated behind the same facade; the request family now covers all seven product domains with explicit safety metadata; accepted products, authored projects, and recent sessions are different types; and unknown binary evidence is labelled opaque rather than recognized. The checked product catalog remains byte-identical. The amendment also adds the live deterministic GNSS-loss session required by Phase 12: exact release stepping, typed snapshots/events/actions, pause and pacing controls, and finalization through the unchanged KSB11 builder. The existing operations console now consumes that session rather than displaying a precomputed result.
