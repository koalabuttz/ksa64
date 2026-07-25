# Phase 9 — deterministic design optimization and robustness workbench

Status: implemented and accepted in software on 2026-07-25. The finite VICE finalist-browser screen probe remains an explicitly recorded environment limitation because the configured binary monitor did not answer its initial PING; the stock MOS image itself builds and fits.

Phase 9 keeps candidate selection on the host and reuses the accepted Phase 8.5 avionics-aware evaluator. It does not add another physics simulator.

## What it provides

- Strict 2,048-byte KOM9 search manifests and 256-byte KDV9 canonical design vectors.
- Exact nominal, eight-case search, and 64-case finalist robustness tiers.
- Feasibility-first constraint handling with exact normalized violation ordering.
- Deterministic GridV1, NSGA-II V1, and DE/rand/1/bin engines.
- Byte-identical proposal, generation, archive, report, and finalist outputs with one, four, or eight workers.
- Generation-boundary checkpoint/resume with prefix validation and atomic replacement.
- KOE9 aggregates, segmented KRA9 archives, KRE9 retained KAS8 case evidence, KSN9 sensitivity records, and KFP9 C64 finalist packs.
- A bounded JSONL evaluator service for external optimizers.
- Live seven-page optimization TUI plus self-contained HTML, strict JSON, and human-oriented CSV reports.
- A stock-C64 finalist browser and exact finalist rerun path through the existing host-world/C64-flight endpoint.

## Accepted studies

| Study | Engine/evidence | Evaluations | Terminal Pareto | 64-case finalists |
|---|---:|---:|---:|---:|
| Passive/recovery | DE | 408 | 1 | 32 |
| Passive/recovery | 17×17 grid | 610 | 14 | 32 |
| Passive/recovery | NSGA-II | 454 | 27 | 32 |
| Gimbal/control | DE | 298 | 1 | 32 |
| Gimbal/control | 17×17 grid | 531 | 2 | 32 |
| Gimbal/control | NSGA-II | 487 | 7 | 32 |
| Coupled demonstration | NSGA-II 32×16 | 256 | 32 | 16 |
| Experimental airframe | NSGA-II 32×12 | 190 | 1 | 0 accepted |

The experimental airframe search is deliberately excluded from the validated physical Pareto evidence. Its geometry/material/aerodynamic derivations are exploratory.

## Use

Run a built-in search:

```powershell
cargo run -p ksa64-host --release --bin phase9 -- search study-a nsga2 accepted target/phase9/study-a 4 --tui
```

Compile and run a JSON manifest:

```powershell
cargo run -p ksa64-host --release --bin phase9 -- compile phase9/examples/accepted-study-a-nsga2.json target/phase9/study-a.kom9
cargo run -p ksa64-host --release --bin phase9 -- search-kom9 target/phase9/study-a.kom9 target/phase9/study-a 4 --resume
```

Start the persistent external-evaluator protocol:

```powershell
cargo run -p ksa64-host --release --bin phase9 -- serve-kom9 target/phase9/study-a.kom9 target/phase9/transcript.jsonl
```

Run the bounded audit:

```powershell
powershell -File phase9/complete.ps1
```

The normal audit validates frozen evidence and finite target packaging. It does not rerun every accepted search or silently start VICE. Add `-RunVice` only to retry the finite finalist-browser probe after diagnosing the binary-monitor handshake.

## Evidence boundary

KSA64 optimization results are outputs of the declared engineering model. They are not launch approval, certification, regulatory advice, structural qualification, or safety authority. External tools may propose candidates, but KSA64 validates and evaluates every materialized pack itself.
