# Phase 5: three-dimensional rigid-body dynamics

Status: Gates 1-13 complete; Gate 14 completion audit and Phase 6 handoff are next.

The Phase 5 numeric/scenario contract and KSA-5A configuration are now frozen
under `phase5/`. Generated Rust constants bind the new work to the accepted
Phase 2/3 checksums without changing any inherited format or entry point.

The spatial foundation now supplies checked fixed-point vectors, scalar-first
Hamilton quaternions, deterministic normalization, and body-to-ECI rotation.
Independent vectors pass both native Rust and the pinned rust-mos target.
Rigid-body propagation now evaluates diagonal Euler coupling, applied torque,
semi-implicit angular rate, and normalized quaternion kinematics at the 32 Hz
fast cadence. Exact integer and analytic float64 references pass natively, and
two distinct target cases pass through an explicitly documented rust-mos
specialization constraint.
The flexible-body layer adds one lateral bending and one active-stage slosh
mode on each transverse axis. Driven, damped, undamped, symmetry, and
fail-closed cases match an independent integer oracle natively and in the
bounded rust-mos probe.
The spatial world now adds ECI translation, spherical gravity, atmosphere
co-rotation, 3-D axial/normal aerodynamics, body aerodynamic torque, and orbit
analysis with inclination. Native and rust-mos agree on the frozen exact world
signature `0x650d5aa7`.
The multirate vehicle layer now composes four fast steps per mission command,
continuous inertia schedules, two-axis engine gimbals, explicit staging, and
bounded upper-stage RCS propellant. Native and size-optimized rust-mos builds
agree on the complete raw signature `0x21e55663`; the target probe is 39,255
bytes.
The avionics boundary now adds strict spatial sensor and actuator transports,
deterministic IMU/barometer/GPS/star-tracker models, truth-isolated 3-D aided
navigation, quaternion attitude control, sequencing, and irreversible safeing.
Independent transport/controller vectors freeze signature `0xaa0a0b0e`; native
and rust-mos agree, and the target probe is 7,367 bytes. See
[AVIONICS.md](AVIONICS.md).

Phase 4 completed the maturity prerequisites for 3-D work: the 2-D world is deterministic and independently checked, flight software is isolated from truth, campaign variation is reproducible, storage is observational, and stock/REU evidence paths are strict.

## Inherited contracts

Phase 5 must preserve the complete Phase 3/4 planar path and its frozen artifacts. A 3-D scenario should be additive, carry a new explicit contract identity, and reduce to the accepted planar equations when out-of-plane state, moments, and torques are zero. REU support remains optional and cannot become a physics dependency.

The existing target evidence also sets expectations:

- correctness and bounded arithmetic come before real-time execution;
- host-native campaigns provide statistical breadth;
- finite MOS/VICE probes provide instruction-level exactness;
- a long C64 run requires a fresh projection and explicit confirmation;
- optimization begins only after representative 3-D kernels are measured.

## Integrated mission evidence

Gate 8 adds local-horizontal launch-plane guidance and freezes six complete
missions. Nominal and gust cases meet the 180-220 km target envelope; sensor
outage and RCS depletion remain stable degraded orbits; gimbal jam and damping
loss abort irreversibly. See [MISSIONS.md](MISSIONS.md). Independent float64
analysis verifies the raw terminal vectors, while a finite rust-mos probe
freezes guidance signature `0xada003ef` without starting a full target mission.

## KST5 telemetry evidence

Gate 9 adds allocation-free mission-cadence KST5 recording through the single
reviewed mission executor, a strict host inspector, independent byte-level
verification, and a finite rust-mos codec probe. The nominal stream is 3,134
frames and 1,328,912 bytes with CRC-32 `0xa9b3b94c`. See
[TELEMETRY.md](TELEMETRY.md).

## Spatial campaign evidence

Gate 10 adds KSC5/KSR5 contracts, a reviewed 15-parameter spatial variation
catalog, deterministic parallel host execution, independent draw/orbit
reconstruction, and finite rust-mos sampling/codec evidence. The frozen 256-run
campaign is byte-identical across serial and eight-worker execution. See
[CAMPAIGNS.md](CAMPAIGNS.md).

## Target timing

Gate 11 measures separate stock-compatible vehicle, avionics, and telemetry
paths. Their conservative sum projects a full target mission to 19.69 hours, so
no full mission was started. One exact zero-variation fast path was accepted and
a native/MOS-divergent inertia candidate was rejected. See [TIMING.md](TIMING.md).

## Adaptive history evidence

Gate 12 adds strict KPH5 compact spatial histories, deterministic stock retention, capacity-scaled REU plans, and recoverable KRA5 archives. The stock baseline is 1,664 bytes; the finite target probe is 5,917 bytes. See [HISTORY.md](HISTORY.md).

## Mission-control replay evidence

Gate 13 adds strict native KPH5 replay plus a 6,252-byte stock-C64 mission-control page. VICE freezes the complete screen, trajectory plot population, and SID cue schedule. See [REPLAY.md](REPLAY.md).

## Next gate

Gate 14 performs the Phase 5 completion audit and records the Phase 6 handoff. A complete target mission still requires a fresh projection and explicit confirmation.
