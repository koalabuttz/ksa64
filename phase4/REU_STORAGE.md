# Phase 4 adaptive REU storage gate

The REU is optional. `ReuPreference` supports automatic use, a user-requested disable, or a cap that can only reduce detected capacity. `StoragePlan` deterministically assigns up to one quarter of capacity to canonical 128-byte KSR4 summaries, returns unused summary space, then fits exact-size KST4 histories followed by KPH4 histories.

## Preserving detection

The target detector first proves that DMA is present, then preserves one byte from every logical 64 KiB bank. It writes descending bank signatures, counts the non-aliased banks in ascending order, and restores all saved bytes. This handles the mirror layouts of 128 KiB through 16 MiB devices without claiming nonexistent capacity. Failure falls back to stock mode and is surfaced as a storage alarm; overrides never enlarge the detected result.

The VICE acceptance matrix covers no REU plus 128, 256, and 512 KiB and 1, 2, 4, 8, and 16 MiB. Every case performs the detector twice around seeded bytes and verifies those bytes survived.

| REU | summaries | full KST4 | compact KPH4 |
|---:|---:|---:|---:|
| none | 5 | 0 | 1 stock plot |
| 128 KiB | 256 | 0 | 13 |
| 256 KiB | 512 | 1 | 6 |
| 512 KiB | 1,024 | 2 | 14 |
| 1 MiB | 1,024 | 6 | 6 |
| 2 MiB | 1,024 | 13 | 10 |
| 4 MiB | 1,024 | 28 | 0 |
| 8 MiB | 1,024 | 56 | 18 |
| 16 MiB | 1,024 | 114 | 14 |

Counts include the frozen 906-frame detailed stream, 901-point REU compact history, record headers, superblock, aggregate, and footer allowance. The 4 MiB result has no compact remainder because the required allocation order fits 28 full histories first.

## KRA4 and DMA

KRA4 uses a CRC-protected superblock and append-only 32-byte record headers. A payload is written behind an uncommitted header; only a final header rewrite marks it committed. A footer binds the final record count, logical length, chain, and campaign. Interrupted writes leave the preceding prefix valid, while committed corruption is rejected at the first bad record.

The same archive interface has stock-slice, host-file, and explicit C64 REU-DMA transports. The C64 probe separately measures 32 transfers of 64, 160, and 256 bytes with CIA interrupt sources disabled. The measured matrix is frozen in `reu-matrix-v1.json`; these costs are storage evidence and never enter simulation state or campaign checksums.