# Phase 9.5 implementation plan

Status: complete. All twelve gates are accepted; see `COMPLETION.md`.

Master seed: `0x4B534195`.

## Gates

1. Freeze the contract, decisions, source inventory, and compatibility
   baseline; run the complete Phase 0–9 audit.
2. Generate numeric ranges and implement strict KPE9/KPA9/KLE9/KLR9 codecs
   with native, independent, and MOS vectors.
3. Extend the host compiler and create provenance-complete Firestorm-C9,
   Firestorm-R9, Firestorm-M9, and KSA-X1 packs.
4. Implement and validate independent four-surface canard physics.
5. Implement and validate twelve-jet RCS, exact pulse edges, both supply
   tables, depletion, and changing mass properties.
6. Add the truth-blind advanced avionics wrapper and deterministic pitot
   sensing/fallback while preserving KLR8 behavior.
7. Implement and independently verify `PriorityResidualV1` and authority
   handoff.
8. Compose accepted missions, named fault cases, KAT9/KAS9 evidence, and the
   independent float64 comparison.
9. Add the 64-case campaign and the accepted-balanced canard/RCS, routine
   mixed, and experimental KSA-X1 searches.
10. Build both stock split endpoints, prove placement equality, and enforce
    the 24,631-cycle advanced-flight budget.
11. Extend Mission Control, storage, finalist browsing, replay, and selected
    split-endpoint reruns.
12. Run the complete audit and record the Phase 10 handoff.

Each gate is committed only after its relevant exactness, compatibility,
corruption, and validation checks pass.

## VICE policy

Use at most one VICE instance. Close it immediately after success or proven
failure. Do not terminate a valid run merely because it is slow. Begin a
complete target mission only after a fresh duration projection and explicit
user confirmation.
