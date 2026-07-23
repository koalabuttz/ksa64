# Phase 5 multirate vehicle coupling

Gate 6 composes the accepted spatial environment, rigid body, and flexible modes
into a deterministic KSA-5A vehicle without modifying any Phase 2-4 API or
artifact.

## Cadence and transaction boundary

One mission command covers exactly 0.125 seconds. The vehicle executes four
0.03125-second fast steps, then commits the complete result. A configuration or
numeric fault discards the local successor and leaves the prior machine state
unchanged. Engine sequencing and separation commands are accepted only at the
mission boundary; gimbal response, RCS, attitude, flexible modes, translation,
and propellant consumption run at the fast cadence.

Each fast step evaluates, in order:

1. two-axis gimbal lag, slew, clamp, and jam state;
2. fuel-dependent diagonal inertia and Mach-dependent aerodynamic coefficient;
3. 3-D aerodynamic force and body torque;
4. engine force, gimbal moment, bounded RCS moment, and rate damping;
5. rigid-body rate/quaternion propagation and transverse bend/slosh modes;
6. ECI translation and exact residual-based propellant accounting.

The force contract remains MN and tonnes, so 1 MN / 1 t is 1 km/s^2. Torque is
MN*m, inertia is t*m^2, gimbal angles are Q16 radians, and RCS requests are
signed Q15 fractions of the per-axis limit.

## Generated KSA-5A data

`reference/generate_vehicle_vectors.py` derives a versioned, allocation-free
vehicle pack from the reviewed KSA-5A values and the inherited KSA-2A vehicle:

- the 12 t payload and 534 t initial mass;
- both stage thrust, flow, burn, delay, geometry, and aerodynamic tables;
- dry/wet cylinder inertia endpoints and midpoint evidence;
- 6 degree gimbal limits, 8 degree/s slew, and four-fast-step lag;
- 0.10 t upper-stage RCS allocation, 0.08 MN*m per-axis torque, and Isp-based
  mass flow;
- launch attitude and active-stage flexible-mode parameters.

The target constructor uses only this compact generated pack. A separate host
constructor validates that the inherited KSC2 is exactly the accepted KSA-2A
base. This keeps target code small without weakening compatibility evidence.

## Staging and RCS

Cutoff changes stage 1 to a bounded separation coast. Separation cannot occur
before its eight-mission-step delay and discards stage-1 dry mass plus any
remaining stage-1 propellant. Stage 2 then observes its four-step ignition
delay. Inertia changes continuously with active propellant and switches to the
next schedule only at separation.

RCS is unavailable before upper-stage separation. Three signed axes share a
bounded propellant tank; sub-Q12 consumption is retained in a Q24 residual so
small fast-step draws cannot disappear. Leak commands use the same clamps as
flight commands. Depletion produces one event, zero torque thereafter, and
cannot perturb future mass or seed/state ordering.

## Evidence

Independent generated vectors cover gimbal response at fast steps 1, 4, and 8,
both inertia endpoints and midpoint, aerodynamic knots, and a one-mission-step
full-axis RCS draw. Native tests cover atomic four-substep execution, forced
cutoff, delay enforcement, separation, upper-stage ignition, inertia switching,
gimbal jam, RCS leak/depletion, and rejection of the wrong planar base.

The complete portable raw-state signature is `0x21e55663`. It includes the
component vectors, one powered four-substep step, forced staging, and an
upper-stage RCS step. Native Rust and the pinned rust-mos instruction-level
probe agree exactly.

The ordinary `release` profile is not a valid stock-size measurement for this
integrated target: it inlines for speed and overflowed the simulator link region.
The already-established `c64` profile (`opt-level = "z"`, one codegen unit, LTO)
links the full Gate 6 signature probe at 39,255 bytes and executes successfully.
Gate 11 will measure the representative mission kernel and remaining UI/telemetry
headroom before any additional optimization is accepted.