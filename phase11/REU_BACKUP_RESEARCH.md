# Future REU package-overlay and backup research

Status: deferred; not Phase 11 implementation.

Phase 11 selects one flight package before session start. A future target
engineering study may investigate whether an REU provides useful package
extensibility or bounded live handover.

An REU is storage and DMA hardware, not directly executable 6510 address
space. Any secondary package must be copied into an executable main-RAM
overlay before use. The study must therefore measure and define:

- inactive image and persistent-state storage layout;
- DMA latency, interrupt policy, and safe release boundaries;
- a versioned common handoff state independent of private implementation layouts;
- package and state CRC validation before control authority changes;
- atomic commit, rollback, and recovery after partial DMA;
- backup-state freshness and the feasibility of shadow execution on one CPU;
- behavior across reset, power fault, corrupt REU contents, and missing REU;
- interaction with C64 Ultimate RAM and acceleration;
- whether one CPU plus storage meaningfully improves extensibility without
  being represented as hardware redundancy.

No future implementation may silently require an REU, change canonical
physics, lower release rates, or call an overlay a dissimilar backup system.

