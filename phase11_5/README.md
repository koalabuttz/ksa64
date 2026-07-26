# Phase 11.5: unified KSA64 application

Status: complete and accepted on 2026-07-26.

Phase 11.5 is a host-product consolidation phase. It adds no simulator or physical model. The accepted vertical, local, global, operations, campaign, optimization, evidence, and C64-target workflows now sit behind one deterministic catalog and one `ksa64` application boundary. A pre-Phase 12 hardening amendment splits the internal adapters, completes the unified request family, and keeps accepted products, authored projects, and generated sessions as separate identity domains. It also supplies Mission Foundry with a deterministic incremental operations session instead of forcing the GUI to reconstruct a mission loop.

## Quick start

```powershell
cargo run -p ksa64-host --bin ksa64 --
cargo run -p ksa64-host --bin ksa64 -- catalog list
cargo run -p ksa64-host --bin ksa64 -- mission control ksa-g10r.operations --scenario gnss-loss
```

The no-argument command is non-mutating. Stored target verification does not launch VICE, and live target work always requires an explicit flag.

## Records

- [PLAN.md](PLAN.md) — accepted implementation contract.
- [BASELINE.md](BASELINE.md) — frozen Phase 11 compatibility baseline.
- [BINARY_INVENTORY.md](BINARY_INVENTORY.md) — executable classification.
- [COMMANDS.md](COMMANDS.md) — unified commands and migration table.
- [TARGETS.md](TARGETS.md) — C64 target catalog and safety policy.
- [COMPLETION.md](COMPLETION.md) — accepted outcome and validation.
- [HARDENING.md](HARDENING.md) — accepted pre-Phase 12 application-boundary amendment.
- [PHASE12_HANDOFF.md](PHASE12_HANDOFF.md) — direct Rust API boundary for Mission Foundry.
- [product-catalog-v1.json](product-catalog-v1.json) — deterministic product snapshot.

Run the bounded audit with:

```powershell
powershell -File phase11_5/complete.ps1
```

Add `-RunVice` only to explicitly repeat the finite sequential target probes. No complete target mission starts implicitly.
