# GMAT cutoff-state validation fixture

This fixture hands the independent float64 KSA-2A cutoff state to NASA GMAT as an equatorial EarthMJ2000Eq Cartesian state. Orientation around the spherical Earth is arbitrary, so local radial velocity maps to `VX` and prograde tangential velocity maps to `VY` at `X = radius`.

The force model is intentionally aligned with KSA64's coast model: spherical Earth point mass only, with drag, SRP, relativity, tides, oblateness, the Moon, and the Sun disabled. The script propagates one analytic orbital period with Prince-Dormand 8(5,3) and reports radius, perigee radius, apogee radius, and eccentricity.

Run `ksa2a_cutoff.script` with GMAT R2026a, then compare the initial and final report rows with `expected.json`. GMAT's built-in Earth radius may differ from KSA64's declared 6378.137 km, so compare `RadPer`/`RadApo` as radii or subtract the explicitly recorded KSA64 radius; do not compare altitude labels without aligning that convention.

This repository does not bundle GMAT, and this fixture is not claimed as executed by the automated gate. The automated independent evidence remains the float64 model plus refined coast convergence; this script makes a repeatable third-party check available without changing runtime dependencies.
