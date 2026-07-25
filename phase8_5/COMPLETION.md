# Phase 8.5 completion record

Status: accepted on 2026-07-25 at the documented stock-fit decision boundary.

Phase 8.5 unifies the local-ENU vehicle world with the shared flight-computer architecture, exact event execution, avionics-commanded recovery, capability-bound control, split C64 placement, Mission Control, and deterministic fault/campaign evidence. Every frozen Phase 0–8 path remains available.

## Gate record

| Gate | Commit | Accepted result |
|---|---|---|
| Contracts and terminology | `f1eca53` | Compatible profile aliases, identities, formats, and additive evaluation boundary frozen. |
| Exact event executor | `8889b71` | Exact 8,192-Q18 releases, retained physical deadlines, scheduled event splits, and N/N+1 ordering accepted. |
| Avionics formats | `530cbc2` | Strict KAP8/KAC8/KLE8/KLR8/KAT8/KAS8 codecs and identity/corruption gates accepted. |
| Navigation and recovery | `845b881` | Truth-blind local sensors/navigation plus measured-state dual deployment and backups accepted. |
| Control and evidence | `f3d7372` | Monitor-only allocation, fictional gimbal physics, strict telemetry, summaries, and named faults accepted. |
| Placements and Mission Control | `83e48f3` | Host/host, KLF6 host/external placement, KMR8, and F1–F7 presentation accepted. |
| Campaign | `44f45c6` | Ordered 64-run campaign, keyed variations, worker exactness, and independent analyzer accepted. |
| Runtime and stock gate | `0c8e74c` | PAL timing, generic monitor/gimbal C64 endpoint, live VICE TUI wiring, and stock-fit decision accepted. |
| Rail-reference correction | `534282d` | Launch-rail attitude target, 20 g fictional actuator assumption, 5 m/s crosswind settling gate, and refreshed evidence accepted. |

## Exact execution and mission evidence

- Avionics releases occur at `N * 8192` Q18 units.
- A split physical interval retains its original physical deadline.
- Sensor sample N produces command N; its first sensor-visible effect is sample N+1.
- Monitor-only nominal: 2,823 releases, ground contact, checksum chains `[0x2a7870ab, 0xda70ca1f, 0xdf07ec01, 0x9ae1b422, 0x56b2883a, 0x0160c000]`.
- Gimbal derivative nominal: 2,799 releases, ground contact, checksum chains `[0x28ef0da8, 0xf513523e, 0x6d130454, 0xe94bb076, 0xd2b37901, 0x015dd2b7]`.
- Original Firestorm physical gimbal commands remain neutral.
- Avionics-commanded drogue/main deployment, continuity checks, acknowledgement, primary logic, and T+15/T+65 backups pass without truth access.
- The 5 m/s fictional-derivative case remains inside the Phase 8 aerodynamic envelope and reaches the <=3 degree rail-relative settling gate within eight releases (0.25 s) after rail exit.

## Campaign evidence

The routine campaign uses seed `0x4b534185` and exactly 64 runs.

- KAS8 ordered archive: 16,896 bytes.
- SHA-256: `ec1b3feda54deee8e2ec0cce2b38c2d2023085ab818a90d4d36841bf86433023`.
- Ordered record CRC-32: `0xa4d42479`.
- One- and four-worker archives are byte-identical.
- All 64 cases complete; five raise declared alarms; none exceeds the model envelope or saturates the accepted controller.
- Maximum campaign navigation error: 466,673 Q13 units.
- Maximum rail-relative attitude error: 1,479 turn16 units.

## C64 evidence

| Target | Result |
|---|---|
| Generic flight endpoint | 15,412 bytes, `$0801-$4432`, monitor and gimbal finite probes pass. |
| Aided release | 21,184 cycles, 68.8% of the full PAL slot, 3,447 cycles below the 80% budget. |
| Fast release | 10,843 cycles. |
| Self-contained combined target | 71,500 resident bytes; ordinary-region deficit 20,301 bytes. |
| Best ROM/I/O banking case | Still short by 8,013 bytes; even all physical RAM is short by 5,964 bytes before mandatory reservations. |

The combined image therefore stopped at the plan's explicit decision boundary. The accepted stock-C64 options remain the standalone Phase 8 world and the Phase 8.5 flight computer attached to a host world. A disk overlay, stock-specialized rewrite, or expansion-memory path requires a separate user decision.

No complete combined C64 mission was started because no accepted combined image exists. No run was canceled for duration, and every finite VICE process was closed after success or proven failure.

## Compatibility and limitations

The frozen truth-triggered Phase 8 executor remains unchanged. Legacy `HobbyVerticalV1` and `HobbySpatialV1` serialized identities remain valid, with canonical source aliases `VerticalPointMassV1` and `LocalEnu6DofV1`. Canards, RCS, mixed allocation, dual-channel recovery avionics, ECEF/ECI propagation, and physical user-port/ACIA/Ultimate acceptance remain deferred.

KSA64 is an engineering simulation, not launch approval, certification, regulatory evidence, or safety authority.
