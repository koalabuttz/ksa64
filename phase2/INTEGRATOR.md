# Phase 2 vacuum coast and integrator decision

Status: semi-implicit Euler selected provisionally; the powered hero mission must confirm the accuracy thresholds.

The product state uses radius, Earth-relative downrange, radial velocity, and inertial specific angular momentum. In an unforced central field, angular momentum therefore remains bit-identical. Tangential velocity, radial gravity/centrifugal acceleration, downrange rate, specific energy, eccentricity, apogee, and perigee are derived through the explicit fixed-point primitives.

Both semi-implicit Euler and midpoint RK2 were implemented at 0.125 seconds. Over the accepted circular-orbit fixture they produce the same raw fixed-point state after one orbit: midpoint's half-step correction is below the chosen radius/acceleration resolution in this regime. A changing-radius 180×220 km C64 timing fixture likewise produces identical 32-second terminal radius and radial velocity.

Three stable PAL VICE measurements record 451,742.59 cycles per semi-implicit step and 452,754.37 cycles per midpoint step. The planar vacuum kernel is intentionally accuracy-first and is already slower than real time; no throughput floor applies. The result selects semi-implicit Euler because RK2 currently adds code/evaluation structure without changing accepted product values.

`integrator-v1.json` independently evaluates the same elliptical coast in floating point, including RK4 at 1/32 and 1/64 of the product timestep. The powered mission will reopen the choice if semi-implicit Euler misses the declared insertion thresholds.
