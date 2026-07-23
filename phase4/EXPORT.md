# Phase 4 interactive analysis and export gate

Gate 7 completes the stock/REU-neutral campaign browser and the strict export path. Export is post-run work: no IEC operation, volume retry, archive failure, or UI action can alter mission state, campaign aggregation, or later run seeds.

## Interactive mission-control UI

The 40 by 25 UI uses the same four bounded pages on stock hardware and every REU tier:

- F1: campaign outcomes and streaming metric statistics;
- F3: outcome histogram;
- F5: trajectory, retention reasons, and retained-run browsing;
- F7: storage identity, integrity, and export readiness.

Cursor keys wrap through retained runs. Return opens and closes a compact drill-down. The browsing state is storage-neutral, so an REU changes the retained set rather than the controls or physics. The target VICE gate injects F3, F5, cursor-right, Return, and F7 through the C64 keyboard buffer and validates the visible screen after every transition.

The accepted interactive stock PRG is 12,864 bytes and loads through `$3A3F`, below the `$C000` stock-RAM gate.

## KST4 detailed history

KST4 now has a strict 96-byte codec rather than only a reserved identity. Its header binds:

- the Phase 4 and Phase 3 telemetry contracts;
- campaign CRC, run index, derived sensor seed, and variation CRC;
- frame count, frame byte count, and payload CRC;
- first/final steps and all four final checksum chains.

Every payload record is independently parsed with the unchanged 160-byte KST3 frame semantics. The frozen run-zero example has 906 frames and is 145,056 bytes. It ends at step 7,200 with truth/sensor/navigation/flight checksums `c86045a0`, `47d11fb0`, `c6f9da7b`, and `02ce28ef`.

## Configurable report construction

`ExportManifest` independently selects configuration, aggregate, sorted campaign-summary ranges, compact KPH4 run IDs, and full KST4 run IDs. Duplicate histories, out-of-range runs, missing source histories, cross-campaign identities, malformed histories, and overflow are rejected before output. One-volume mode rejects a selection above its payload limit before allocating or writing; multi-volume mode splits only at explicit logical offsets.

The default limit is 160 KiB. The stock report retains configuration, the frozen aggregate, summaries for runs `0`, `8`, `96`, `796`, and `1`, plus the baseline compact history. The KRA4 payload is 3,712 bytes; its KXV4 volume is 3,776 bytes. The selection-manifest CRC is `0xcc21d093`.

## KXV4 and disk evidence

Each 64-byte KXV4 header binds archive CRC, selection CRC, volume index/count, logical offset/length, payload length, and payload CRC. The strict joiner rejects missing, duplicated, reordered, mixed-identity, truncated, or corrupt volumes before producing output.

`export.ps1` validates both:

- the one-volume stock report on a standard 35-track D64 image;
- a synthetic 3,000-byte archive split into three 1,000-byte payload volumes, each written to and read from a separate D64 image.

The gate also proves missing, reordered, corrupt, oversized one-volume, and disk-full cases fail closed. The host artifacts carry SHA-256 sidecars.

## Actual C64 IEC exporter

`ksa64-phase4-export-c64` is a separate 4,280-byte utility PRG. It writes the stock KXV4 bytes as a sequential file through the pinned LLVM-MOS Commodore KERNAL library, closes the data channel, and validates the device-8 command channel. Its VICE gate boots the utility from the same D64 being modified, extracts the resulting file, and compares all 3,776 bytes with the host-generated KXV4 source. A nearly full D64 produces a nonzero target-visible failure instead of a false success.

Device 8 and 160 KiB are defaults, not campaign inputs. User-port export remains Phase 6 work.

## Reproduction

```powershell
phase4/export.ps1
phase4/export-c64.ps1
phase4/stock.ps1
cargo test -p ksa64-host --test phase4_export
cargo test -p ksa64-sim --test phase4_detail --features fixtures
```