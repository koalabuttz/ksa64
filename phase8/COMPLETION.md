# Phase 8 completion record

Status: accepted on 2026-07-24.

Phase 8 is complete. `HobbySpatialV1` adds geometry-derived mass properties and stability, bounded 6-DOF ascent, layered deterministic wind, recovery drift, strict evidence, independent analysis, and stock-C64 presentation without changing the accepted Phase 0–7 paths.

## Gate record

| Gate | Commit | Accepted result |
|---|---|---|
| Contract | `47a906c` | Spatial profile, coordinates, envelopes, formats, and evidence policy frozen. |
| Numeric/formats | `4f74263` | Generated fixed-point ranges and strict KVP8/KMP8/KMC8/KWP8 codecs accepted. |
| Vehicle compiler | `6d17d9e` | Provenance-backed Firestorm geometry, CG, inertia, and packs accepted. |
| Aerodynamics | `e3cdcbf` | CP, static margin, damping, AoA, and bounded Mach/Cd models accepted. |
| World/wind | `0427298` | ENU translation, rail, quaternions, layered wind, and keyed gusts accepted. |
| Mission/recovery | `89287ec` | Rail-to-landing Firestorm mission and failure cases accepted. |
| External evidence | `681a340` | Float64 and aligned OpenRocket 24.12 evidence accepted. |
| Campaign/storage | `dc81ce9` | KST8/KSR8/KPH8, KSC8/KRA8, keyed campaigns, and adaptive retention accepted. |
| Host/C64 presentation | `6b9570e` | Host exports/plots, seven-page C64 replay, exact trace, and timing accepted. |

## Frozen mission evidence

- KST8: 57,408 bytes, 358 frames, SHA-256 `100f4a6ef498b6c1f263b446d2ee957e8b751dcb4a31ccc95de69c5c177080b6`.
- KSR8: 256 bytes, SHA-256 `848d447ace102e9eecf5f42cc7bef100d32d5b2370206714ec2825962a50c0b2`.
- KPH8: 2,036 bytes, 82 points, SHA-256 `dab361bdfff29fbc06fe6928905f1bbf06d4eb2c587bcd2f3abaf2e8d3306974`.
- Exact mission: 2,244 steps, event history `0x003f`, checksum `0x836accb3`.
- Calm: apogee 754.234009 m, max speed 139.254835 m/s, max Q 11,704.330 Pa, landing T+88.298668 s.
- Crosswind 5 m/s: apogee 742.220093 m, landing distance 234.672485 m, max AoA 14.612576 degrees.

The independent float64 event times stay within one applicable timestep and apogee stays within 0.5 percent. All 19 OpenRocket comparisons pass. OpenRocket reports 745.315 m calm apogee and 232.098 m 5 m/s landing distance; these are engineering comparisons, not launch approval or safety authority.

## Campaign evidence

The 1,024-run reference uses seed `0x4b534138`.

- KSC8 SHA-256: `ceffbd577555928fb159bf1b558c18c247ec25890227a99239cc064475bf24e7`.
- KRA8: 270,400 bytes, SHA-256 `75a96bf482172c4d27990bfa66d01812b64d96a61f239a636ae198e9a796030e`.
- Ordered KSR8 CRC-32: `0xe4792560`.
- One- and four-worker archives are byte-identical.
- All 1,024 cases reach ground contact; no model envelope is exceeded.
- Apogee range: 718.343–792.756 m; maximum landing distance: 100.719 m.

## Stock-C64 evidence

| Program | Bytes | End | Evidence |
|---|---:|---:|---|
| Full mission | 45,754 | `$BAB9` | Built below `$C000`; not run by policy. |
| Exact trace | 46,344 | `$BD07` | 17 states exactly match native execution. |
| Replay | 14,768 | `$41AF` | Seven pages, 82 plot points, screen CRC `0x7b9f7288`. |

The target diagnostic mailbox is at `$C800`, above the rust-mos static stack; moving it there fixed checksum-history corruption discovered by the first finite trace. The trace costs 59,421,528 net PAL cycles for 16 powered steps, or 3,713,845.5 cycles/step. A deliberately conservative 2,244-step projection is 8,458.651 seconds (2.35 hours), above the 30-minute threshold. Therefore no complete C64 mission was started, in accordance with the accepted plan.

The Phase 8 environment links only the exact 0–3,000 m prefix needed by the Firestorm envelope, saving roughly 5 KiB of target data. Values and interpolation are exactly equal to the Phase 7 table within that interval; higher altitude fails closed.

## Compatibility and limitations

The complete Phase 0–7 regression suite passes. `HobbyVerticalV1`, KSA-2A, KSA-5A, and KSA-6R keep their original implementations and artifacts. REU capacity changes retention only, never physics.

The Firestorm model combines published, measured, assumed, and derived data with explicit provenance. Its OpenRocket and float64 comparisons are engineering evidence only—not certification, regulatory, launch-approval, reliability, or safety evidence.
