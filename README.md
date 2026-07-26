# KSA64

KSA64 is a deterministic aerospace simulation framework for the Commodore 64. It combines a portable fixed-point physics core, simulated avionics and flight software, strict telemetry contracts, host-side validation, stock-C64 presentation, and optional REU-backed analysis.

> **Project status:** Phases 0–9.5 are complete. Phase 9.5 adds portable canard and cold-gas RCS physics, exact pulse/depletion and changing mass properties, truth-blind advanced avionics, deterministic mixed allocation, full integrated missions, robust grid/NSGA-II studies, live Mission Control, and selected-finalist stock-C64 execution. The interim stock baseline is host world plus externally paced C64 flight; realtime C64 flight and the portable C64 world remain priority follow-on tracks. Phase 10 global atmospheric/suborbital flight is next.

KSA64 asks a deliberately unreasonable question:

> What would a modern aerospace simulation architecture look like if its target computer were a Commodore 64?

The answer is not an arcade game and not a desktop simulator squeezed unchanged into 64 KB. KSA64 uses the strategy that made early aerospace computing practical: select the smallest model that answers the current question, use explicit numeric representations, isolate vehicle truth from flight software, validate independently, and spend memory and CPU only where measurements justify it.

## What works today

KSA64 currently provides:

- a portable `no_std` Rust core compiled natively and through David's rust-mos fork;
- deterministic fixed-point arithmetic with declared range, rounding, overflow, and fail-closed behavior;
- one-dimensional vertical flight and a rotating-Earth planar multistage ascent model;
- the fictional KSA-2A launch vehicle, atmosphere, Mach-dependent drag, staging, pitch guidance, and orbital classification;
- transport-isolated accelerometer, gyro, altimeter, and GPS-like sensors;
- truth-blind aided navigation, closed-loop insertion guidance, sequencing, actuator feedback, alarms, and abort logic;
- canonical KST1/KST2/KST3 telemetry and strict derived replay formats;
- deterministic Phase 4 parameter campaigns and streaming statistics;
- stock-C64 campaign analysis with five retained interesting runs and a sparse trajectory history;
- optional REU detection and adaptive archives from 128 KiB through 16 MiB;
- interactive PETSCII campaign pages and strict one- or multi-volume IEC disk export;
- independent Python/float64 analysis, host/C64 exactness probes, and frozen regression artifacts;
- additive Phase 5 ECI translation, rigid attitude, bending/slosh, two-axis gimbal and RCS dynamics;
- strict Phase 5 IMU/barometer/GPS/star-tracker transports, truth-isolated 3-D navigation, quaternion control, and fail-closed sequencing;
- KLF6/KLR6 split-endpoint contracts, deterministic broker and impairment evidence, passive Mission Control, and independent ground tracking;
- a responsive host-native F1–F7 Mission Control console with planned-versus-observed ascent, osculating-orbit, Earth ground-track, GNC, navigation, vehicle, link, event, and truth-isolated SIM Director views;
- adaptive fast/real-time/step presentation, operator pacing and safe detach/stop controls, procedural cues, and recoverable KMR6 session recording/replay with CSV/JSON export;
- the KSA-6R 32/8/1 Hz stock-C64 flight profile, measured PAL deadline margin, physical ACIA packaging, and a complete shadow-verified host/VICE flight.
- an additive Phase 7 evaluation facade preserving KSA-2A/KSA-5A while adding the separately scaled `HobbyVerticalV1` profile;
- offline compiled KVP7/KMP7/KMC7 packs, sampled motor thrust and mass depletion, rail launch, apogee, dual-deploy recovery, strict KST7/KSR7/KPH7 evidence, and KRA7 campaigns;
- a frozen 1,024-run Firestorm 54/I211W uncertainty campaign independently parsed in Python and byte-identical across worker counts;
- stock-C64 Phase 7 replay, 129-state exactness evidence, and a complete 2,702-step target mission matching the host checksum in 17.72 PAL CPU minutes.
- the separately versioned `HobbySpatialV1` profile with ENU 6-DOF ascent, rail constraint, layered keyed wind, weathercocking, event-driven dual deployment, and 3-D recovery drift;
- provenance-backed Firestorm geometry with derived CG, inertia, CP, static margin, damping, and reviewed Mach/Cd tables;
- strict KVP8–KRA8 evidence, independent float64 analysis, 19 passing OpenRocket 24.12 comparisons, and a byte-identical 1,024-run campaign;
- host CSV/JSON and trajectory plots plus a seven-page stock-C64 replay and an exact finite native/MOS trace; the full target mission is built below `$C000` but not run because its conservative projection is 2.35 hours.
- an exact 32 Hz local-ENU avionics executor with retained physical deadlines, sensor-N/command-N/effective-N+1 ordering, truth-blind navigation, and measured-state dual-deploy recovery;
- monitor-only control for the original Firestorm and a separately identified fictional two-axis-gimbal derivative that passes the 5 m/s crosswind settling gate;
- KAP8/KAC8/KLE8/KLR8/KAT8/KAS8/KMR8 contracts, host/host and host/VICE placements, a shared live F1–F7 Mission Control TUI, and a deterministic 64-run avionics campaign;
- a 15,412-byte stock-C64 flight endpoint and PAL avionics kernel using 68.8% of a release slot; the 71,500-byte self-contained combined image stopped at the documented stock-fit boundary rather than silently requiring expansion memory;
- a deterministic Phase 9 optimization workbench with strict manifests, grid/NSGA-II/DE engines, feasibility-first Pareto ranking, 1/8/64-case robustness, exact resume, sensitivity analysis, and an external JSONL evaluator;
- ten frozen search studies and 13.1 MB of independently verified evidence, byte-identical at one, four, and eight workers, plus live TUI and self-contained HTML/JSON/CSV reports;
- a 15,391-byte stock-C64 KFP9 finalist browser below `$C000`; its finite VICE probe validates four finalists and manifest `e86077d4`, then closes the sole emulator instance.
- Phase 9.5 canard, twelve-jet cold-gas RCS, exact 1/256-second pulses, regulated/blowdown supply tables, changing propellant mass properties, pitot fallback, and `PriorityResidualV1` mixed-effector allocation;
- accepted Firestorm C9/R9/M9 missions and deterministic canard, RCS, and mixed workbench studies, with experimental KSA-X1 evidence kept outside the accepted physical Pareto front;
- an externally paced KLF6/KLR9 host-world/stock-C64-flight baseline that shadow-verifies exact command/status cells and checksum chains without claiming realtime performance or requiring an REU;
- a passive Phase 9.5 F1–F7 host Mission Control TUI and KMR9 recording for host/host or host/C64 flight placement;
- adaptive KFE9 finalist retention, a 29,010-byte stock finalist browser, and a 39,963-byte configurable stock flight endpoint that exactly reruns selected canard, RCS, and mixed finalists through strict KFB9 bootstraps.

An REU is **not required** to run the simulation, calculate campaign aggregates, browse the stock analysis UI, or export the default stock report. More REU capacity increases retained summaries and detailed histories without changing physics or campaign results.

## Architecture

The implemented single-machine system is:

```text
scenario and campaign inputs
             |
             v
    +-------------------+
    | simulated world   |  private truth, environment, vehicle
    +---------+---------+
              |
       sensor transports
              v
    +-------------------+
    | flight software   |  navigation, guidance, sequencing
    +---------+---------+
              |
      actuator commands
              v
    +-------------------+
    | world authority   |  validates and applies physical effects
    +---------+---------+
              |
       canonical evidence
              v
    +-------------------+
    | host / C64 tools  |  inspection, analysis, UI, REU, disk export
    +-------------------+
```

Phase 6 can now place those logical endpoints in separate native processes, VICE, or a hybrid host/C64 arrangement:

```text
C64 #1  flight computer
C64 #2  vehicle simulator
C64 #3  mission control
```

The accepted accessible baseline is a host-owned world plus one C64 flight computer. Two- and three-C64 arrangements remain optional demonstrations, and the split still uses one physics implementation.

## Completed phases

| Phase | Result |
|---|---|
| 0 — Feasibility | Rust/rust-mos selected over the Oscar64 C++ challenger for the frozen arithmetic workload; numeric behavior and toolchains pinned. |
| 1 — Vertical laboratory | Checked variable-mass vertical flight, canonical telemetry, host/C64 exactness, timing, and independent high-precision error attribution. |
| 2 — Planar ascent | Rotating-Earth KSA-2A multistage ascent, atmosphere and drag, orbital insertion/failure cases, KST2 evidence, and PETSCII/SID replay. |
| 3 — Closed-loop avionics | Truth-isolated sensors, navigation, guidance, sequencing, actuator feedback, deterministic faults, KST3/KRP3, and bounded C64 probes. |
| 4 — Statistical analysis | Deterministic campaigns, KSR4 summaries, independent float64 analysis, stock/REU storage, interactive UI, KRA4 archives, and KXV4 disk export. |
| 5 — 3-D dynamics | Spatial numeric/world models, rigid and flexible dynamics, multirate KSA-5A vehicle, strict avionics, integrated missions, KST5, spatial campaigns, PAL target timing, adaptive spatial history, and stock-C64 replay pass the completion audit. |
| 6 — Commodore-in-the-loop | Exact and realtime endpoint contracts, deterministic broker/replay, passive ground systems, a stock-C64 flight profile, and a complete 12,692-epoch host/VICE flight pass the software audit; physical-link validation remains open. |
| 7 — Multi-profile evaluation | Complete: frozen legacy adapters, compiled hobby vehicle/motor/mission packs, published-data vertical ascent and dual-deploy recovery, strict evidence, deterministic campaigns, independent analysis, and an exact complete stock-C64 mission. |
| 8 — Spatial hobby flight | Complete: geometry-derived mass properties/stability, bounded 6-DOF ascent, deterministic wind, recovery drift, strict evidence, float64/OpenRocket comparison, campaigns, host plots, and stock-C64 replay. |
| 8.5 — Unified avionics | Complete: exact event execution, local truth-blind navigation/recovery, monitor and gimbal capabilities, strict formats, host/VICE placement, live Mission Control, campaigns, PAL timing, and an explicit combined-stock fit decision. |
| 9 — Optimization workbench | Complete: deterministic grid/NSGA-II/DE search, 1/8/64 robustness, feasibility-first Pareto evidence, exact archives/resume, external optimizer protocol, reports/TUI, and a finite stock-C64 finalist-browser VICE pass. |
| 9.5 — Advanced effectors | Complete: canards, cold-gas RCS, exact pulses/depletion, mixed allocation, robust searches, externally paced stock-C64 flight, live Mission Control, adaptive finalist browsing, selected-finalist split reruns, and full completion audit. |

The reviewed Phase 4 campaign uses seed `0x4b534134` and 1,024 runs. Its campaign identity is `0xa2e9e9d5` and its ordered summary chain is `0x813ce420`. Run zero reproduces the frozen Phase 3 nominal truth, sensor, navigation, flight, and KST3 checksums exactly.

The legacy Phase 3/4 accuracy-first closed-loop path still projects to 243.7 minutes per C64 mission, so its campaigns remain native with finite target probes. The smaller Phase 7 vertical profile completed its full target mission in 17.72 PAL CPU minutes. Phase 8 spatial flight measures about 3.71 million PAL cycles per powered step and conservatively projects to 2.35 hours, so its full target mission is not a completion requirement and was not started. Campaign breadth remains host-native. Long C64 runs require a fresh projection and explicit user confirmation and are never canceled merely to obtain timing evidence.

## Repository guide

### Start here

- [Roadmap](ROADMAP.md) — phase boundaries, entry criteria, and future direction.
- [Architecture](docs/architecture.md) — system layers, ownership, storage, and portable-core boundaries.
- [Decision record](docs/decisions.md) — accepted architectural and numerical choices.
- [Validation strategy](docs/validation.md) — analytic, exact, high-precision, independent, and target evidence.
- [Data formats](docs/data-formats.md) — versioned scenario, telemetry, campaign, archive, and export families.
- [Toolchains](toolchains/README.md) — pinned rust-mos, Oscar64, and VICE setup.

### Phase records

- [Phase 0](phase0/README.md) and [compiler results](phase0/RESULTS.md)
- [Phase 1](phase1/README.md) and [completion audit](phase1/COMPLETION.md)
- [Phase 2](phase2/README.md) and [completion audit](phase2/COMPLETION.md)
- [Phase 3](phase3/README.md) and [completion audit](phase3/COMPLETION.md)
- [Phase 4](phase4/README.md) and [completion audit](phase4/COMPLETION.md)
- [Phase 5](phase5/README.md), [completion audit](phase5/COMPLETION.md), [Phase 6 handoff](phase5/PHASE6_HANDOFF.md), and [deployment options](phase5/PHASE6_OPTIONS.md)
- [Phase 6](phase6/README.md), [wire/authority contract](phase6/CONTRACT.md), and [software completion record](phase6/COMPLETION.md)
- [Phase 7](phase7/README.md), [implementation contract](phase7/PLAN.md), and [completion audit](phase7/COMPLETION.md)
- [Phase 8](phase8/README.md), [implementation contract](phase8/PLAN.md), and [completion audit](phase8/COMPLETION.md)
- [Phase 8.5](phase8_5/README.md), [implementation contract](phase8_5/PLAN.md), [completion audit](phase8_5/COMPLETION.md), [stock-fit decision](phase8_5/STOCK_FIT_DECISION.md), and [Phase 9 handoff](phase8_5/PHASE9_HANDOFF.md)
- [Phase 9](phase9/README.md), [implementation contract](phase9/PLAN.md), [completion record](phase9/COMPLETION.md), and [Phase 9.5 handoff](phase9/PHASE9_5_HANDOFF.md)
- [Phase 9.5](phase9_5/README.md), [implementation contract](phase9_5/PLAN.md), [completion record](phase9_5/COMPLETION.md), [integrated evidence](phase9_5/INTEGRATED_EVIDENCE.md), [stock-target decision](phase9_5/stock-target-decision.md), [finalist workflow](phase9_5/FINALIST_WORKFLOW.md), and [Phase 10 handoff](phase9_5/PHASE10_HANDOFF.md)

### Phase 4 detail

- [Campaign contract](phase4/CONTRACT.md)
- [Distributions](phase4/DISTRIBUTIONS.md)
- [Campaign execution](phase4/CAMPAIGNS.md)
- [Formats](phase4/FORMATS.md)
- [Independent host analysis](phase4/HOST_ANALYSIS.md)
- [Stock storage and UI](phase4/STOCK_STORAGE.md)
- [Adaptive REU storage](phase4/REU_STORAGE.md)
- [Browsing and export](phase4/EXPORT.md)

The [host tools](host/README.md) capture, inspect, analyze, and package canonical evidence. Files under `sources/` are synced reference material and must remain read-only.

## Building and checking

Prerequisites and immutable toolchain pins are documented in [toolchains/README.md](toolchains/README.md). From the repository root:

```powershell
powershell -File tools/toolchains/verify.ps1
cargo test --workspace --features fixtures
```

The complete Phase 4 audit is intentionally decomposed:

```powershell
python -B phase4/reference/generate_distributions.py --check
python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json --check
powershell -File phase4/stock.ps1
powershell -File phase4/reu.ps1
powershell -File phase4/export.ps1
powershell -File phase4/export-c64.ps1
```

These commands validate checked-in artifacts; normal checks do not silently regenerate or replace frozen evidence.

The complete bounded Phase 5 audit is:

```powershell
powershell -File phase5/complete.ps1
```

It validates native and independent evidence plus finite MOS/VICE probes. It
does not start a complete C64 mission or campaign.

The bounded Phase 6 software audit is:

```powershell
powershell -File phase6/complete.ps1
```

It validates the checked-in full-flight evidence without silently rerunning the approximately 17-minute target mission.

The bounded Phase 7 audit is:

```powershell
powershell -File phase7/complete.ps1
```

It rebuilds packs and mission artifacts, reproduces the 1,024-run campaign with one and four workers, runs the independent analyzer, and reruns only finite C64 trace/replay probes. The frozen 17.72-minute Phase 7 target flight is verified by result and binary hash rather than silently repeated.

The bounded Phase 8 audit is:

```powershell
powershell -File phase8/complete.ps1
```

It rebuilds packs and strict mission artifacts, reproduces the 1,024-run campaign with one and four workers, checks independent float64/OpenRocket evidence, builds every stock-C64 program below `$C000`, and runs only finite exact-trace/replay probes. It does not silently start the projected 2.35-hour target mission.

The bounded Phase 8.5 audit is:

```powershell
powershell -File phase8_5/complete.ps1
```

It verifies all frozen Phase 0–8 evidence, exact local avionics tests, the one/four-worker campaign, checked PAL and VICE evidence, and MOS endpoint packaging. Add `-RunVice` only when you explicitly want the sequential finite emulator probes rerun. It never attempts the non-fitting combined image or a long target mission.

The bounded Phase 9 audit is:

```powershell
powershell -File phase9/complete.ps1
```

It verifies the full native workspace, independent accepted-search evidence, the external JSONL protocol, fresh one/four/eight-worker quick searches, strict reports, KFP9 packaging, and the pinned rust-mos stock browser. It validates checked-in accepted searches rather than silently spending minutes regenerating all thirty worker variants. Add `-RunVice` to revalidate the checked finite browser evidence with one sequential emulator instance.

## Phase 6: software baseline accepted

Phase 6 splits world authority, flight software, and passive Mission Control across configurable native, VICE, hybrid, or physical endpoints. The KSA-6R flight profile completed 12,692 epochs on a stock C64 image under 1x PAL x64sc with every command/status cell shadow-verified, exact terminal checksums, and no deadlines or alarms. Its measured controller workload uses about 49.5% average PAL CPU.

The accepted VICE mailbox path is externally paced and therefore does not prove end-to-end wall-clock realtime transport. The SwiftLink/Turbo232 endpoint is built below the stock-memory boundary, while physical ACIA, user-port, and Ultimate hardware acceptance remain open. See the [Phase 6 completion record](phase6/COMPLETION.md).
Run a complete host-world/host-flight/host-Mission-Control mission with:

```powershell
# Fast run with compact summary and an automatic KMR6 recording
powershell -File phase6/run.ps1

# Live 32 Hz F1–F7 Mission Control dashboard, initially focused on orbit
powershell -File phase6/run.ps1 -Pace realtime -Display tui -TrajectoryView orbit
```

The live console fills terminals from 80x24 through ultra-wide layouts. F2 switches among planned-versus-observed ascent, orbital geometry, and rotating-Earth ground track; wide pages add timelines, histories, status matrices, and link ribbons. It also supports pause, single-step, 0.25x through MAX pacing, bookmarks, SI/US units, three procedural sound profiles, safe detach, postflight review, and replay/export. See the [Mission Control guide](phase6/MISSION_CONTROL.md) and [deployment launcher guide](phase6/LAUNCHER.md).

Move only the flight computer into one VICE C64 with `-Flight vice`. The launcher rejects VICE-world and multi-VICE selections until those endpoint programs actually exist; see the [deployment launcher guide](phase6/LAUNCHER.md).


## Guiding principles

1. Use the simplest model that answers the question.
2. Maintain one portable core, not parallel simulators that can drift.
3. Host/C64 agreement proves consistency, not physical correctness.
4. Keep vehicle truth inaccessible to flight software.
5. Make every format, approximation, resource cost, and failure mode explicit.
6. Preserve frozen evidence unless a versioned model decision deliberately replaces it.
7. Prefer a slower correct result to a real-time decorative one.
8. Optimize measured kernels and keep everything else readable.
