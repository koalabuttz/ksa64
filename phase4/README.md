# Phase 4: adaptive storage and statistical analysis

Status: implementation in progress.

Phase 4 adds deterministic parameter campaigns, streaming statistics, compact stock-C64 analysis, optional capacity-scaled REU archives, and configurable disk export. An REU is never required for simulation or aggregate results.

The accepted plan freezes a 64-run smoke campaign and a 1,024-run reference campaign with master seed `0x4b534134`. Run zero remains the exact Phase 3 nominal baseline.
Completed implementation gates:

- contracts and exact Phase 3 compatibility;
- deterministic distributions and independent vectors;
- parameterized missions, KSR4, and the frozen 64-run smoke campaign;
- ordered native execution and the independently analyzed 1,024-run reference campaign.

See `DISTRIBUTIONS.md`, `CAMPAIGNS.md`, `FORMATS.md`, `HOST_ANALYSIS.md`, and `STOCK_STORAGE.md` for the frozen evidence and reproduction commands.
