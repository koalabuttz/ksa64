# Phase 11.5 compatibility baseline

The frozen baseline is commit:

```text
20abf4ccf44074523c511cb7399505a1bd805416
```

Validation performed before Phase 11.5 source changes:

- The complete Phase 0-10 native and stored-evidence audit passed.
- The complete Phase 11 native, prediction, mission SDK, replay, corruption,
  and stored-evidence audit passed.
- The rust-mos target-only audit passed with Docker access.
- `SafeholdRecoveryV1` remained a 32,857-byte flat stock image ending below
  `$C000`.
- The banked reference-operations bundle remained 55,423 bytes with SHA-256
  `cb1979fd4e5abf5c26bfbc71ef031ed4d4e0c2d2a9eee4e0522fdc306d9ab377`.
- No VICE process or complete target mission was started.

The first sandboxed target attempt was unable to access Docker Desktop's named
pipe. The isolated target-only rerun with Docker permission passed; this was an
environment permission failure, not a KSA64 failure.
