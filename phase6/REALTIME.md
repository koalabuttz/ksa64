# KSA-6R realtime profile

Status: controller timing and full externally paced PAL execution accepted; live-link wall-clock acceptance remains open.

KSA-6R is distinct from the frozen accuracy-first exact-paced profile. It uses 32 Hz control, 8 Hz navigation/sequencing/status, and 1 Hz guidance slices. Navigation releases occur at fast phase 0 and guidance at phase 2 so their largest bounded workloads do not collide. Compact cells carry Q12 delta velocity and Q15 quaternion-vector attitude observations. Commands are tagged with their source epoch and become effective in the following fast epoch.

The controller uses a small-angle Q15 vector error with signed rate feedback. The C64 hashes signed gimbal values from explicit four-byte two's-complement representations; it does not rely on target-dependent signed widening.

## Frozen mission

The native oracle and the stock C64 flight endpoint both complete after 12,692 fast epochs and 3,173 committed world steps. Every C64 command and status cell was compared with a native shadow flight computer during the accepted 1x PAL run.

| Evidence | Accepted value |
|---|---:|
| Terminal position Q12 | `[21360371, 4030786, 15731027]` |
| Terminal velocity Q24 | `[-69442203, 96406364, 65655653]` |
| Navigation position Q12 | `[21360000, 4031445, 15731484]` |
| Navigation velocity Q24 | `[-68076267, 95786604, 65320561]` |
| Navigation checksum | `0x82e09168` |
| Final flight checksum | `0xacf09b87` |
| Deadline misses | 0 |
| Alarms | 0 |

The full evidence is stored in [vice-mailbox-v1.json](vice-mailbox-v1.json). The relay ran x64sc at normal PAL CPU speed but used binary-monitor mailbox transactions that pause emulation. Its 1,011.328-second wall duration therefore does not claim end-to-end 32 Hz wall-clock transport.

## Target timing

Three refreshed PAL timing runs produced identical result words:

| Release | Cycles | 80% fast-slot budget | Result |
|---|---:|---:|---|
| Ordinary 32 Hz | 12,339 | 24,631 | Pass |
| 8 Hz navigation/status | 23,656 | 24,631 | Pass |
| 1 Hz guidance lookup | 14,914 | 24,631 | Pass |

The projected flight-computer CPU demand is 196.436 seconds over a 396.625-second mission, or about 49.5% average utilization. The slowest measured release retains 975 cycles of margin under the deliberately conservative 80% slot budget. This is compute evidence for one stock PAL C64; it does not include a hardware driver's polling or interrupt cost.

## Scheduling and safeing

- One fast release occurs every 31.25 ms.
- Navigation, sequencing, aiding, and status occur every fourth fast release.
- A new guidance slice is installed every 32 releases and interpolated by the fast loop.
- Two missing fast cells hold the preceding continuous command without replaying discrete actions.
- A third miss latches stale-link safeing and propulsion cutoff.
- A measured release above the budget latches deadline safeing.
- Late or wrong-session commands are rejected by the world endpoint.

## Acceptance interpretation

KSA-6R has passed three distinct gates:

1. native complete-mission exactness;
2. finite target cycle measurement under pinned PAL x64sc;
3. complete stock-C64 execution at normal PAL CPU speed under deterministic external pacing.

A fourth gate remains: complete live transport over a physical SwiftLink/Turbo232 or another accepted realtime adapter without monitor pauses. That hardware result may refine the supported baud rate or scheduling margin but may not change the frozen flight equations or command sequence silently.
