# Phase 2 atmosphere, aerodynamics, and open-loop guidance

Status: accepted for the bounded Phase 2 ascent model.

The environment is a rotating spherical Earth with an atmosphere that co-rotates at the declared Earth angular rate. A generated, reviewed table supplies density and speed of sound from sea level through 120 km; the portable core performs deterministic fixed-point interpolation and clamps beyond the table endpoints. This is intentionally a compact learning model, not a claim of reproducing a named standard atmosphere.

Aerodynamics are evaluated in the air-relative radial/tangential frame. The core derives relative speed, Mach number, dynamic pressure, and a linearly interpolated drag coefficient, then resolves drag opposite the relative-velocity vector. A vehicle exactly co-rotating at the surface therefore sees zero dynamic pressure, while either radial or tangential relative motion produces opposing drag.

Open-loop guidance is a fixed-capacity sequence of step-aligned time/pitch knots. Pitch is an unsigned binary turn measured from local radial toward prograde. Linear interpolation produces a deterministic commanded pitch at every integration step; generated Q1.15 trigonometry resolves thrust into radial and tangential components.

The combined force evaluator is pure: for one immutable truth state and command it returns gravity, thrust, drag, atmospheric observables, and total accelerations. The successor updates radial velocity, radius, specific angular momentum, downrange, mass, and propellant without exposing mutable truth to guidance.

Acceptance evidence includes native and rust-mos target self-tests for table identity, interpolation, pitch endpoints, co-rotation, drag direction, dynamic-pressure scale, thrust axes, and angular-momentum response. The next gate adds bounded stages, ignition/cutoff/separation sequencing, and complete nominal/failed missions.
