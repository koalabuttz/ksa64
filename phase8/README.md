# Phase 8: spatial hobby flight

Status: complete and accepted on 2026-07-24.

Phase 8 adds the separately versioned `HobbySpatialV1` profile while leaving the inexpensive `HobbyVerticalV1` and every Phase 0–7 artifact intact. The accepted Firestorm 54/AeroTech I211W case now flies in local east/north/up coordinates from rail constraint through 6-DOF ascent, coast, weathercocking, dual deployment, three-dimensional recovery drift, and ground contact.

The offline compiler derives bounded mass properties, CG, inertia, CP, normal-force slope, damping, reference area, and stability schedules from reviewable geometry plus explicit provenance. The portable evaluator uses exact fixed-point arithmetic; host float64 and aligned OpenRocket 24.12 runs provide independent physical evidence.

## Accepted model boundary

- Local ENU translation and scalar-first Hamilton body-to-ENU quaternions.
- 0.01 s powered, 0.02 s coast translation with 0.01 s attitude substeps, and 0.05 s recovery steps.
- Full 6-DOF until drogue deployment; point-mass 3-D recovery afterward.
- Mach <= 0.8, angle of attack <= 15 degrees, and a Firestorm environment table valid through 3,000 m.
- Explicit `ModelEnvelopeExceeded` outside reviewed aerodynamic or environmental bounds.
- Flat launch-site terrain; no Coriolis, spherical-Earth, canopy pendulum, active avionics, staging, CFD, FEA, or fin-flutter model.

## Evidence

The calm reference reaches 754.234 m apogee, 139.255 m/s maximum speed, and 11.704 kPa maximum dynamic pressure. A 5 m/s steady crosswind case lands 234.672 m downwind with 14.613 degrees maximum accepted AoA. All 19 aligned OpenRocket checks pass without trajectory-output coefficient fitting.

KST8/KSR8/KPH8 provide strict telemetry, summary, and stock plot records. The seed `0x4b534138` reference campaign contains 1,024 ordered runs; serial and four-worker archives are byte-identical and all cases reach ground contact.

Stock C64 builds require no REU. Replay provides seven pages. A 17-state host/MOS trace is exact, while its conservative full-mission projection is 8,458.651 PAL seconds, so the routine audit deliberately does not start the complete C64 mission.

Run the bounded audit with:

```powershell
powershell -File phase8/complete.ps1
```

See [PLAN.md](PLAN.md), [COMPLETION.md](COMPLETION.md), [PHASE8_5_HANDOFF.md](PHASE8_5_HANDOFF.md), and [PHASE9_HANDOFF.md](PHASE9_HANDOFF.md).
