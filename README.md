# KSA64

KSA64 is a deterministic aerospace simulation framework for the Commodore 64. It combines a portable fixed-point physics core, simulated avionics and flight software, strict telemetry contracts, host-side validation, stock-C64 presentation, and optional REU-backed analysis.

> **Project status:** Phases 0–5 are complete and the Phase 6 software baseline is accepted. KSA64 now supports exact split endpoints, deterministic link contracts, a stock-C64 32 Hz flight-computer profile, host/VICE Commodore-in-the-loop execution, and passive ground systems. Live physical-link acceptance remains open.

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
- a host-native F1–F7 live Mission Control console with trajectory, GNC, navigation, vehicle, link, event, and truth-isolated SIM Director pages;
- adaptive fast/real-time/step presentation, operator pacing and safe detach/stop controls, procedural cues, and recoverable KMR6 session recording/replay with CSV/JSON export;
- the KSA-6R 32/8/1 Hz stock-C64 flight profile, measured PAL deadline margin, physical ACIA packaging, and a complete shadow-verified host/VICE flight.

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

The reviewed Phase 4 campaign uses seed `0x4b534134` and 1,024 runs. Its campaign identity is `0xa2e9e9d5` and its ordered summary chain is `0x813ce420`. Run zero reproduces the frozen Phase 3 nominal truth, sensor, navigation, flight, and KST3 checksums exactly.

The C64 accuracy-first closed-loop path is intentionally slower than real time. The accepted projection for one complete target mission is 243.7 minutes, so full target campaigns are not started as routine validation. Native runs provide campaign breadth; finite MOS/VICE probes provide target exactness, storage, UI, and transport evidence. Long C64 runs require a fresh projection and explicit user confirmation and are never canceled merely to obtain timing evidence.

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

## Phase 6: software baseline accepted

Phase 6 splits world authority, flight software, and passive Mission Control across configurable native, VICE, hybrid, or physical endpoints. The KSA-6R flight profile completed 12,692 epochs on a stock C64 image under 1x PAL x64sc with every command/status cell shadow-verified, exact terminal checksums, and no deadlines or alarms. Its measured controller workload uses about 49.5% average PAL CPU.

The accepted VICE mailbox path is externally paced and therefore does not prove end-to-end wall-clock realtime transport. The SwiftLink/Turbo232 endpoint is built below the stock-memory boundary, while physical ACIA, user-port, and Ultimate hardware acceptance remain open. See the [Phase 6 completion record](phase6/COMPLETION.md).
Run a complete host-world/host-flight/host-Mission-Control mission with:

```powershell
# Fast run with compact summary and an automatic KMR6 recording
powershell -File phase6/run.ps1

# Live 32 Hz F1–F7 Mission Control dashboard
powershell -File phase6/run.ps1 -Pace realtime -Display tui
```

The live console supports pause, single-step, 0.25x through MAX pacing, bookmarks, SI/US units, three procedural sound profiles, safe detach, postflight review, and replay/export. See the [deployment launcher guide](phase6/LAUNCHER.md).

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
