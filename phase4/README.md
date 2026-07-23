# Phase 4: adaptive storage and statistical analysis

Status: complete. See `COMPLETION.md` for the final audit and `../phase5/README.md` for the handoff.

Phase 4 adds deterministic parameter campaigns, streaming statistics, compact stock-C64 analysis, optional capacity-scaled REU archives, and configurable disk export. An REU is never required for simulation or aggregate results.

The accepted plan freezes a 64-run smoke campaign and a 1,024-run reference campaign with master seed `0x4b534134`. Run zero remains the exact Phase 3 nominal baseline.
Completed implementation gates:

- contracts and exact Phase 3 compatibility;
- deterministic distributions and independent vectors;
- parameterized missions, KSR4, and the frozen 64-run smoke campaign;
- ordered native execution and the independently analyzed 1,024-run reference campaign;
- stock streaming retention, sparse KPH4 plotting, and the bounded mission-control UI;
- preserving REU detection, adaptive KRA4 archives, and the full VICE capacity matrix;
- strict KST4 detailed histories, configurable KXV4 report packs, multi-volume joins, and actual C64 IEC export.

See `DISTRIBUTIONS.md`, `CAMPAIGNS.md`, `FORMATS.md`, `HOST_ANALYSIS.md`, `STOCK_STORAGE.md`, `REU_STORAGE.md`, and `EXPORT.md` for the frozen evidence and reproduction commands.
