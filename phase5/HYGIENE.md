# Phase 5.1 hygiene record

This bounded post-completion pass changes no simulator model, numeric contract,
mission result, campaign artifact, or timing decision.

- Phase 4 and Phase 5 REU probes publish completion magic only after all result
  fields; the corrected Phase 4 image is 4,470 bytes and remains stock-compatible.
- Phase 4 export checks compare bytes with Windows PowerShell 5.1-compatible
  bounded loops.
- Canonical JSON, Rust, and SHA-256 manifests use LF on every host.
- The pinned rust-mos wrapper persists Cargo downloads only under the ignored
  project-local toolchain cache.
- Target probe publication discipline is documented as an evidence contract.
- Routine Phase 4 REU validation writes transient timing output under `target/`;
  replacing frozen evidence requires explicit `-Update`.

Acceptance requires the affected Phase 4 REU and export gates plus the complete
Phase 5 audit. No complete target mission or campaign is part of this pass.

## Acceptance results

- Windows PowerShell 5.1 parsed every affected wrapper.
- Phase 4 REU validation passed without an REU and at 128 KiB, 256 KiB,
  512 KiB, 1 MiB, 2 MiB, 4 MiB, 8 MiB, and 16 MiB.
- Phase 4 stock, multi-volume, C64 IEC, corruption, ordering, missing-volume,
  and disk-full export checks passed.
- The complete Phase 5 audit passed all native tests, target probes, adaptive
  history tiers, replay, stable three-run timing, and 30 frozen SHA-256
  sidecars.
- The persistent rust-mos Cargo cache was reused by later target builds.
