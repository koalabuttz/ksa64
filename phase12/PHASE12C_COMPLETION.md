# Phase 12C completion record — global mission visualization and renderer parity

Status: **complete and accepted**.

Date: 2026-07-28

Entry commit: `eb666cbaf3b8950218656a7ad7fe135b05385813`

Accepted source commit: `64d72f2a4ee0848bf7ff73c345fcd1cf56579ba1`

Phase 12C is complete. The source-bound completion audit ran the frozen
regressions, portable contracts, exact native and WebAssembly missions, bridge
harnesses, Unreal build and automation, packaged Win64/D3D12 runtime, rendered
Babylon lanes, and the strict cross-renderer comparator without skipped
acceptance gates.

The machine-readable acceptance record is
[`phase12c-completion-audit.json`](phase12c-completion-audit.json).

## Accepted capability

Phase 12C now provides one Rust-owned, renderer-neutral `GlobalDisplayV1`
boundary for the complete global KSA-G10R mission:

- fixed-width display definitions, exact-release samples, role-permitted source
  poses, frame transitions, event-preserving paths, and replay indices;
- additive, capability-gated KPS1 1.0 records and an optional size-tagged
  `GlobalDisplayApiV1` table without changing any ABI-v1 symbol or layout;
- Rust-owned ENU/ECEF/GCRF conversion, source identity, role filtering,
  continuity and discontinuity classification, path construction, and mission
  disposition;
- exact-release replay seeking, stepping, event jumps, bounded path levels,
  and fail-closed snap boundaries;
- a packaged Unreal procedural global viewer with Large World Coordinate and
  explicit-origin handling; and
- a Babylon/PWA global viewer with WebGPU, forced WebGL2, complete 2-D fallback,
  context-loss fallback, local-WASM authority, broker, and verified-replay
  lanes.

Unreal and Babylon remain passive renderers. Neither owns physics, navigation,
frames, events, actions, success classification, or canonical evidence.

## Frozen evidence preserved

| Evidence | Accepted value |
|---|---|
| Product catalog | 13 entries |
| Catalog SHA-256 | `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13` |
| Nominal KTT10 SHA-256 | `a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a` |
| Nominal KPH10 SHA-256 | `cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2` |
| Nominal KSR10 SHA-256 | `9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa` |
| Nominal replay | 22,015 releases, 0 through 22,014 |
| Nominal transitions | 4 |
| GNSS-loss session | 21,591 releases; 4 accepted actions |
| GNSS-loss KSB11 | 2,911,464 bytes |
| GNSS-loss KSB11 SHA-256 | `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4` |

The nominal planned/reference lineage and the separately identified exact
current re-execution remain governed by
[PHASE12C_NOMINAL_COMPATIBILITY.md](PHASE12C_NOMINAL_COMPATIBILITY.md). Phase
12C visualizes both without rewriting either lineage.

## Completion-gate ledger

| Gate | Status | Durable evidence |
|---|---|---|
| Frozen Phase 12B.5 entry hashes | Accepted | `complete-phase12c.ps1` source-bound audit |
| Formatting and warnings-denied Clippy | Accepted | completion audit |
| Complete native workspace suite | Accepted | completion audit |
| GlobalDisplay Rust/C++/TypeScript vectors | Accepted | native, web, and Unreal contract suites |
| Shared in-process/WASM/native path products | Accepted | strict parity manifest |
| Legacy KPS1 1.0 and ABI-v1 exactness | Accepted | frozen vectors and bridge harnesses |
| Broker, LAN, reconnect, and role isolation | Accepted | portable completion suite |
| WebAssembly capability and exact-evidence lanes | Accepted | complete nominal and GNSS-loss WASM evidence |
| Nominal 22,015-release lineage/replay | Accepted | native harness and strict comparator |
| GNSS 21,591-release exact session/replay | Accepted | exact KSB11 and guided milestones |
| Unreal Editor target | Accepted | clean UE 5.8 build |
| `KSA64.Phase12C` automation | Accepted | 13 succeeded, 0 failed |
| Packaged Win64/D3D12 bridge smoke | Accepted | package audit and runtime validation |
| Packaged launch/coast/entry/recovery/landing visuals | Accepted | nine 1920×1080 semantic/screenshot pairs |
| Babylon WebGPU rendered lane | Accepted | 71.895 fps |
| Babylon forced-WebGL2 rendered lane | Accepted | 74.384 fps |
| Babylon complete 2-D fallback | Accepted | 75.562 fps |
| Context-loss fallback | Accepted | rendered browser manifest |
| Exact source/path/event/discontinuity/continuity parity | Accepted | strict cross-renderer comparator |
| Raw path-state flags and normalized view-mode parity | Accepted | strict cross-renderer comparator |
| Render/action/evidence invariance | Accepted | Unreal, Babylon, and operations suites |
| Phase 0–12B.5 frozen regressions | Accepted | source-bound completion audit |

The physical Duet, Vita3K, and physical Vita Phase 12B.5 qualification remains
an independent open workstream. Phase 12C acceptance neither marks those gates
complete nor makes them prerequisites retroactively.

## Runtime and package metrics

| Measurement | Acceptance threshold | Accepted result |
|---|---:|---:|
| Unreal procedural tier | 1920×1080 at 60 fps | 192.260 fps |
| Unreal display service p99 | < 1 ms | 305,300 ns |
| Unreal display service maximum | < 2 ms | 366,300 ns |
| Bridge availability poll p99 | < 1 ms | 8,500 ns |
| Bridge range poll p99 | < 1 ms | 364,600 ns |
| Babylon WebGPU | responsive ≥ 30 fps | 71.895 fps |
| Babylon WebGL2 | responsive ≥ 30 fps | 74.384 fps |
| Babylon 2-D fallback | fully operational | 75.562 fps |
| Packaged executable | report only | 340,359,680 bytes |
| Immutable packaged application | report only | 958,121,179 bytes / 14 files |
| Full package archive | report only | 1,036,404,342 bytes / 54 files |
| Web production bundle | report only | 3,340,231 bytes / 115 files |
| Nominal display replay | report only | 16,043,524 bytes |
| Exact whole-mission path storage | report only | 3,503,288 bytes / 124,921 points |
| Exact active-window path | report only | 28,716 bytes / 1,024 points |
| Unreal origin changes | continuity required | 35; semantic continuity preserved |
| Browser origin changes | continuity required | 1; semantic continuity preserved |

The Unreal timing scope is `GlobalDisplayV1` poll, decode, semantic update,
origin handling, and procedural-scene service time; it is not GPU frame
latency. The bridge p99 threshold applies to availability and range polling.
Bulk path fetches are reported separately by the native harness and are not
misrepresented as sub-millisecond polling.

## Evidence identities

| Artifact | SHA-256 |
|---|---|
| Strict cross-renderer evidence v2 | `c869a5dbc341ea6b5272e901882fe803dd2e15f1ab49cbeff48788527c01e50e` |
| Native GlobalDisplay harness | `ac2a848926a6e027d7ca508082f56fb9ecb059ee3884984be433de5ae689242c` |
| Rendered-browser evidence | `20e988752baf1c692d4f71a68ea979e9a3252458f63af64623dec6faf2330315` |
| Packaged Unreal runtime evidence | `8ee6ba9b182666f329f356f92906fe15e9ce5bacf9665ed2100cce0c3d6d39cd` |
| Unreal runtime validation | `a33f0a9e3d97e00df60ef7fad424e56aa4702154284333d94cdfe05851ca1d33` |
| Unreal automation evidence | `82cc65374ffcd044c7af1ae5e08b584c51e078573ed29f22415dc858d9b98a6e` |
| Package audit | `beb0f264d76b2ae7c291731fc26e18b12a7fae7f89663408e48e657ca90e25b0` |
| Packaged directory inventory | `1b73e4790cf206463ab88b77047801c17d4351667e21de3bae1f59514051acf2` |
| Immutable packaged directory tree | `a0509377e0fd245b72cae881eb3d53d9bd677cc3cad9fe76d8d8cdd7b0378846` |
| Web production tree | `a815caaedcc783164da4ef18285e042aa34582a41b44eb68be164779b7dd6933` |

The strict parity record contains nine nominal milestones—launch, burnout,
coast, apogee, entry, recovery, drogue, main, and landing—and six guided
operations milestones at releases 5,760, 5,824, 6,080, 6,240, 6,560, and
6,720. Every compared milestone binds visible sources, paths, event masks,
discontinuity masks, continuity identities, dispositions, and role policy.

## Role, truth, and outcome acceptance

- Guided Operator products contain no SIM-truth pose or path.
- Read-only SIM Director replay permits truth but starts with it hidden.
- Truth-enabled presentation retains a persistent `SIM TRUTH` label.
- Nominal replay finishes with the accepted nominal multidimensional
  disposition.
- GNSS-loss finishes as degraded success, not an inferred failure from path
  deviation.
- Polling rate, renderer, backend, origin, camera, layout, replay speed, and
  visibility controls cannot change evidence or action ordering.

## Retained limitations and handoff

- Procedural engineering visuals are not production art.
- NASA imagery, terrain, production meshes, Niagara, Lumen, Nanite, installers,
  signing, and store distribution remain outside Phase 12C.
- Linux/Vulkan and macOS/Metal Unreal packaging await qualified hosts.
- Physical Duet, Vita3K, and physical Vita product qualification remains open
  under the independent Phase 12B.5/12C.5 device workstream.
- Visual and numerical agreement is engineering evidence, not launch approval,
  certification, regulation, or safety authority.

Phase 12C.5 may now qualify and productize cross-platform clients. Phase 12D may
consume the accepted display and application boundaries for mission authoring;
neither handoff grants a renderer authority over simulation or evidence.
