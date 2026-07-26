# Phase 11.5: product consolidation and unified application shell

Status: accepted implementation contract.

Baseline: Phase 11 completion commit `20abf4ccf44074523c511cb7399505a1bd805416`.

## Purpose

Turn KSA64's integrated simulation architecture into one discoverable host
product without changing authoritative models, frozen formats, evidence, or
C64 programs.

Phase 11.5 adds a host-only application facade, a deterministic two-tier
product catalog, and a unified `ksa64` command. Domain names become the normal
way to discover and invoke missions, campaigns, optimization, evidence,
targets, and audits. Phase-numbered entrypoints remain compatibility and
verification surfaces.

## Accepted direction

- `ksa64` is the primary host executable and default Cargo run target.
- Running it without a command prints a non-mutating quick start.
- The flagship experience is guided KSA-G10R GNSS-loss operations.
- Current product experiences and historical verification tools occupy
  separate catalog tiers.
- Clap 4 is host-only and provides the command hierarchy and validation.
- Catalog and application services are public Rust APIs for Phase 12.
- Documented legacy host binaries remain compatible through at least Phase 13.
- No phase directory, crate, artifact, or target program is moved.
- No VICE, hardware, network, or long C64 run starts implicitly.

## Product catalog

The current tier contains:

- `firestorm.vertical`
- `firestorm.spatial`
- `firestorm.avionics`
- `firestorm.canard`
- `firestorm.rcs`
- `firestorm.mixed`
- `firestorm.design`
- `firestorm.control`
- `firestorm.effectors`
- `ksa-g10r.global`
- `ksa-g10r.operations`
- `ksa-g10r.safehold`
- `ksa-5a.orbit-coast`

The historical tier contains Phase 0-11 audits, compatibility commands,
reference utilities, and target probes. Catalog JSON uses schema
`ksa64.product-catalog.v1`; it is deterministic host metadata, not canonical
simulation evidence.

## Command contract

```text
ksa64 catalog list|show
ksa64 project lint|compile|run|script
ksa64 mission run|control
ksa64 campaign run
ksa64 optimize compile|run|run-manifest|serve
ksa64 evidence inspect|verify|replay|debrief
ksa64 target list|show|build|verify|probe
ksa64 audit list|run
```

Human output is the default. Discovery commands support deterministic JSON.
Live target probes require `--live`; live audit VICE requires `--live-vice`.
Stored evidence is used otherwise.

## Gates

1. Freeze and inventory the Phase 11 baseline.
2. Add the catalog and host application facade.
3. Add the unified Clap CLI and quick start.
4. Route projects, missions, and evidence through application services.
5. Route campaigns and optimization through stable domain IDs.
6. Add target inventory and safe audit dispatch.
7. Preserve documented phase entrypoints as compatibility wrappers.
8. Complete parity, frozen-regression, target, documentation, and Phase 12
   handoff evidence.

## Acceptance

- Every Phase 0-11 artifact remains byte-identical.
- Current experiences are discoverable without phase knowledge.
- Catalog ordering and JSON are deterministic.
- Existing mission, session, campaign, search, and target results remain exact.
- Compatibility commands preserve arguments, stdout, exit status, and output
  bytes.
- Stored target verification cannot launch VICE.
- The application facade is callable directly by Phase 12 without parsing CLI
  output or spawning phase binaries.

## Non-goals

- Physics, avionics, numeric, campaign, optimizer, or binary-format changes.
- A repository-wide directory migration or universal state representation.
- New C64 code, a 6502 rewrite, REU overlays, Ultimate acceleration, or
  physical-link acceptance.
- Mission Foundry, a graphical launcher, or a 3-D viewer.
- Automatic hardware interaction or an unconfirmed long target run.
