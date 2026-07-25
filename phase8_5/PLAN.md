# Phase 8.5 — Unified Avionics and Exact Event Execution

Phase 8.5 connects the Phase 6 flight-computer architecture to the Phase 8 local-ENU vehicle world. It is additive: the frozen Phase 8 executor, truth-triggered recovery, formats, artifacts, and legacy profile names remain unchanged.

## Accepted direction

- Add a separately identified avionics-aware local-ENU executor.
- Drive it from an exact Q18 32 Hz clock (`8192` raw units per release).
- Treat physical timesteps as maxima and retain the original physical deadline when an avionics release splits a step.
- Preserve sensor-N, command-N, effective-N+1 ordering in host, VICE, and in-memory loopback placements.
- Give the original Firestorm truth-blind navigation, health monitoring, attitude monitoring, and commanded recovery without inventing steering.
- Prove active control with a separately identified fictional derivative using a bounded two-axis motor gimbal.
- Keep guidance effector-neutral and bind it to a statically selected control allocator. Canards, cold-gas RCS, and mixed allocation remain Phase 9.5 work.
- Adapt the complete F1–F7 Phase 6 Mission Control presentation.
- Use a deterministic 64-run routine campaign with seed `0x4B534185` plus named fault cases. No 1,024-run Phase 8.5 campaign is required.
- Require a combined stock-C64 loopback image without an REU. Try a flat image, measured size optimization, and stock RAM-under-ROM banking in that order. Stop and report the exact deficit if those options cannot satisfy the target.

## Timing contract

At release N the world is at the exact release time, sensors sample the current state, and flight software produces command N. Continuous command N is held through the interval to release N+1; its first sensor-visible physical effect is sample N+1. Discrete deployment commands are one-shot and epoch-tagged.

The accepted multirate schedule remains 32 Hz for IMU/control, 8 Hz for navigation/recovery/health/status, and 1 Hz for GPS and mission guidance.

## Recovery and control

The avionics-aware recovery profile uses measured launch/burnout qualification, two consecutive descending 8 Hz estimates for drogue deployment, and descending filtered AGL at or below 200 m for main deployment. Reference backup deadlines are T+15 seconds for drogue and T+65 seconds for main. Continuity and deployment feedback are physical simulated channels, not private truth.

The original Firestorm uses monitor-only attitude allocation. The fictional derivative uses a ±5 degree two-axis gimbal, 30 degrees/second slew, two-release lag, neutral safe state, no rail authority, and no post-burnout authority.

## Required placements and evidence

1. Host world plus host flight computer, with fast, real-time, and step-and-acknowledge modes.
2. Host world plus VICE/C64 flight computer through KLF6.
3. Combined stock-C64 world and avionics through the same raw cells in an in-memory mailbox.

Acceptance requires exact cell/checksum agreement across placements, passive presentation, deterministic campaign results across worker counts, a standalone avionics kernel below 80% of the PAL 32 Hz budget, and preservation of every accepted Phase 0–8 artifact.

The software is engineering simulation evidence, not launch approval, certification, or safety authority.
