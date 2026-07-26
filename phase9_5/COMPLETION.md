# Phase 9.5 completion record

Status: complete and accepted on 2026-07-25.

Phase 9.5 adds four-surface aerodynamic canards, twelve-jet cold-gas RCS, exact 1/256-second valve events, regulated and blowdown supply tables, changing propellant mass properties, truth-blind pitot fallback, and deterministic priority-residual allocation. The accepted Firestorm-C9, Firestorm-R9, and Firestorm-M9 missions complete through the same exact 32 Hz event executor used by Phase 8.5 avionics. KSA-X1 remains explicitly experimental.

## Accepted evidence

- Every Phase 0–9 regression and frozen artifact remains unchanged.
- Analytic and independent float64 canard/RCS/allocation evidence stays inside the declared 0.5% acceptance bounds; the largest recorded canard vector error is 0.2133%.
- The seed `0x4b534195` 64-case campaign and seven grid/NSGA-II studies reproduce exact archives at one, four, and eight workers.
- The studies perform 2,974 unique robust candidate evaluations. Every accepted finalist passes its 64-case promotion tier; KSA-X1 promotes no accepted physical finalist.
- Passive F1–F7 Mission Control, KMR9 recording, KFE9 browsing, and stock/REU retention cannot change physical or selection checksums.
- The stock browser is 29,010 bytes and ends at `$7951`. The selected-finalist flight endpoint is 39,963 bytes and ends at `$A41A`. Neither requires an REU.
- Finite VICE probes exact-match the baseline plus accepted canard, RCS, and mixed finalists for eight releases each. Every process is closed before the next is launched.

The repeatable audit is [complete.ps1](complete.ps1). Its machine-readable result is [completion-audit.json](completion-audit.json).

## Stock target boundary

The accepted accessible baseline is **host world plus externally paced stock-C64 flight**. The host advances to one exact simulated release, sends truth-blind KLR9 sensor cells, waits for the real C64 flight/allocation kernels, shadow-verifies the response, and only then advances. This is exact hardware-in-the-loop execution, not a wall-clock realtime claim.

The current advanced flight release is 27.65 times the conservative PAL budget, and the portable advanced world endpoint needs 97,707 estimated resident bytes. Phase 9.5 therefore does not claim realtime advanced flight or a stock portable world. It does not lower rates, move allocation to the host, remove effectors, or require an REU to hide those deficits.

Realtime 6502-specific kernels, C64 Ultimate acceleration/integration, and portable C64-world long runs remain priority follow-on tracks. A complete Phase 9.5 C64 mission was not started.

## VICE process discipline

During completion, one VICE process exited before the monitor connected. No simulation code ran and no result was accepted. The unchanged RCS and mixed probes subsequently passed. The audit now enforces a 20-second cooldown between sequential VICE processes in addition to the one-instance and close-on-exit rules.

## Phase 10 boundary

Phase 10 inherits the portable advanced-effector library and the strict ownership rule, but it must add a separately versioned global coordinate/time model rather than stretching local ENU. See [PHASE10_HANDOFF.md](PHASE10_HANDOFF.md).

This evidence supports engineering simulation and software validation only. It is not launch approval, certification, regulatory evidence, or safety authority.
