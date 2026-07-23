# Phase 5 rigid-body propagation

Gate 3 turns the spatial numeric foundation into checked rotational dynamics at
the frozen 0.03125-second fast cadence.

## Model

The state is a scalar-first Q30 Hamilton body-to-ECI quaternion plus a Q24 body
angular-rate vector. The current inertia model is a positive diagonal tensor in
Q12 t*m^2. Input torque is Q16 MN*m.

One step evaluates the diagonal Euler equations:

```text
I * omega_dot + omega cross (I * omega) = torque
```

The implementation explicitly converts MN*m to kN*m so the inertia and torque
units close. Angular rate advances semi-implicitly. Quaternion kinematics use
the updated body rate, a normalized first-order step, and the same sticky
fail-closed numeric policy as the translational core. Invalid inertia, invalid
time, or any arithmetic fault preserves the original state.

## Independent evidence

`phase5/reference/generate_rigid_body_vectors.py` independently implements the
frozen integer operation order and also records float64 special-case truths.
Native acceptance covers:

- one spherical-inertia constant-torque step;
- one asymmetric-inertia torque-free step with nonzero Euler coupling;
- 64 constant-torque steps against the analytic final rate and angle;
- 64 constant-rate steps against the analytic spin angle;
- invalid configuration and preexisting-fault containment.

The finite target probes split the cases to avoid a rust-mos multi-case defect: the
spherical executable checks the complete one-step state field by field, while the
asymmetric executable checks the unique nonzero Euler-coupling acceleration. Native
tests retain the complete asymmetric propagation check.

## rust-mos specialization constraint

During target acceptance, one rigid step was exact in isolation but combining
the asymmetric full step with it produced either a wrong quaternion component or
unbounded execution, depending on optimization profile. The portable wrapper
remains forced inline, and target acceptance uses separate finite executables:
full spherical propagation plus the asymmetric Euler-coupling acceleration.
This preserves every arithmetic oracle while keeping target evidence bounded.

The production mission loop has one stable step call site. The explicit
`#[inline(always)]` is therefore part of the target correctness contract, not a
performance claim. Gate 11 will measure its code-size and cycle consequences;
it must not be removed without rerunning the multi-call-site MOS probe.