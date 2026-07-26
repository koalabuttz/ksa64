# KSA64 host application and telemetry tools

## Unified product shell

`ksa64` is the primary host entrypoint and the Cargo package default. Running it with no arguments prints a non-mutating quick start. The shared `Ksa64Application` facade exposes the same catalog and services directly to Phase 12; graphical tools must not parse CLI output or spawn phase binaries.

```powershell
cargo run -p ksa64-host --bin ksa64 --
cargo run -p ksa64-host --bin ksa64 -- catalog list
cargo run -p ksa64-host --bin ksa64 -- mission control ksa-g10r.operations --scenario gnss-loss
```

See [Phase 11.5 commands](../phase11_5/COMMANDS.md), [target policy](../phase11_5/TARGETS.md), and the checked [product catalog](../phase11_5/product-catalog-v1.json). `ksa64-host` and documented phase-numbered programs remain compatibility entrypoints.

The host crate is a `std` adapter around the portable `ksa64-core`; it does not contain a second simulator. It captures records through the same `TelemetrySink` boundary used by C64 targets, then inspects the canonical binary stream with the core decoder.

## Capture and inspect

From the project root:

    cargo run -p ksa64-host -- capture target/phase1-vertical.kst
    cargo run -p ksa64-host -- inspect target/phase1-vertical.kst
    cargo run -p ksa64-host -- phase2-capture target/phase2-ascent.kst2
    cargo run -p ksa64-host -- phase2-inspect target/phase2-ascent.kst2

`capture` executes the checked Phase 1 mission, writes one 32-byte header and 257 40-byte frames, then immediately reads and validates the resulting file. `inspect` performs the same validation without running the mission.

`phase2-capture` executes KSA-2A, writes one 40-byte header and 901 64-byte frames, and strictly validates the 57,704-byte result. `phase2-inspect` validates an existing canonical `KST2` stream without rerunning physics.

The compact text view reports scenario identity, timestep and stride, stream length and CRC, final physical state, rolling state checksum, and frames carrying cutoff, depletion, or numeric-fault events. Decimal values are presentation only; validation uses canonical raw integers.

## Strict validation

Inspection rejects:

- truncated headers, partial frames, and trailing bytes;
- unknown versions, lengths, numeric contracts, reserved fields, status bits, or event bits;
- header or frame CRC failures;
- a stream bound to a different scenario, timestep, or telemetry stride;
- an initial frame that differs from the validated scenario;
- non-monotonic steps, nonterminal off-stride frames, or inconsistent mission time;
- numeric-fault frames without end-of-run;
- terminal frames before the end of the file or a missing terminal frame;
- successful terminal frames that do not reach the scenario step limit.

The writer adapter propagates I/O errors through the existing mission failure type. File format, scheduling, dynamics, and checksumming remain owned by `ksa64-core`.

## Phase 3 host validation

Phase 3 host support lives in `ksa64_host::phase3`. It captures each closed-loop case through the canonical KST3 sink, strictly inspects an existing stream against the unchanged KSC2 scenario plus its exact KSC3 image, and derives KRP3 only from an accepted stream. The library reports the first bad frame and rejects framing, identity, CRC, reserved-field, cadence, time, terminal, sensor-projection, and engine/phase inconsistencies.

The checked example regenerates all four reviewed case sets during development:

    cargo run -p ksa64-host --example generate_phase3_artifacts

Normal completion uses `phase3/check.ps1`, which validates frozen artifact hashes and tests inspection/derivation without silently updating golden files. Independent physical acceptance comes from `phase3/reference/verify_missions.py`; it parses KST3 separately rather than treating the host inspector as its oracle.

## Phase 4 campaigns and exports

Phase 4 host support executes parameterized closed-loop missions, emits KSC4/KSR4 evidence, folds summaries in run-index order, prepares stock- or REU-neutral archives, and joins strict KXV4 volumes. Parallel workers affect elapsed time only; configuration, summaries, aggregates, archive selection, and output bytes remain deterministic.

The reviewed smoke campaign contains 64 runs. The frozen reference campaign contains 1,024 runs with master seed `0x4b534134`. Its independent analyzer reconstructs every variation without Rust and computes authoritative float64 orbital, load, and navigation results:

    python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json --check

Host archive/export tests build a one-volume stock report and a synthetic three-volume selection, then require the joiner to reject corruption, missing, duplicate, reordered, or mismatched volumes. The actual C64 IEC utility is validated separately by `phase4/export-c64.ps1`.

See `phase4/HOST_ANALYSIS.md`, `phase4/FORMATS.md`, and `phase4/EXPORT.md` for commands, frozen identities, and accepted evidence.


## Phase 6 endpoint broker

Phase 6 host support can run the world broker over TCP while a native or C64 flight endpoint owns guidance and control. The broker reads the KLR6 readiness preamble, streams inertial/aid cells, validates returned command/status cells, and compares every result with a native shadow flight computer. The shadow is an observer only and never supplies commands to the world.

The frozen full target run used the same broker behind the VICE binary-monitor mailbox relay. See `phase6/README.md` and `phase6/COMPLETION.md` for exact evidence and the remaining physical-link boundary.


## Phase 6 deployment launcher

`phase6_launch` runs the host world and native flight endpoint across a localhost TCP seam, with optional passive host Mission Control and fast, 32 Hz wall-paced, or manual-step release. The repository-level `phase6/run.ps1` wrapper adds the host-world/VICE-flight combination while enforcing one-VICE-at-a-time lifecycle rules. See `phase6/LAUNCHER.md`.


## Phase 6 live console and sessions

`phase6_launch` selects the responsive Ratatui F1–F7 console for realtime/step runs on an interactive terminal and a compact summary for fast or redirected runs. `--display tui` forces the dashboard; `--display summary` and `--display none` support automation. `--plot auto|braille|ascii` controls plot glyphs, while `--trajectory-view ascent|orbit|ground` selects the initial F2 visualization. F2 compares the frozen KPH5 plan with onboard and independent ground estimates; only F7 reads simulator truth. Recording defaults to `target/phase6/sessions/*.kmr6` in every display mode and may be disabled with `--record off`.

Replay a session with:

    cargo run -p ksa64-host --bin phase6_launch -- --replay target/phase6/sessions/<session>.kmr6

Inspect or export without opening the TUI with:

    cargo run -p ksa64-host --bin phase6_session -- --input <session>.kmr6 --csv <flight>.csv --json <flight>.json

KMR6 recording is append-only and CRC-protected per update. Partial recordings recover through their last complete record. Replay rebuilds plots and events from the exact recorded prefix. The TUI and recorder are passive sinks around `phase6_runner`; they do not enter the world/flight command path. See `phase6/MISSION_CONTROL.md` for source badges, responsive tiers, visual controls, and authority rules.
## Phase 7 profile tools

Phase 7 host tools compile human-readable hobby sources, run the portable exact
mission, build sparse plots, execute deterministic campaigns, and emit a native
trace oracle:

```powershell
cargo run -p ksa64-host --bin phase7_compile -- phase7/source-data target/phase7/packs
cargo run -p ksa64-host --bin phase7_run -- phase7/examples target/phase7/run
cargo run -p ksa64-host --release --bin phase7_campaign -- phase7/examples target/phase7/campaign 1024 4
cargo run -p ksa64-host --bin phase7_trace
```

`phase7/reference/analyze_campaign.py` independently parses and validates the
frozen campaign. See `phase7/README.md` and `phase7/COMPLETION.md`.

## Phase 8 spatial hobby tools

Phase 8 tools rebuild source-bound packs, execute exact missions and campaigns, export presentation data, and emit the native half of the target trace:

```powershell
cargo run -p ksa64-host --bin phase8_compile -- phase8/source-data target/phase8/packs
cargo run -p ksa64-host --bin phase8_run -- phase8/examples target/phase8/run
cargo run -p ksa64-host --release --bin phase8_campaign -- phase8/examples target/phase8/campaign 1024 4
cargo run -p ksa64-host --bin phase8_export -- phase8/examples target/phase8/exports
cargo run -p ksa64-host --bin phase8_trace
```

`phase8/reference/analyze.py`, `analyze_campaign.py`, and `openrocket/compare.py` independently validate canonical outputs. `phase8/complete.ps1` is the bounded completion audit; it runs finite VICE trace/replay probes but never silently launches the projected 2.35-hour target mission.

## Phase 9 optimization workbench

The `phase9` binary compiles KOM9 manifests, runs built-in or compiled searches, emits deterministic KRA9/KPF9/KSN9 evidence and HTML/JSON/CSV reports, opens the passive optimization TUI, and exposes a persistent JSONL evaluator service.

`phase9_search` owns grid/NSGA-II/DE proposal and selection logic; `phase9` owns candidate materialization and 1/8/64-case evaluation; `phase9_archive` owns generation commits/resume; `phase9_report`, `phase9_tui`, and `phase9_sensitivity` are observational. See `phase9/README.md` for commands and the accepted evidence boundary.

## Phase 9.5 Mission Control and finalist tools

Run the advanced host world and flight computer with the passive F1–F7 console:

    cargo run -p ksa64-host --bin phase9_5_launch -- --display tui --pace realtime

Use `--display summary|none`, `--pace fast|realtime`, `--max-releases N`, and `--record <path>.kmr9` for bounded automation and replay evidence. The same presentation sink can observe the externally paced C64 flight bridge without entering the command path.

Inspect a retained finalist package and rerun its accepted robustness tier exactly:

    cargo run -p ksa64-host --bin phase9_5_finalists -- --package phase9_5/evidence/workbench/mixed-nsga2.kfe9 --index 0 --reu-kib 0 --rerun

The stock finalist browser consumes KFE9 directly. A separate selected-finalist flight endpoint consumes the strict KFB9 configuration sent in the KLF6 Start payload; the host still owns the world and shadow-verifies every KLR9 command/status cell. See `phase9_5/FINALIST_WORKFLOW.md` for build, VICE, stock/REU retention, and evidence commands.

## Phase 10 global mission tools

`phase10_launch` runs the complete host-world/host-flight KSA-G10R mission and
feeds the passive global F1–F7 Mission Control model. It emits strict KTT10,
KSR10, KPH10, CSV, JSON, HTML/SVG, and optional KMR10 evidence.

`phase10_bridge` owns the externally paced KLF6/KLR10 seam. The host advances
to one exact release, delivers transported measurements, waits for the C64
flight response, shadow-verifies its cells, and then advances.

`phase10_campaign` executes deterministic 64/256-case studies with ordered
merging. `phase10_world_reference` emits the production uninstrumented
physical reference consumed by the independent Python comparison.

```powershell
cargo run -p ksa64-host --bin phase10_launch -- --display tui
cargo run -p ksa64-host --release --bin phase10_campaign -- target/phase10/campaign 64 8
cargo run -p ksa64-host --bin phase10_world_reference
```

See `phase10/README.md` and `phase10/VALIDATION.md`.
