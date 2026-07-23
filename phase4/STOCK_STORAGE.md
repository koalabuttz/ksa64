# Phase 4 stock-C64 storage gate

The stock configuration is a complete Phase 4 analysis target, not a degraded execution mode. It streams all campaign summaries through the same canonical aggregate and retains five deterministic points of interest:

1. run zero, the Phase 3-compatible baseline;
2. the lowest compact insertion proxy;
3. the highest dynamic-pressure load;
4. the largest navigation position error;
5. the first non-stable compact outcome.

Duplicate selections are filled by the lowest remaining run index. For the frozen 1,024-run campaign the retained indices are `0`, `8`, `96`, `796`, and `1`. The streaming outcome counts are `[857, 166, 1, 0, 0, 0]` and the ordered summary chain is `0x813ce420`.

## KPH4 baseline history

The stock plot records every 32nd simulation step. The accepted baseline contains 226 eight-byte points plus a 64-byte strict header, for 1,872 bytes total. Its CRC-32 is `0x7719f7af`. Header and payload CRCs are independent, reserved bytes must remain zero, and corruption or truncation fails closed.

## Mission-control pages

The target program renders fixed 40 by 25 pages for F1 campaign status, F3 outcome histogram, F5 trajectory and retained-run summary, and F7 storage integrity. A VICE acceptance probe reads all four rendered pages from C64 memory and checks their titles, values, and navigation footer.

The accepted stock PRG is 11,595 bytes and loads through `$354A`, comfortably below the `$C000` stock-RAM link gate. No REU is required to compute aggregates, retain the five summaries, show the UI, or prepare the stock report.