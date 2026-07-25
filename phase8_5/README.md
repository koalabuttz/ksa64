# Phase 8.5 — Unified Avionics and Exact Event Execution

Status: software baseline accepted on 2026-07-25. The self-contained combined stock-C64 image reached the explicit fit decision boundary; split stock-C64 avionics remains accepted and the frozen Phase 8 stock world remains available.

Phase 8.5 connects the Phase 6 flight-computer architecture to the Phase 8 local-ENU world without changing the frozen Phase 0–8 executors or artifacts.

## What it adds

- Exact 32 Hz releases at Q18 increments of 8,192, with physical steps split at avionics or mission-event deadlines without moving the original physical deadline.
- Sensor-N, command-N, effective-N+1 execution: a command first changes sensor-visible state at the next release.
- Truth-blind local-ENU IMU/barometer/GPS/attitude aiding, navigation, health, recovery sequencing, feedback, alarms, and safeing on the shared 32/8/1 Hz architecture.
- The real Firestorm remains monitor-only for attitude while avionics commands one-shot drogue and main deployment from measured state.
- A separately identified fictional derivative adds a 20 g, two-axis motor-gimbal installation, ±5 degree travel, 30 degree/s slew, two-release lag, rail inhibition, burnout loss of authority, and a launch-rail attitude reference.
- KAP8/KAC8/KLE8/KLR8/KAT8/KAS8/KMR8 contracts with strict identity, reserved-byte, CRC, and KLF6 framing rules.
- Host/host and host-world plus VICE/C64-flight placements, both feeding the same full F1–F7 Mission Control presentation.
- A 64-run deterministic avionics campaign and named dropout, delay, link, deadline, gimbal, continuity, and recovery-feedback cases.

## Running it

Host world, host flight computer, live Mission Control:

```powershell
powershell -File phase8_5/run.ps1 -Flight host -Display tui -Pace realtime
```

Fast host summary, optionally using the fictional gimbal derivative:

```powershell
powershell -File phase8_5/run.ps1 -Flight host -Display summary -Pace fast
powershell -File phase8_5/run.ps1 -Flight host -Display summary -Pace fast -Gimbal
```

Finite VICE/C64 flight-computer acceptance (eight releases by default):

```powershell
powershell -File phase8_5/run.ps1 -Flight vice -Display summary -ProbeReleases 8
powershell -File phase8_5/run.ps1 -Flight vice -Display summary -ProbeReleases 8 -Gimbal
```

Live host world plus VICE/C64 flight computer and host Mission Control:

```powershell
powershell -File phase8_5/run.ps1 -Flight vice -Display tui -Pace realtime
```

Only one VICE instance is allowed. The launcher refuses to start if another is running, and the relay closes VICE after success or proven failure.

## C64 result

The generic flight endpoint is 15,412 bytes and fits at `$0801-$4432`. The aided avionics release costs 21,184 PAL cycles and the fast release 10,843 cycles; the aided path uses 68.8% of a full 31.25 ms slot and passes the stricter 80% budget with 3,447 cycles of headroom.

The monolithic world-plus-avionics program requires 71,500 resident bytes. That exceeds both the ordinary linker region and all 64 KiB of physical address space, so ROM banking alone cannot solve it. The exact decision and user-selectable overlay/rewrite/expansion alternatives are in [STOCK_FIT_DECISION.md](STOCK_FIT_DECISION.md). No REU requirement or feature cut was chosen automatically.

## Evidence

- [Completion record](COMPLETION.md)
- [Implementation contract](PLAN.md)
- [Stock-fit decision](STOCK_FIT_DECISION.md)
- [PAL timing](avionics-timing.json)
- [64-run campaign](campaign-64.json)
- [Monitor VICE probe](reference/vice-probe-8.json)
- [Gimbal VICE probe](reference/vice-probe-gimbal-8.json)

KSA64 is an engineering simulation. These results are not launch approval, certification, regulatory evidence, or safety authority.
