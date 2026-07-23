# Phase 5 adaptive history and storage

Gate 12 adds a compact spatial history and capacity-scaled retention without making an REU a requirement or a source of simulator state.

## KPH5 compact history

KPH5 is a strict, independently verifiable presentation artifact. Its 80-byte header binds the Phase 5 numeric/scenario contracts, campaign and run identity, sampling stride, point count, terminal step, payload CRC, and header CRC. Each 16-byte point contains:

- mission step;
- three ECI position components quantized to one quarter kilometre;
- dynamic pressure in one-sixteenth kPa;
- a conservative maximum-axis navigation error in quarter kilometres;
- event and alarm masks.

The stock baseline samples every 32 mission steps and always retains the initial and terminal states. It therefore needs 99 points and 1,664 bytes for the accepted 3,133-step nominal mission. KPH5 is noncanonical presentation data: KST5 remains the authoritative detailed stream.

## Stock baseline

A stock C64 retains the streaming campaign aggregate, exactly five deterministic summaries, and one sparse baseline KPH5 history. The five summaries are selected in this order, omitting duplicates and filling with the lowest run indices:

1. baseline;
2. worst perigee/insertion proxy;
3. highest dynamic pressure;
4. greatest navigation error;
5. first non-stable outcome.

For the frozen 256-run campaign the result is `[0, 1, 4, 53, 2]`. Recording and retention run only through observer/sink boundaries. The recorded and unrecorded nominal mission summaries and checksum chains are exactly equal.

## REU capability ladder

Phase 5 reuses the preserving Phase 4 REU detector, manual disable/cap controls, and explicit DMA driver. Gate 12 adds a Phase 5-specific plan and append-only KRA5 archive. Up to 25% of detected capacity is reserved for the 160-byte KSR5 summary stream; unused summary capacity immediately returns to history storage. Remaining capacity holds complete KST5 histories first, then stride-8 KPH5 histories. History rerun selection is baseline, worst insertion, worst load, worst navigation error, first failure, then the lowest unused indices.

The frozen 256-run plan, using the observed 3,134-frame nominal KST5 size and 393-point REU KPH5 size, is:

| REU | Summaries | Full KST5 | Compact KPH5 |
|---:|---:|---:|---:|
| none | 5 | 0 | 1 stock plot |
| 128 KiB | 204 | 0 | 15 |
| 256 KiB | 256 | 0 | 34 |
| 512 KiB | 256 | 0 | 75 |
| 1 MiB | 256 | 0 | 157 |
| 2 MiB | 256 | 1 | 113 |
| 4 MiB | 256 | 3 | 25 |
| 8 MiB | 256 | 6 | 58 |
| 16 MiB | 256 | 12 | 123 |

Exact byte counts are frozen in `history-evidence-v1.json`. The first VICE pass exposed a rust-mos C64-target divergence in general quotient-based allocation at 128 KiB even though native and `mos-sim` agreed. The accepted planner uses bounded addition loops (at most 256 iterations) and now agrees on all three execution paths. The non-monotonic compact count is intentional: complete KST5 streams receive priority and each consumes approximately 1.33 MB.

## KRA5 recovery

KRA5 is independent of KRA4 and binds a new Phase 5 contract. Records are written payload-first, then committed by a CRC-protected header. The superblock is updated only after a record commits. A failed write leaves the preceding prefix readable; corruption is rejected at the first invalid record. The existing C64 REU transport implements both archive interfaces, so no duplicate DMA path was introduced.

## Evidence

- Native tests freeze the KPH5 codec, corruption rejection, exact observer neutrality, stock selection, all capacity plans, KRA5 commit/recovery, and manual REU disable/capping.
- `verify_history.py` independently parses the frozen KPH5 and KSR5 bytes, reconstructs selection and storage allocation, and checks both CRC layers.
- A finite rust-mos probe freezes signature `0xb5783bf2`; its size-optimized image is 5,917 bytes.
- A separate 4,491-byte PAL VICE probe detects no REU and every 128 KiB–16 MiB tier, preserves pre-existing bytes, publishes its result marker last, and matches the independent allocation table exactly.
- No complete target mission or target campaign is part of this gate.
