# Post-Phase-10 handoff

Phase 10 closes the local/global coordinate overlap. The next work should make
the accepted simulator easier to program, operate, author, and understand
before expanding physical scope again.

## Next planned phase

Phase 11 is **Mission Operations and Programmable Flight**:

- formalize a versioned flight-software package envelope over the existing
  profile-specific KLR contracts;
- add estimate-based onboard and ground trajectory prediction;
- add mission-plan, operator/uplink, procedure, action-log, session-bundle, and
  deterministic-debrief contracts;
- add role-based operations without changing the authoritative run;
- provide the headless mission-authoring SDK and derivation ledger consumed by
  the later graphical workbench.

Phase 11 adds no new authoritative physics. It should first reuse KSA-G10R for
a nominal and deterministic GNSS-loss operations scenario.

## Following phase

Phase 12 is the host-only **Mission Foundry and passive 3-D operations
viewer**. It borrows approachable vehicle assembly, procedural geometry,
staging, planning, and visual authoring ideas from spaceflight games while
remaining distinct:

- editable sources compile into immutable identified KSA64 packs;
- vehicle connections, provenance, assumptions, envelopes, and authority are
  visible rather than magical;
- mission lanes distinguish world, flight-software, ground, and SIM Director
  authority;
- Sketch, Evaluated, and Frozen Candidate states describe evidence maturity;
- the stock C64 executes compiled packs and replay products rather than the
  graphical authoring environment.

The full 3-D editor does not block the Phase 11 operational contracts.

## Parallel stock-C64 engineering

- profile the global and advanced flight hot kernels;
- evaluate a deliberate 6502-specific rewrite;
- evaluate C64 Ultimate acceleration without changing canonical behavior;
- restore a portable C64-world long-run role;
- accept physical user-port, ACIA, or Ethernet transports.

The Phase 11 package ABI should make target-specific flight implementations
comparable without creating another simulator. This track remains important
but does not block the host-authoring phases.

## Next physical mission

After the operations and authoring layers are stable, select one concrete
sustained Earth-orbital spacecraft experiment. Only that mission may lock the
scope of bounded electrical, thermal, communications, propulsion,
degradation/redundancy, sustained propagation, tracking, deorbit,
`SixAxisWrenchV1`, rendezvous, or docking work.

Do not begin with a universal spacecraft-systems graph.

## Preserved rules

- One model owns state in an interval.
- The portable deterministic evaluator remains authoritative.
- External tools produce frozen validation fixtures, never runtime corrections.
- Flight software, procedures, and operational predictors never read private
  truth.
- Operator actions are explicit, epoch-tagged, authority-checked, and
  replayable.
- Prediction model and initial-estimate source are always identified.
- Editable authoring projects are sources; compiled packs and bound session
  artifacts identify actual runs.
- Stock, REU, host, VICE, and physical-C64 placement cannot change physics.
- Long target runs require a fresh projection and explicit confirmation.
- Added fidelity receives a new identity and never mutates frozen artifacts.
- Evidence maturity is not certification or calibrated real-world reliability.
