# KSA-6R realtime profile

Status: native mission and finite PAL controller timing accepted.

KSA-6R is distinct from the frozen accuracy-first Phase 5/Phase 6 exact-paced profile. It uses 32 Hz control, 8 Hz navigation/sequencing/status, and 1 Hz guidance slices. Navigation releases occur at fast phase 0 and guidance at phase 2 so their bounded workloads do not collide. Compact cells carry Q12 delta velocity and Q15 quaternion-vector attitude observations. Commands become effective in the following fast epoch.

The controller uses a small-angle Q15 vector error with signed rate feedback. This is deliberately cheaper than reconstructing a complete quaternion on the 6510. A complete native mission reaches the terminal phase after 12,692 fast epochs (3,173 mission steps) without safeing. Its exact terminal state and checksum chains are frozen in the integration test.

## Target timing

The finite rust-mos probe measures both an ordinary fast release and the worst coincident navigation/status release under pinned PAL x64sc. Three identical runs produced:

| Release | Cycles | 80% slot budget | Result |
|---|---:|---:|---|
| ordinary 32 Hz | 12,452 | 24,631 | pass |
| 8 Hz navigation/status | 23,787 | 24,631 | pass |
| 1 Hz guidance lookup | 14,997 | 24,631 | pass |

The projected flight-computer CPU demand is 197.94 seconds over a 396.625-second mission, or about 49.9% average utilization. The 14,586-byte probe remains well below the stock-memory boundary.

This is evidence that the flight-computer workload fits one stock PAL C64. It is not yet the full two-endpoint 1x VICE bridge acceptance: transport interrupt/polling costs and the complete externally paced run remain later Phase 6 gates.

A failed optimization trial is intentionally recorded in development history: reconstructing quaternion scalars and interpolating four components caused a rust-mos target-only stall, while multiplication-heavy FNV checksum chaining missed the deadline. The accepted design transports the bounded vector representation and uses an incremental rotate/xor/add evidence chain. Native full-mission behavior and finite target timing both pass.
