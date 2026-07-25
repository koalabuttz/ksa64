# Phase 9 completion record

Status: accepted on 2026-07-25. The deterministic host evidence, stock-C64 packaging, and finite one-instance VICE finalist-browser probe pass.

## Accepted implementation

Phase 9 adds a host optimization workbench around the unchanged Phase 8.5 evaluator. Search algorithms choose bounded design vectors; materialization creates identity-bound candidate packs; the portable evaluator alone produces physical, navigation, control, and recovery evidence.

Implemented gates:

1. KOM9/KDV9 manifest and candidate compilation with validation and duplicate canonicalization.
2. Fixed 1/8/64 uncertainty tiers, metric aggregation, feasibility-first ordering, and KOE9 aggregates.
3. Lexicographic grids, exact one-quantum sensitivity, NSGA-II V1, and DE/rand/1/bin.
4. Ordered multi-worker evaluation and keyed random proposals independent of scheduling.
5. KRA9 generation commits, KRE9 retained KAS8 evidence, corruption rejection, and byte-identical resume.
6. Bounded JSONL external optimizer protocol with ordered responses and transcript capture.
7. Passive live TUI and self-contained HTML/JSON/CSV reports.
8. KFP9 stock finalist packaging and a stock-C64 browser below `$C000`.

## Reproducibility evidence

Seed: `0x4b534139`.

All ten checked studies emitted byte-identical manifest, archive, finalist, sensitivity, CSV, JSON, and HTML files with one, four, and eight workers. The independent Python implementation verified strict framing, CRCs, KDV9 quantization, KOE9 identities and objectives, generation fingerprints, all retained KAS8 records, terminal Pareto fronts, and unique feasible 64-case finalists.

Key evidence:

- Worker exactness SHA-256: `61365ef93009f4c46bfe490c352f6a8a93a27777071bb5946667c8a5c56c74b7`.
- Independent audit SHA-256: `9801b4d268f51bcf8f0f5cbb43bafe55e7ff0818f5b468144cfe2e89597ce8de`.
- External protocol example SHA-256: `7851878f2022942d1e409b16ae3e098abcc0bf2f260453715e5eba3d251501b3`.
- Checked Phase 9 evidence: 74 files, 13,094,864 bytes.

The six accepted primary archives preserve their earlier exact SHA-256 values; upgrading reports, progress observation, and resume support did not change any search archive bytes.

## Target evidence

The pinned David/koalabuttz rust-mos container builds `ksa64-phase9-finalist-c64` as:

- 15,391-byte PRG.
- Load address `$0801`.
- End address `$441E`.
- SHA-256 `b953f152daafdcf98d15407241f3029f5f9aecfdc222ace08875025c1ffd275d`.
- No REU required.

Two earlier finite attempts encountered a binary-monitor PING timeout while the host had been overloaded by stale/multiple VICE instances. With every prior instance closed, the unchanged helper and PRG passed on the first clean diagnostic and again in checked-evidence mode. The accepted result reports status/code zero, four finalists, manifest `e86077d4`, and zero remaining VICE processes. The evidence therefore supports transient emulator starvation rather than a protocol or PRG defect. Selected finalists can also use the accepted Phase 8.5 split host/C64 flight endpoint.

## Scope and limitations

The KFP9 default pack retains bounded finalist summaries. Detailed canonical flight histories are generated on demand by exact finalist reruns rather than duplicated into every optimization archive. REU capacity may increase presentation retention later but cannot alter ordering or evaluation.

The broad-airframe demonstration is explicitly experimental and contributes no accepted finalist. Canards, cold-gas RCS, mixed-effector allocation, Bayesian optimization, CMA-ES, ECEF/ECI global flight, and physical-link acceptance remain deferred.

These results are engineering-model evidence, not launch approval, certification, regulatory evidence, or safety authority.
