# Phase 12C completion record — global mission visualization and renderer parity

Status: **draft; completion not yet claimed**.

Date: 2026-07-28

Entry commit: `eb666cbaf3b8950218656a7ad7fe135b05385813`

This record is prepared alongside the implementation so the remaining
acceptance work is explicit. A source implementation, passing unit tests, or a
working screenshot does not by itself complete Phase 12C. The phase becomes
complete only after `complete-phase12c.ps1` passes every non-skipped gate with
the explicit Unreal, packaged, rendered-browser, and runtime/parity evidence
enabled.

## Implemented scope awaiting completion audit

The Phase 12C source tree contains the intended additive boundaries:

- renderer-neutral `GlobalDisplayV1` definitions, samples, source poses, path
  chunks, transitions, replay indices, cursor state, and exact range requests;
- capability-gated KPS1 1.0 message kinds that remain invisible to legacy
  clients;
- a separately discoverable, size-tagged `GlobalDisplayApiV1` C function table
  that does not change any ABI-v1 symbol or structure;
- Rust-owned ENU/ECEF/GCRF resolution, role filtering, source identity,
  continuity, exact-event classification, and event-preserving path levels;
- strict nominal Phase 10 and guided GNSS-loss replay adapters;
- additive broker and WebAssembly publication over the same normalized display
  products;
- a procedural Unreal global-viewer plugin that consumes the existing
  operations bridge rather than opening a second authority;
- a Babylon/React viewer using the same semantic display contract with WebGPU,
  WebGL2, and complete 2-D fallback lanes; and
- deterministic semantic scene snapshots intended for cross-renderer
  comparison.

This list describes implementation inventory, not accepted runtime evidence.

## Frozen evidence boundary

Phase 12C may not rewrite the evidence it visualizes:

| Evidence | Frozen value |
|---|---|
| Product catalog | 13 entries |
| Catalog SHA-256 | `b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13` |
| Nominal KTT10 SHA-256 | `a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a` |
| Nominal KPH10 SHA-256 | `cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2` |
| Nominal KSR10 SHA-256 | `9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa` |
| Nominal releases | 22,015 |
| GNSS-loss releases | 21,591 |
| GNSS-loss accepted actions | 4 |
| GNSS-loss KSB11 bytes | 2,911,464 |
| GNSS-loss KSB11 SHA-256 | `7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4` |

The nominal planned/reference path and exact current re-execution are distinct,
labelled lineages. Their fail-closed treatment is recorded in
[PHASE12C_NOMINAL_COMPATIBILITY.md](PHASE12C_NOMINAL_COMPATIBILITY.md). This
exception preserves both the frozen artifacts and current checked-in physics;
it is not a general tolerance for future drift.

## Completion-gate ledger

No pending row may be inferred as passed from a related test:

| Gate | Status | Evidence |
|---|---|---|
| Frozen Phase 12B.5 entry hashes | Pending final audit | `complete-phase12c.ps1` |
| Formatting and warnings-denied Clippy | Pending final audit | — |
| Complete native workspace suite | Pending final audit | — |
| GlobalDisplay Rust/C++/TypeScript vectors | Pending final audit | — |
| Legacy KPS1 1.0 and ABI-v1 exactness | Pending final audit | — |
| Broker, LAN, reconnect, and role isolation | Pending final audit | — |
| WebAssembly capability and exact-evidence lanes | Pending final audit | — |
| Nominal 22,015-release lineage/replay | Pending final audit | — |
| GNSS 21,591-release exact session/replay | Pending final audit | — |
| Unreal Editor target | Pending final audit | — |
| `KSA64.Phase12C` automation | Pending final audit | — |
| Packaged Win64/D3D12 bridge smoke | Pending final audit | — |
| Packaged launch/coast/entry/recovery/landing visuals | Pending runtime capture | — |
| Babylon WebGPU rendered lane | Pending runtime capture | — |
| Babylon forced-WebGL2 rendered lane | Pending runtime capture | — |
| Babylon complete 2-D fallback | Pending runtime capture | — |
| Context-loss fallback | Pending runtime capture | — |
| Unreal/Babylon semantic parity | Pending runtime capture | — |
| Render/action/evidence invariance | Pending runtime capture | — |
| Phase 0–12B.5 frozen regressions | Pending final audit | — |

The physical Duet, Vita3K, and physical Vita Phase 12B.5 qualifications remain
a separate open workstream. Their absence neither completes nor invalidates
Phase 12C.

## Runtime and package metrics

These are deliberately blank until source-bound evidence exists. `Pending`
means unmeasured, not zero.

| Measurement | Required threshold | Recorded result |
|---|---:|---:|
| Unreal procedural tier | 1920×1080 at 60 fps | Pending |
| Display publication p99 | ≤ 1 ms | Pending |
| Bridge polling p99 | ≤ 1 ms | Pending |
| Babylon WebGPU | responsive ≥ 30 fps | Pending |
| Babylon WebGL2 | responsive ≥ 30 fps | Pending |
| 2-D fallback | fully operational | Pending |
| Unreal package size | report only | Pending |
| Web production bundle size | report only | Pending |
| Nominal replay memory | report only | Pending |
| Exact active-window path memory | report only | Pending |
| Renderer origin changes | report count and continuity | Pending |

## Completion command

The default command runs the portable, contract, exact replay, bridge, and web
gates but deliberately reports only a portable/contract pass:

```powershell
./phase12/complete-phase12c.ps1
```

The final completion invocation must also supply explicit visual/runtime
evidence:

```powershell
./phase12/complete-phase12c.ps1 `
  -RunUnrealBuild `
  -RunUnrealAutomation `
  -RunPackage `
  -RunBrowserEvidence `
  -BrowserEvidenceManifest <rendered-browser-evidence.json> `
  -NativeHarnessEvidenceManifest <native-global-display-harness.json> `
  -UnrealEvidenceManifest <packaged-unreal-global-evidence.json> `
  -RuntimeEvidenceManifest <strict-cross-renderer-evidence.json>
```

No audit invocation may translate skipped or unavailable visual gates into a
completion claim.

### Evidence integrity rule

The final runtime manifest must be the deterministic output of
`compare-phase12c-renderers.mjs`. The completion script reruns that comparator from
the actual native C++ harness JSON, packaged Unreal manifest/semantic files/screenshots,
and rendered-browser raw records/screenshots, then requires an exact SHA-256 match
with the recorded manifest. All three producers must bind the same clean source
commit. The accepted record contains nine nominal milestones and six GNSS operations
milestones (outage onset/qualification and four accepted action epochs); aggregate
`pass` booleans alone are never evidence.

## Limitations retained

- Renderers remain passive and may not derive frames, events, outcomes,
  actions, or canonical evidence.
- SIM truth is absent from non-director products and hidden by default for a
  director.
- Procedural engineering visuals are not production art.
- NASA imagery, terrain, production meshes, Niagara, Lumen, Nanite, installers,
  signing, and store distribution remain outside Phase 12C.
- Visual agreement is numerical/presentation evidence, not launch approval,
  certification, regulation, or safety authority.

The Phase 12C.5 handoff remains a draft until every completion gate above is
accepted.
