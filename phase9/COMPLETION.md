# Phase 9 completion record

Status: software implementation and deterministic evidence accepted on 2026-07-25. One finite VICE presentation probe remains blocked at monitor handshake and is not claimed as passed.

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

Two sequential finite VICE attempts were closed after the binary monitor failed to answer its initial PING. No emulator process remains. This is presentation/automation evidence still to repair, not a physics or optimizer failure, and the completion record does not mislabel it as accepted. Selected finalists can still use the already accepted Phase 8.5 split host/C64 flight endpoint.

## Scope and limitations

The KFP9 default pack retains bounded finalist summaries. Detailed canonical flight histories are generated on demand by exact finalist reruns rather than duplicated into every optimization archive. REU capacity may increase presentation retention later but cannot alter ordering or evaluation.

The broad-airframe demonstration is explicitly experimental and contributes no accepted finalist. Canards, cold-gas RCS, mixed-effector allocation, Bayesian optimization, CMA-ES, ECEF/ECI global flight, and physical-link acceptance remain deferred.

These results are engineering-model evidence, not launch approval, certification, regulatory evidence, or safety authority.
