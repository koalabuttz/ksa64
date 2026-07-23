# Phase 4 completion record

Status: complete. KSA64 now has deterministic parameter campaigns, strict run summaries, independently analyzed reference evidence, stock-C64 streaming analysis, optional capacity-scaled REU archives, interactive retained-run browsing, strict detailed histories, and validated one- or multi-volume disk export. No REU is required for campaign execution, aggregate results, the stock UI, or the default report.

## Exit criteria

| Criterion | Evidence | Result |
|---|---|---:|
| Phase 3 behavior is unchanged when recording is disabled | Run zero reproduces truth `0xc86045a0`, sensor `0x47d11fb0`, navigation `0xc6f9da7b`, flight `0x02ce28ef`, and frozen KST3 CRC `0xaf79b36e`. | Pass |
| Campaigns are deterministic across execution order | Serial, 5-worker, and 12-worker executions produce identical KSC4/KSR4 bytes; the 64-run smoke campaign repeats exactly. | Pass |
| Statistical results are independently analyzed | The Python analyzer reconstructs every draw and seed without Rust, validates all 1,024 summaries, and computes authoritative float64 orbital/load/navigation results. | Pass |
| Stock mode is complete without an REU | Streaming aggregate, five retained summaries, 1,872-byte KPH4 plot, four-page UI, browsing, drill-down, and report export pass. | Pass |
| REU transfers are explicit, preserving, and bounded | No-REU plus 128 KiB through 16 MiB VICE matrix passes twice-preserving detection, exact capacity plans, DMA ordering, archive commit, and recovery tests. | Pass |
| Archives and exports fail closed | KST4, KRA4, and KXV4 reject corruption, truncation, identity mismatch, incomplete archives, missing/duplicate/reordered volumes, oversize selections, and disk-full writes. | Pass |
| C64 export is real rather than host-only | The separate IEC utility writes 3,776 exact KXV4 bytes to a D64 sequential file, validates the command channel, and reports a nonzero disk-full failure. | Pass |

## Frozen campaign evidence

The reviewed campaign uses master seed `0x4b534134` and 1,024 runs. Campaign identity remains `0xa2e9e9d5`; the ordered KSR4 chain remains `0x813ce420`.

| Measurement | Result |
|---|---:|
| Compact C64 outcomes | 857 stable, 166 suborbital, 1 impact |
| Authoritative float64 outcomes | 934 stable, 89 suborbital, 1 impact |
| Float64 perigee range | -4.843 to 193.631 km |
| Float64 apogee range | 188.065 to 256.840 km |
| Max-Q range | 39.906 to 43.896 kPa |
| Proper-acceleration range | 54.611 to 55.634 m/s2 |
| Cutoff navigation position error | 0.488 to 62.744 m |
| Current 12-worker release execution | 4.368 s campaign time |
| Accepted serial execution | 30.580 s |

The compact fixed-point classifier exists for deterministic selection and C64 presentation. It is deliberately not treated as physical acceptance evidence; the independent float64 analyzer is authoritative.

## Storage capability

| Hardware | Retained summaries | Full KST4 | Compact KPH4 |
|---:|---:|---:|---:|
| Stock | 5 | 0 | 1 |
| 128 KiB REU | 256 | 0 | 13 |
| 256 KiB REU | 512 | 1 | 6 |
| 512 KiB REU | 1,024 | 2 | 14 |
| 1 MiB REU | 1,024 | 6 | 6 |
| 2 MiB REU | 1,024 | 13 | 10 |
| 4 MiB REU | 1,024 | 28 | 0 |
| 8 MiB REU | 1,024 | 56 | 18 |
| 16 MiB REU | 1,024 | 114 | 14 |

The final PAL VICE matrix measured 32-transfer totals of 12,713-13,056 cycles for 64-byte DMA, 16,087-16,386 cycles for 160-byte DMA, and 19,417-19,461 cycles for 256-byte DMA. Normal VIC bus phase changes exact totals slightly; sizes remain strictly ordered and never enter campaign state.

## Target memory and programs

Linked writable-section figures exclude the hardware stack and include initialized data, BSS/no-init storage, plus zero-page separately.

| C64 program | PRG bytes | Loaded end exclusive | Writable static | Zero page |
|---|---:|---:|---:|---:|
| Interactive stock UI | 12,864 | `$3A3F` | 1,490 bytes | 17 bytes |
| REU/DMA probe | 4,470 | `$1975` | 897 bytes | 17 bytes |
| IEC export utility | 4,280 | `$18B7` | 0 bytes | 0 bytes |

All three fit stock RAM. The stock program's no-init end is `$3C21`, still far below the `$C000` gate.

## Target campaign decision

The accepted Phase 3 composed-path measurement projects one 7,200-step PAL mission at 14,621.3 seconds, or 243.7 minutes. Using that conservative per-run projection:

- 64 target runs project to approximately 10.8 days;
- 1,024 target runs project to approximately 173.3 days.

These exceed the locked 30-minute target-run threshold. No full C64 campaign was started or canceled. Finite exactness, storage, DMA, UI, and IEC probes are the accepted target evidence.

## Completion audit

The final audit passed:

```powershell
cargo test --workspace --features fixtures
python -B phase4/reference/generate_distributions.py --check
python -B phase4/reference/analyze_campaign.py --ksc phase4/examples/ksa4-reference.ksc4 --ksr phase4/examples/ksa4-reference.ksr4 --output phase4/reference-campaign-analysis.json --check
phase4/stock.ps1
phase4/reu.ps1
phase4/export.ps1
phase4/export-c64.ps1
```

Phase 4 optimization remained limited to measured storage and presentation paths. No frozen physical result or Phase 3 artifact changed.