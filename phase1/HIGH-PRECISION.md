# Phase 1 high-precision comparison

Date: 2026-07-22

Status: passed.

## Result

The accepted exact C64 trajectory finishes 279.355 m lower and 2.857 m/s slower than the confirmed high-precision RK4 reference after 256 simulated seconds.

| Comparison at 256 s | Altitude delta | Velocity delta |
|---|---:|---:|
| Fixed-point minus same-step Decimal | +7.842186 m | +0.042079 m/s |
| Decimal semi-implicit Euler minus confirmed RK4 | -287.197179 m | -2.898800 m/s |
| Fixed-point minus confirmed RK4 | -279.354992 m | -2.856721 m/s |
| RK4 reference minus confirmation | -0.000006487 m | -0.000000037 m/s |

Fixed-point and table quantization are therefore small compared with the accumulated first-order integration error in this mission. The total altitude difference is about 0.074 percent of the 380.030 km confirmed reference altitude. This is accepted for the Phase 1 learning model; it is measured error, not an engineering accuracy claim.

## Independent calculation

`reference/generate_high_precision.py` uses Python's standard-library `Decimal` type at 80-digit precision. It reads the human-readable scenario and source environment values, independently evaluates the force equation, and never calls the Rust core.

Three paths distinguish error sources:

1. A Decimal semi-implicit-Euler run preserves the product integrator, 0.125 s step, and update order while removing fixed-point and table-value quantization.
2. A Decimal RK4 reference uses a 0.00390625 s step, 32 times finer than the product step.
3. A Decimal RK4 confirmation uses a 0.001953125 s step, 64 times finer than the product step.

The two RK4 results must differ by less than 1 mm altitude and 0.01 mm/s velocity. Their observed residual is much smaller. The powered/coast boundary is aligned exactly in every run, and each RK4 substep uses the appropriate one-sided vehicle mode at cutoff.

## C64 report

The generator emits reviewed Q16.16 constants for the total fixed-minus-confirmed-RK4 altitude and velocity deltas. The post-run C64 page displays them as:

    HP ALT DELTA        -279.355 M
    HP VEL DELTA          -2.857 M/S

These constants are presentation evidence, not inputs to the simulation. Display work remains outside the accepted physics and recorded-telemetry timing regions.

## Decision

Phase 1 retains semi-implicit Euler. RK2 or RK4 would require at least one additional force evaluation and would erase the current 8 Hz raw-physics margin. The measured error is adequate for the vertical laboratory's purpose, while Phase 2 must repeat the integrator tradeoff for orbital-insertion goals.

## Reproduce

From the project root:

    .\phase1\high_precision.ps1

The command recomputes all Decimal trajectories, verifies the checked JSON and generated Rust binding byte for byte, enforces the convergence tolerances, and prints the four comparison deltas.
