# Phase 2 completion record

Date: 2026-07-22

Status: complete.

Phase 2 delivers a deterministic equatorial planar launch simulation: rotating spherical Earth, co-rotating atmosphere, Mach-dependent drag, open-loop pitch guidance, bounded multistage sequencing, nominal orbital insertion, a failed-insertion case, canonical telemetry, host validation, measured C64 execution, and post-run PETSCII/SID replay.

## Exit criteria

| Criterion | Evidence | Result |
|---|---|---|
| A configured fictional vehicle reaches a stable or deliberately failed orbit | KSA-2A reaches a fixed-point 188.169 x 188.169 km stable orbit. Its five-percent-short upper-stage variant is classified as impact. Independent float64 results reach 199.989 x 200.015 km nominally and also classify the failure as impact. | Pass |
| Stage events conserve the quantities required by the model | Ignition, cutoff, and separation occur only on 0.125-second boundaries. Separation drops dry mass and residual propellant while position and velocity remain continuous; torque-free coast preserves specific angular momentum exactly. | Pass |
| Coast propagation satisfies energy and angular-momentum tolerances | Circular and 180 x 220 km vacuum fixtures pass the declared radius, energy, and angular-momentum bounds. Product semi-implicit Euler and midpoint produce the same accepted fixed-point state for the measured fixtures, so the cheaper product path remains selected. | Pass |
| Compatible segments agree with independent trajectory calculations after assumptions are aligned | Independent float64 semi-implicit and refined RK4 models use the same spherical-Earth, atmosphere, vehicle, and event assumptions and satisfy the mission/coast gates. A GMAT R2026a point-mass cutoff-state fixture and comparator are supplied for an optional third-party run; no unexecuted GMAT result is claimed. | Pass |

## Final evidence

- Native Rust: 68 tests plus formatting, lint, and `no_std` workspace checks pass.
- rust-mos: generated numeric, environment, guidance, scenario, telemetry, and mission-smoke checks pass under `mos-sim-none`.
- Packed scenarios: nominal and early-cutoff `KSC2` images are 884 bytes and fail closed on framing, identity, range, topology, alignment, and CRC errors.
- Nominal fixed-point mission: 7,200 steps over 900 seconds, 188.169 x 188.169 km stable orbit, Max-Q 40.779 kPa, peak proper acceleration 55.283 m/s2, final checksum `0xcc57612b`.
- Independent nominal mission: 199.989 x 200.015 km, eccentricity 0.000002, Max-Q 40.777 kPa, peak proper acceleration 55.292 m/s2.
- Early-cutoff mission: both exact and independent models classify the trajectory as impact.
- Vacuum target timing: 451,562.59 cycles per semi-implicit step and 452,574.37 per midpoint step; terminal product states are identical for the timing fixture.
- Powered target timing: 1,232,700.625 cycles per raw step (0.799 PAL steps/s) and 1,368,798.500 cycles per checksummed/recorded step (0.720 PAL steps/s).
- Canonical telemetry: 901 `KST2` frames, 57,704 bytes, stream CRC-32 `0x7d13b2bf`, final state checksum `0xcc57612b`.
- C64 replay: 2,851-byte source-bound `KRP2` tape, 16,169-byte PRG, 50 plotted cells, 135-byte retained sink, exact screen-memory verification, and SID schedule hash `0x9473fcdb`.

## What the timing means

Phase 2 prioritizes trustworthy arithmetic and architecture over real-time execution. The measured powered fixture implies roughly 2.5 hours for a raw 7,200-step mission or 2.8 hours with rolling checksums and canonical recording on a PAL C64. These are estimates from a representative powered sea-level fixture, not a claim that every mission step has identical cost.

The post-run display therefore consumes a compact presentation tape derived only from host-validated canonical telemetry. It does not rerun the mission or become a second source of physical truth.

## Accepted limits

- The model is equatorial and planar; it has no attitude dynamics, lift, winds, sensors, actuators, or closed-loop flight software.
- Earth is spherical, gravity is point-mass, and the atmosphere is a compact generated learning table rather than a named standard atmosphere.
- Engines use constant thrust and mass flow per stage; pitch, ignition, cutoff, and separation are step-aligned.
- Fixed-point quantization produces a roughly 12 km lower nominal circular orbit than the independent float64 model, while remaining inside the declared 180-220 km acceptance envelope.
- The instruction-level MOS mission gate is intentionally bounded; complete nominal/failure missions run natively, and representative exact target paths run through rust-mos and PAL VICE.
- GMAT is not bundled, required, or claimed as executed. Its aligned R2026a fixture remains an optional external cross-check.
- The replay tape is a derived presentation index. `KST2` remains the canonical regression record.

## Reproduce

From the project root:

    .\phase2\complete.ps1

This reruns the generated-evidence, native, rust-mos, vacuum-timing, host-capture, powered-timing, and C64 replay matrix. A passing run ends with:

    PHASE 2 COMPLETION AUDIT: PASS

## Phase 3 boundary

New flight-computer behavior belongs to Phase 3. Sensors, navigation, guidance modes, closed-loop control, actuators, delays, failures, and truth isolation must build on the accepted Phase 2 planar executor and telemetry contracts rather than bypassing them.
