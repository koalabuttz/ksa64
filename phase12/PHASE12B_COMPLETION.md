# Phase 12B completion record

Status: complete and accepted at source commit `423c116cf58632f344d4a48774a97a4487c34113`.

Date: 2026-07-27

## Outcome

Phase 12B provides the accepted packaged, role-filtered KSA-G10R GNSS-loss operations experience. The Guided-Operator reference transcript completes at exact release 21,591, or 674.71875 seconds at 32 Hz. The untouched no-action path remains 22,015 releases, or 687.96875 seconds.

The accepted reference run:

- injects and qualifies persistent coast-phase GNSS loss from transported observations;
- performs four recorded operator actions through the ordinary Review -> Stage -> Validate -> Commit authority boundary;
- completes physical recovery;
- seals strict KTT10, KPH10, KSR10, action, procedure, prediction, journal, and session evidence into one KSB11 archive; and
- classifies the run as **Degraded Success**, not failure merely because its avionics state is off nominal.

The four-action reference achieved its primary objective and nominal vehicle/recovery criteria with complete evidence. Persistent GNSS loss leaves the avionics axis Degraded Operational; the aggregate is therefore Degraded Success, not Nominal Success. A plan or procedure deviation alone is not mission failure.

| Axis | Accepted result |
|---|---|
| Mission objective | Primary achieved |
| Vehicle | Nominal |
| Procedure | Completed |
| Operator | Timely reference |
| Avionics | Degraded operational |
| Evidence | Complete |
| Overall | Degraded success |

## Frozen reference evidence

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Completed KSB11 session | 2,911,464 | `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4` |
| KTT10 telemetry | 175,232 | `456c512825388b7df1d65c1fa8f08a0c086c4be794c6912cc7e1223cd406e2e1` |
| KPH10 compact history | 32,896 | `cef09c40f95fd75f52ec7a15f8e9db0e12f9d2ffd12b6c107bbc4c6cfb853223` |
| KSR10 summary | 512 | `6aee34461cc0da65b79ba1954a48a6ad90803d29857bf444a53998ae9de622d1` |

The reference transcript contains exactly four accepted action records. The machine-readable companion is [phase12b-completion-audit.json](phase12b-completion-audit.json).

## Authority and role boundary

Live Guided Operator surfaces are filtered in Rust before they cross the C ABI. They expose operational telemetry, estimates, procedures, predictions, action proposals, receipts, and public event history; they do not expose SIM Director truth.

A completed KSB11 is a sealed, role-neutral post-run evidence archive. The bridge returns it only as opaque bytes for hashing, storage, or transfer to the owning Rust verifier. Unreal does not parse canonical KTT10, KPH10, KSR10, or KSB11 internals and cannot use private truth in a live operational role.

## Compatibility retained

Phase 12B is additive:

- Phase 12A ABI-v1 layouts and the original start function remain unchanged.
- The compressed nine-release compatibility session remains exactly 22,369 bytes with SHA-256 `38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8`.
- The accepted 13-entry product catalog remains unchanged, with SHA-256 `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13`.
- No new canonical K-format, physics authority, flight computer, or direct effector command was added.
- Presentation pacing, polling, rendering, and storage remain outside mission identity.
- Unreal opens Rust in Fast execution-capacity mode so bounded Advance requests are honored; Unreal alone schedules wall-clock realtime, pause, single-step, 4x, 16x, and maximum-fast presentation.

## Accepted Unreal product evidence

The product acceptance run qualified:

- bridge ABI major 1, build identity `0x120B0001`;
- `ksa64_viewer_bridge-423c116cf586-120b0001.dll`, 944,640 bytes, SHA-256 `da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565`;
- bridge manifest SHA-256 `b618e31c08b185e40db83955dc47cb8440e488779dfab1f7899307abf9852365`;
- 17/17 `KSA64.Operations` Unreal automation tests in 4.317767143249512 seconds, with report SHA-256 `7d20f060084c3a9071cd8e37fad5ec54158f8fe74f0fc932f49f744e8df9b2f0`;
- a 54-file, 1,033,569,675-byte base package whose game executable SHA-256 is `7d9182c2263d976310a2ac1f96483bb056e332986b3349d878a2c2be0897019c` and which contains no editor-only plugin binaries;
- a packaged full-mission run with exit code 0, exact terminal release 21,591, byte-identical accepted KSB11 evidence, and acceptance-log SHA-256 `d385f265c3385b5fe9624fe4572eb819bfaae4a03e701437031eebd12729b109`; and
- a packaged D3D12/SM6 presentation capture on the pinned RTX 2080 SUPER / 581.15 workstation at 1920x1080, release 6,080, with screenshot SHA-256 `55ea4b4c94a7a50fac29fd4e981197ee53a3e6bc01eb3614959d754f4a687fd0`, semantic SHA-256 `557a4d9a83917f539464818f24d44cd142e8b987dc2eebddea0bc6acda4d6bb3`, and manifest SHA-256 `6c48d17b7ecca8c0f82c4bcd316e88dc2ba9f09aedfde859a1454d78d9921dd8`.

The D3D12 acceptance capture was nonblank, Slate-inclusive, high-contrast, reduced-motion, and rendered at 1.25 text scale with sound cues disabled. It reached release 6,080 after 600 presentation frames and 320 authoritative releases with no queue overflow. Release 6,080 is the screenshot/action checkpoint, not mission termination.

The fixed-60-Hz performance sample ran 120 warmup plus 600 measured frames, advanced exactly 320 releases from 6,144 to 6,464, and measured 258,900 ns p99 and 460,000 ns maximum bridge-service time. Both are below the accepted 1 ms p99 and 2 ms maximum limits. Release 6,464 is the end of this bounded sample, not the mission terminal epoch.

The 30/60/144-Hz automation fixtures prove scheduling, action, release, and KSB11 invariance. They are not a claim that every GPU sustains those display rates. The reported latency covers bridge polling, typed drains, prediction-path service, and advance enqueue work; it is not total GPU frame time or a general renderer benchmark.

The packaged runtime has no dependency on Unreal Editor, MCP, Python, Starter Content, NASA assets, or network services.

## Shutdown and finalization

Clean shutdown of a partial session is not evidence failure. The worker may terminate cleanly while finalization remains `InProgress` and no completed archive exists. `Failed` is reserved for an actual worker or finalization error. A completed session becomes available only after Rust seals the exact evidence archive.

## Reproducing the audit

The non-live default revalidates compatibility and core/bridge evidence without launching Unreal:

```powershell
pwsh -NoProfile -File phase12/complete-phase12b.ps1
```

A fresh full product acceptance run is explicit:

```powershell
pwsh -NoProfile -File phase12/complete-phase12b.ps1 `
  -RunUnrealBuild -RunUnrealAutomation -RunPackage -RunPresentationEvidence `
  -PackageArchive target/phase12b-acceptance-fresh
```

The script composes the complete Phase 12A audit, Phase 12B Rust evidence, both C++ harnesses, Unreal build and automation, packaged full-mission execution, and real-RHI presentation evidence. Live hashes may change with a later commit-qualified bridge or PNG encoding; the frozen values above identify the accepted `423c116` run.

## Handoff boundary

Phase 12C is unblocked. It consumes the accepted role-filtered streams and recordings to build the passive global 3-D engineering viewer. It may not reinterpret Phase 12B dispositions, read private truth through operational roles, parse canonical evidence in Unreal, infer authoritative events from scene state, or move simulation authority out of Rust.

See [PHASE12C_HANDOFF.md](PHASE12C_HANDOFF.md).
