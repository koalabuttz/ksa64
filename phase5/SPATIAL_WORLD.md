# Phase 5 spatial world and orbit model

Gate 5 supplies the translational half of the 6-DOF world in the frozen
right-handed ECI frame.

## Environment and translation

`SpatialState` owns Q12 kilometre position, Q24 kilometre-per-second velocity,
and the last Q28 kilometre-per-second-squared acceleration. A two-word
three-component magnitude primitive supports radii, speeds, angular momentum,
and transverse flow without compiler-provided 64-bit products.

The environment uses the accepted spherical Earth radius, gravitational
parameter, rotation rate, and Phase 2 atmosphere table. It computes:

- central gravity directed toward Earth;
- rigid atmosphere co-rotation as `omega cross position`;
- vehicle-relative air velocity and speed;
- density, speed of sound, Mach, and dynamic pressure.

Translation advances velocity then position semi-implicitly at 8 Hz. Applied
force is expressed in ECI kN and divided by vehicle tonnes with the explicit
m/s^2 to km/s^2 conversion. Invalid radius, mass, timestep, or arithmetic leaves
the previous state intact.

## Aerodynamics

The initial 3-D model intentionally retains KSA-2A's simple axial coefficient
philosophy. It adds a linear normal-force slope in body Y/Z and an aft
centre-of-pressure arm. The evaluator returns total aerodynamic force in ECI,
aerodynamic torque in body MN*m, Mach, dynamic pressure, and a bounded sine-of-
angle-of-attack proxy. Axial and lateral forces oppose relative flow; the
normal force produces the expected restoring moment.

This is a learning database, not CFD or wind-tunnel correlation. Stage-specific
area, coefficient, slope, and arm remain scenario data.

## Orbit analysis

The 3-D classifier derives specific energy, angular momentum, radial and
transverse speed, eccentricity, apsides, and inclination. A generated 257-knot
acos table maps the angular-momentum direction to binary-turn inclination. The
analytic 200 km circular state at 51.6 degrees is classified stable with near-
zero eccentricity and the expected plane.

## Evidence

Native tests cover the Cape-like launch site, exact co-rotation, surface
gravity, axial and lateral aerodynamics, inclined circular-orbit classification,
and 16 seconds of vacuum propagation. The independent generator supplies the
launch and circular states plus the acos table. A portable exact signature over
environment, orbit, and successor raw values is frozen as `0x650d5aa7`; native
Rust and the pinned rust-mos instruction-level probe agree exactly.

The rust-mos build requires `advance_spatial_state` to remain `#[inline(always)]`
when more than one distinct call site appears in an exactness probe. This is the
same bounded code-generation safeguard recorded for the rigid-body step;
removing it is an explicit cross-target revalidation task, not a cosmetic
cleanup.