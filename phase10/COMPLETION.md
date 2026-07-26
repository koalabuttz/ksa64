# Phase 10 completion record

Status: complete and accepted on 2026-07-26.

Phase 10 delivers the separately versioned `GlobalEcef6DofV1` world, exact
frame ownership events, compiled Earth/time data, a truth-blind global flight
computer, a complete controlled KSA-G10R mission, KSA-5A orbital
corroboration, deterministic campaigns, passive Mission Control, and bounded
stock-C64 flight/replay endpoints.

## Accepted outcome

- Every Phase 0-9.5 regression and frozen artifact remains unchanged.
- UTC/TAI/TT/UT1 conversion, leap handling, EOP coverage, WGS 84 geodesy,
  ECEF/GCRF orientation, and ENU/ECEF/GCRF round trips pass.
- The world changes owner only at exact 32 Hz releases and preserves state
  continuity without resetting the onboard estimate from truth.
- The controlled KSA-G10R mission crosses all four frame boundaries, reaches
  210.897 km apogee, travels 336.169 km downrange, and completes recovery.
- The independent uninstrumented float64 model passes the accepted trajectory,
  attitude, transition, and event tolerances.
- The KSA-5A one-orbit handoff remains inside 5 km and 5 m/s.
- All 256 completion cases recover with zero numeric/frame/time faults, and
  archives are byte-identical across one, four, and eight workers.
- The stock flight, timing, and replay programs fit below `$C000` without an
  REU and pass finite warp-disabled VICE evidence.

The repeatable audit is [complete.ps1](complete.ps1). Machine-readable results
are in [completion-audit.json](completion-audit.json).

## Important audit correction

The final independent-model review found that the earlier physical reference
did not cover the entire global mission and that two event qualifications were
too sensitive to coordinates:

- apogee now uses velocity projected on the current WGS 84 geodetic up vector;
- main deployment after the final handoff uses recovery-site local AGL rather
  than ellipsoidal altitude.

The accepted Python reference now independently propagates the complete
uninstrumented local/ECEF/GCRF/entry/recovery path and fails on tolerance
drift. Generated nominal, campaign, report, replay, and target evidence was
regenerated after the correction.

The independent landing time differs by 0.09375 s. Phase 10 therefore records
a separate four-recovery-step terminal-contact tolerance, while all flight
events still meet the one-step criterion. See [VALIDATION.md](VALIDATION.md).

## Stock target boundary

Host world plus externally paced stock-C64 flight is the accessible Phase 10
hardware baseline. The host delivers an exact sensor release, waits for the
real C64 flight kernel, shadow-verifies returned cells, and then advances the
world. This preserves logical time and successor-command semantics while wall
time pauses.

The endpoint is 37,403 bytes and ends at `$9A1A`. The worst measured transition
release costs 3,512,697 PAL cycles, or 114.1 nominal release slots. Phase 10
therefore makes no realtime-C64 claim. The 17,002-byte replay and 35,247-byte
timing program also fit stock memory. No complete target mission was started.

Portable C64-world work, a measured 6502-specific rewrite, C64 Ultimate
acceleration/integration, and physical user-port/ACIA/Ethernet acceptance
remain open.

## Audit policy

The default audit:

- runs the frozen legacy audit;
- checks formatting, clippy, workspace tests, generators, independent
  analysis, campaign integrity, and stored target evidence;
- rebuilds and hashes the three stock artifacts;
- does not launch VICE or a complete C64 mission.

`-RunVice` explicitly requests the finite replay, release-class, transition,
and timing probes. It allows only one VICE process, disables warp, closes the
process after success or proven failure, and observes the 20-second cooldown.

## Handoff

The next mission phase should select one concrete objective rather than merely
adding fidelity. Candidate tracks are documented in
[NEXT_PHASE_HANDOFF.md](NEXT_PHASE_HANDOFF.md).

These results are engineering-simulation and software-validation evidence.
They are not launch approval, certification, regulatory evidence, or safety
authority.
