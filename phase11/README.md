# Phase 11: mission operations and programmable flight

Status: complete and accepted.

Phase 11 makes the accepted KSA-G10R simulation operable, programmable, and
replayable without adding or replacing authoritative physics.

## What it can do

- Select a versioned flight-software package before a bounded session.
- Compile mission plans, procedures, faults, roles, and project sources into
  strict binary evidence.
- Stage, validate, acknowledge, commit, cancel, and execute high-level uplinks
  at exact 32 Hz releases.
- Keep the avionics loop alive through a separate ground-communications outage.
- Predict from onboard navigation and from an independent delayed ground
  estimate without reading private truth.
- Run deterministic Observer, Guided Operator, Flight Controller,
  Flight-Software Engineer, SIM Director, and scripted-operator roles.
- Recover missed package-journal records after communications reacquisition.
- Replay an exact action transcript and generate deterministic debrief and
  controlled-counterfactual evidence.
- Execute the reference operations package or the independent limited
  `SafeholdRecoveryV1` package through the profile-specific KLR10 ABI.

## Authority model

```text
world sensors <-> selected flight package <-> actuator commands

operational telemetry <-> Mission Control <-> staged/committed uplinks
```

The first link is the onboard avionics loop. The second is ground operations.
Ground-link loss cannot remove onboard sensing, navigation, guidance, control,
prediction, recovery, or journaling. Operator actions affect the mission only
through the recorded load-validate-commit boundary; direct effector commands
are not permitted.

## Host usage

Run the guided GNSS-loss operations scenario:

```powershell
cargo run -p ksa64-host --bin phase11_mission_control -- phase11/examples/gnss-loss.json
```

Use a different information/authority role:

```powershell
cargo run -p ksa64-host --bin phase11_mission_control -- phase11/examples/gnss-loss.json --role flight-controller
```

Use the headless authoring and evidence SDK:

```powershell
cargo run -p ksa64-host --bin phase11 -- lint phase11/examples/gnss-loss.json
cargo run -p ksa64-host --bin phase11 -- compile phase11/examples/gnss-loss.json target/definition.ksb11
cargo run -p ksa64-host --bin phase11 -- script phase11/examples/gnss-loss.json target/session.ksb11
cargo run -p ksa64-host --bin phase11 -- verify target/session.ksb11
cargo run -p ksa64-host --bin phase11 -- replay target/session.ksb11
cargo run -p ksa64-host --bin phase11 -- debrief target/session.ksb11 target/debrief
```

Source projects use reviewable JSON and exact decimal strings. The C64 consumes
only bounded compiled records; it never parses JSON.

## Stock C64

`SafeholdRecoveryV1` is a 32,857-byte flat endpoint. Its initialized image
ends at `$8858`; the rust-mos static stack extends the runtime footprint to
`$9942`, still below `$C000`. The finite 16-release target probe exactly
matches the host signature `e3c56a95`.

The complete portable `KsaG10rReferenceOpsV1` package uses the authorized
banked stock-RAM stopgap described in
[C64_BANKED_REFERENCE_OPS.md](C64_BANKED_REFERENCE_OPS.md). It requires no REU
and exactly matches 13 native operations in warp-disabled VICE. The accepted
placement is host world plus externally paced C64 flight; no realtime or
physical-link claim is made.

## Audit and handoff

Run the bounded completion audit:

```powershell
powershell -File phase11/complete.ps1
```

Add `-RunVice` to explicitly rerun the two sequential finite target probes.
The audit never starts a complete target mission.

See [COMPLETION.md](COMPLETION.md), [completion-audit.json](completion-audit.json),
[PLAN.md](PLAN.md), and [PHASE12_HANDOFF.md](PHASE12_HANDOFF.md).

KSA64 evidence is engineering-simulation and software-validation evidence. It
is not launch approval, certification, regulatory evidence, or safety
authority.
