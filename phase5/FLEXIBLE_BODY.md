# Phase 5 bending and slosh modes

Gate 4 adds four bounded second-order states without turning KSA64 into a
finite-element or fluid solver:

```text
Y bending   Z bending
Y slosh     Z slosh
```

Each mode stores Q24 displacement and rate. Its Q16 parameters are natural
frequency in rad/s, damping ratio, and drive gain. The update evaluates

```text
x_ddot = gain * input - 2 * damping * frequency * x_dot - frequency^2 * x
```

and advances rate then displacement semi-implicitly at the 32 Hz fast cadence.
A bad frequency, damping ratio, gain, timestep, or arithmetic result preserves
all four previous modes. Zero damping is deliberately valid because the Phase
5 damping-loss mission needs to represent it explicitly.

The stage-specific frequencies and damping ratios remain data. Gate 6 will
select the active stage schedule and connect rigid angular acceleration,
lateral specific force, modal feedback torque, and the IMU observation point.
The modal layer itself owns no vehicle truth outside these four states.

## Evidence

An independent Python integer model freezes one-step and 128-step driven,
damped, and undamped results. Native tests verify exact raw values, demonstrate
that damping reduces the retained bending response, prove Y/Z symmetry under
identical inputs, check the 32-byte state footprint, and exercise fail-closed
configuration behavior. A finite rust-mos probe checks every field of the
representative four-mode step against the same generated oracle and returns
zero failures.