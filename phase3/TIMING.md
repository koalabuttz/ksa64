# Phase 3 C64 timing and replay

The Phase 3 target gate uses finite, naturally terminating 64-step probes. No
probe or emulator run is canceled to obtain these results.

## Pinned target results

| Probe | Net cycles (64 steps) | Cycles/step |
|---|---:|---:|
| composed lower-atmosphere world + actuator + sensors + flight computer | 114,410,378 | 1,787,662.2 |
| GPS-aided upper-stage guidance/navigation | 13,639,485 | 213,117.0 |
| stuck-steering monitoring and abort | 13,595,518 | 212,429.9 |
| orbital coast | 31,451,996 | 491,437.4 |
| actuator | 179,547 | 2,805.4 |

All three PAL VICE runs were cycle-stable. Every native/MOS state field and the
truth, sensor, navigation, and flight checksum chains matched exactly; the
comparison reports the first named field if they diverge.

The conservative eligibility estimate adds composed and GPS-guidance costs,
then projects 7,200 mission steps at the PAL 985,248 Hz clock. The result is
14,621.3 seconds (243.7 minutes), above the locked 1,800-second threshold.
Therefore the full nominal C64 simulation is deliberately not run. This is the
pre-agreed gate outcome, not an interrupted run.

## Memory

The probe PRG is 37,830 bytes, loads from `$0801` through `$9BC5`, and leaves the
`$C000` result region untouched. The linked MOS ELF reports 17 zero-page bytes
and 835 bytes of `.noinit` static working/stack storage (852 bytes total
zero-fill storage). It fits stock C64 RAM and uses no REU.

The KRP3 replay PRG is 26,841 bytes and ends at `$70D8`. Its linked image reports
17 zero-page bytes and 175 bytes of `.noinit` static working/stack storage. The
embedded nominal KRP3 tape is 21,776 bytes.

## Replay

The C64 independently parses every KRP3 header and record CRC, checks scenario
and config identity, enforces ordering and terminal events, accumulates event
and alarm cues, renders a PETSCII mission page, and schedules a SID cue. Pinned
VICE verified 906 records, terminal step 7200, source KST3 CRC `af79b36e`, two
ignitions, two cutoffs, one separation, one end cue, zero aborts, and the final
`PHASE 3 REPLAY PASS` display.