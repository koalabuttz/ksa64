# Phase 11 implementation contract

Status: active.

## Purpose

Phase 11 turns the accepted Phase 10 global simulator into an operable and
programmable mission system without adding authoritative physics. It freezes
the headless contracts later consumed by Mission Foundry:

- profile-specific flight-software packages over KLF6 and KLR10;
- atomic load, validate, acknowledge, and commit commanding;
- compiled mission plans and deterministic procedures;
- onboard-estimate and ground-estimate prediction;
- operational roles and truth isolation;
- immutable mission sessions, exact action replay, and deterministic debriefs;
- a headless JSON authoring and validation SDK.

The accepted world remains KSA-G10R under `GlobalEcef6DofV1`. Existing KLR8,
KLR9, and KLR10 bytes remain frozen.

## Locked architecture

The simulated avionics loop and the simulated ground link are distinct even
when one host/C64 cable carries both:

```text
world sensors <-> flight software <-> actuator commands

spacecraft telemetry <-> ground operations <-> validated uplinks
```

Loss of the ground link cannot remove onboard sensors or actuator control.
Flight software continues the committed plan, predictor, recovery logic, and
event journal.

Ground operations may stage:

- a ground-estimator position/velocity update;
- a bounded target for a declared mission event;
- a contingency branch or navigation-mode selection;
- continue, hold, recovery, abort, or safe requests.

The flight package validates the complete load and returns a receipt. A
separate commit activates an accepted load on an exact 32 Hz release. Direct
effector commanding, partial activation, and arbitrary uploaded programs are
forbidden.

## Flight packages

`KsaG10rReferenceOpsV1` wraps the frozen Phase 10 flight computer. When no
Phase 11 operation is active, it must reproduce the accepted KLR10 command,
status, navigation, and checksum chains exactly.

`SafeholdRecoveryV1` is a separately identified, deliberately small package
for ECI coast, ECEF entry, and local recovery. It is built from one portable
source for host and rust-mos and proves package interchangeability. It is not
a dissimilar safety system and is not engaged during another package's live
session.

Package selection occurs before session start. Live handover is deferred.

## Prediction

The flight package runs a compact one-hertz predictor from its own navigation
estimate and committed plan. Host services create richer paths separately
from transported onboard navigation and from an independent ground estimate.
Only SIM Director may use a truth-seeded counterfactual.

Prediction products always identify their model, source estimate, plan,
package, source epoch, assumptions, and validity.

## Reference operations

The accepted evidence covers:

1. nominal KSA-G10R operations with no command;
2. a deterministic coast GNSS outage and ground state update;
3. a bounded planned guidance-event update;
4. committed and uncommitted loads across a ground-link blackout;
5. invalid, stale, corrupt, late, and incompatible command loads;
6. the bounded `SafeholdRecoveryV1` coast/entry/recovery session.

Human and scripted copies of the same action transcript must reproduce the
same mission, procedure, prediction, journal, and checksum evidence.

## Target policy

Host-world plus externally paced stock-C64 flight remains the hardware
baseline. `SafeholdRecoveryV1` remains a flat image below `$C000`. The
complete portable reference-operations package uses the authorized banked
stock-RAM stopgap documented in `C64_BANKED_REFERENCE_OPS.md`: its main image
ends below `$C000`, while state and cold helpers occupy RAM normally hidden
by I/O and KERNAL. It requires no REU and makes no realtime claim.

The accepted banked gate is stock-CPU/stock-RAM VICE evidence. A validated
physical loader/link, a 6502-specific rewrite, C64 Ultimate acceleration, and
the portable C64 world remain explicit follow-on tracks.

Only one warp-disabled VICE process may run at once. It is closed after success
or proven failure. A complete target mission requires a fresh projection of no
more than 30 minutes and explicit user confirmation.

## Non-goals

- No new atmosphere, gravity, vehicle, subsystem, or entry physics.
- No universal sensor ABI, script language, bytecode VM, dynamic library, or
  in-flight code upload.
- No direct effector commands.
- No live PASS/BFS-style package engagement.
- No claim of certification, calibrated reliability, or dissimilar
  redundancy.

