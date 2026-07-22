# Phase 1 completion record

Date: 2026-07-22

Status: complete.

Phase 1 delivers the smallest end-to-end KSA64 vehicle simulation: a deterministic one-dimensional vertical rocket with variable mass, thrust, altitude-dependent gravity and density, quadratic drag, cutoff events, exact telemetry, host tooling, and a C64 post-run display.

## Exit criteria

| Criterion | Evidence | Result |
|---|---|---|
| Analytic special cases pass | Constant velocity, signed constant acceleration, convergence, mass flow, interpolation, force, and transition fixtures run natively and through rust-mos; the target-practical pack also runs under PAL C64 VICE. | Pass |
| Host and C64 exact modes agree | The 2,048-step mission ends at checksum `0x72bf6e0e`; host capture, `mos-sim`, C64 acceptance, and C64 status execution agree on the final raw state. | Pass |
| High-precision comparison exists | 80-digit Decimal semi-implicit Euler isolates fixed-point error; RK4 at 1/32 and 1/64 steps isolates integration error and converges inside declared bounds. | Pass |
| C64 cycle, memory, and accumulated error are reported | Final linked binaries measure 8.34 Hz raw and 5.62 Hz recorded; the screen reports an 80-byte status sink and deltas of -279.355 m / -2.857 m/s. | Pass |

## Final evidence

- Native Rust: 44 tests, formatting, lint, and `no_std` checks pass.
- rust-mos: the exhaustive exact self-test pack passes under `mos-sim-none`.
- C64 acceptance: zero failures under pinned PAL VICE 3.10; PRG size 38,519 bytes.
- Exhaustive C64 diagnostic build: 49,773 bytes.
- Raw checked dynamics: 118,111.48 cycles per step, 8.34 Hz maximum, 5,044.52 cycles or 4.10 percent headroom at 8 Hz.
- Checksum plus canonical telemetry: 175,307.68 cycles per step, 5.62 Hz maximum.
- Canonical telemetry increment: 7,504.00 cycles per step or 59,798.38 cycles per emitted frame.
- Status display: 28,353-byte PRG, 80-byte retained sink, verified directly from C64 screen memory.
- Golden telemetry: 257 frames, 10,312 bytes, stream CRC-32 `0xcf56fe65`.
- Fixed-point minus same-step Decimal: +7.842186 m altitude and +0.042079 m/s velocity.
- Fixed-point minus confirmed RK4: -279.354992 m altitude and -2.856721 m/s velocity.
- RK4 convergence residual: 0.006487 mm altitude and 0.000037 mm/s velocity.

The final common-clock snapshots are `production-timing-v4.json` and `telemetry-timing-v2.json`. Earlier timing files remain as optimization-history evidence because whole-program C64 code layout changed as the final adapters and acceptance runner were added.

## Accepted limits

- The environment is a deliberately simple tabulated learning model, not a standard-atmosphere claim.
- Semi-implicit Euler's accumulated error is accepted for this laboratory and explicitly displayed.
- Raw dynamics fits 8 Hz; per-successor checksum plus canonical telemetry intentionally runs slower than real time.
- The C64 display is post-run. Live refresh cadence has not been measured.
- No claim is made that the fictional vehicle matches a physical launch vehicle.

## Reproduce

From the project root:

    .\phase1\complete.ps1

This reruns the complete correctness, C64 acceptance, timing, high-precision, host-capture, and screen-memory matrix. A passing run ends with:

    PHASE 1 COMPLETION AUDIT: PASS

## Phase 2 boundary

New dynamics now belong to Phase 2. The next phase may add downrange motion, horizontal velocity, pitch guidance, staging, dynamic pressure, and orbital classification, but it must preserve the Phase 1 exact-core, telemetry, validation, and target-measurement boundaries.
