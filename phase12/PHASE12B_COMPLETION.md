# Phase 12B completion record

Status: core mission evidence accepted; Unreal presentation acceptance pending.

Date: 2026-07-27

## Outcome

Phase 12B now has a measured, deterministic full-mission reference lane over
the accepted KSA-G10R world and Phase 11 operations package. The scripted
Guided-Operator-equivalent transcript completes at exact release 21,591, or
674.71875 seconds at 32 Hz. This is the accepted reference duration; the
untouched no-action path remains 22,015 releases, or 687.96875 seconds.

The accepted reference run:

- injects and qualifies the persistent coast-phase GNSS loss from transported
  observations;
- performs four recorded operator actions through the ordinary
  Review -> Stage -> Validate -> Commit authority boundary;
- completes physical recovery;
- seals strict KTT10, KPH10, KSR10, action, procedure, prediction, journal, and
  session evidence into one KSB11 archive; and
- classifies the run as **Degraded Success**, not failure merely because its
  avionics state is off nominal.

The independent disposition axes are:

| Axis | Accepted result |
|---|---|
| Mission objective | Primary achieved |
| Vehicle | Nominal |
| Procedure | Completed |
| Operator | Timely reference |
| Avionics | Degraded operational |
| Evidence | Complete |
| Overall | Degraded success |

This is an intentional outcome contract. Mission success, vehicle recovery,
procedure conformance, operator performance, avionics condition, and evidence
integrity are related but not interchangeable.

## Frozen reference evidence

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Completed KSB11 session | 2,911,464 | `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4` |
| KTT10 telemetry | 175,232 | `456c512825388b7df1d65c1fa8f08a0c086c4be794c6912cc7e1223cd406e2e1` |
| KPH10 compact history | 32,896 | `cef09c40f95fd75f52ec7a15f8e9db0e12f9d2ffd12b6c107bbc4c6cfb853223` |
| KSR10 summary | 512 | `6aee34461cc0da65b79ba1954a48a6ad90803d29857bf444a53998ae9de622d1` |

The reference transcript contains exactly four accepted action records. The
machine-readable companion is
[phase12b-completion-audit.json](phase12b-completion-audit.json).

## Authority and role boundary

Live Guided Operator surfaces are filtered in Rust before they cross the C ABI.
They expose operational telemetry, estimates, procedures, predictions, action
proposals, receipts, and public event history; they do not expose SIM Director
truth.

A completed KSB11 is different: it is a sealed, role-neutral post-run evidence
archive. The bridge may return it as opaque bytes so it can be hashed, stored,
or passed back to the owning Rust verifier. Its availability does not authorize
Unreal or another viewer to parse canonical KTT10/KPH10/KSR10 internals or use
private truth in a live role. Role-filtered live presentation and opaque
post-run evidence custody are therefore compatible, not contradictory.

## Compatibility retained

Phase 12B is additive:

- Phase 12A's ABI-v1 layouts and original start function remain unchanged.
- The compressed nine-release compatibility session remains exactly 22,369
  bytes with SHA-256
  `38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8`.
- The accepted 13-entry product catalog remains unchanged.
- No new canonical K-format, physics authority, flight computer, or direct
  effector command was added.
- Presentation pacing, polling, rendering, and storage remain outside mission
  identity.
- Unreal opens the Rust session in `Fast` execution-capacity mode so bounded
  `Advance(n)` requests are honored, while Unreal alone schedules realtime,
  pause, single-step, 4x, 16x, and maximum-fast wall-clock presentation. This
  internal setting is noncanonical, emits no pace evidence, and preserves exact
  KSB11 for identical release and action transcripts.

## Acceptance state

The following evidence is accepted from the current implementation:

- exact full scripted mission completion and hashes above;
- truth-filtered live Guided Operator samples;
- fail-closed role/action validation;
- exact absolute procedure windows;
- the no-action physical recovery path with degraded-success classification;
- the declared conservative-recovery branch and multi-axis outcome contract;
- additive bridge role filtering and full-mission harness implementation.

Phase 12B is **not yet declared fully complete** in this record. These gates
remain to be supplied by the final Unreal acceptance run:

- current native C++ compatibility and full-mission harness results;
- Unreal Editor target build for the Phase 12B operations plugin;
- `KSA64.Operations` automation, including pacing, sparse/burst polling,
  resizing, queue, failure, abort, and shutdown cases;
- packaged full-mission execution without Editor, MCP, Python, network,
  Starter Content, or NASA assets;
- measured 30/60/144 Hz presentation behavior and the declared bridge-frame
  latency budget; and
- final screenshot/semantic presentation evidence and accessibility checks.

No Unreal performance, package, screenshot, or current automation metric is
claimed until those gates run. Preserve the accepted Phase 12A completion files
unchanged; they remain the bridge/toolchain baseline rather than being rewritten
as Phase 12B evidence.

## Reproducing the audit

From the repository root, run the bounded Phase 12B audit:

```powershell
powershell -File phase12/complete-phase12b.ps1
```

Live Unreal work is explicit:

```powershell
powershell -File phase12/complete-phase12b.ps1 -RunUnrealBuild -RunUnrealAutomation -RunPackage
```

The script composes the complete Phase 12A audit, Phase 12B Rust evidence,
the frozen and additive C++ harnesses, and only the Unreal gates explicitly
requested by the caller. Until the pending measurements above are recorded,
a successful non-Unreal run means **core/bridge evidence pass**, not full Phase
12B product acceptance.

## Handoff boundary

Phase 12C may consume the accepted role-filtered operational stream and sealed
session evidence. It may not reinterpret Phase 12B evidence, read private truth
through Guided Operator surfaces, move simulation authority into Unreal, or
quietly absorb unfinished Phase 12B package/performance gates.
