# Phase 5 stock mission-control replay

Gate 13 turns the strict stock KPH5 history into a bounded post-flight presentation. It does not run the vehicle, sensors, navigation, guidance, or campaign engine.

## Portable replay contract

`replay_phase5_history` first performs the complete KPH5 validation: magic/version/contract identities, reserved-zero bytes, header CRC, exact stream length, payload CRC, ordered points, initial step, and terminal step. It then requires the expected campaign and run identities and reduces the 99 points to:

- final step and quantized ECI position;
- sampled Max-Q and navigation-error maxima;
- event/alarm unions;
- ignition, cutoff, separation, abort, and RCS/gimbal cue counts;
- a deterministic cue-schedule hash.

The accepted nominal result has cue counts `[2, 2, 1, 0, 0]`, event mask `0x0007`, no alarms, and cue hash `0x3b2fb64b`. Native tests reject corruption and identity substitution before presentation.

## Stock C64 page

The C64 binary embeds the reviewed 1,664-byte KPH5 file, invokes the same portable decoder, and renders only after validation. The 40×25 page contains run/point/terminal identity, final quantized spatial position, sampled Max-Q, navigation error, event counts, a Y–Z trajectory projection, source CRC, and cue hash. Event-bearing samples receive a distinct plot marker. The SID registers receive the accepted cue hash only after the whole tape passes.

The projection deliberately uses shifts and fixed reviewed bounds rather than another orbital model. It is presentation, not physical evidence. KST5 and the independent float64 campaign analyzer remain authoritative.

## Evidence

`vice_replay.py` reads all 1,000 VIC-II screen bytes and checks every status row, the plot population, and the final pass marker. `replay.ps1` additionally freezes the complete screen SHA-256, PRG SHA-256, KPH5 SHA-256, linked size, and load end.

- Replay PRG: 6,252 bytes.
- Load end: `$206B`.
- Plot cells: frozen in `replay-v1.json`.
- Source events: two ignitions, two cutoffs, one separation, no abort/RCS fault cue.
- No target mission is executed.
