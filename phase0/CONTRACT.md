# Phase 0 v1 benchmark contract

## Purpose

This contract freezes the first fair workload used to compare rust-mos and Oscar64 C++. It is not a final KSA64 physics specification.

Changes to this contract require a version increment and regeneration of all vectors.

## Design constraints

- All timed C64 arithmetic uses signed or unsigned integers.
- Both candidates execute the same operation order.
- Both candidates use the same table bytes.
- General multiplication uses a signed 64-bit intermediate in the baseline.
- General division uses a signed 64-bit shifted numerator in the baseline.
- Optimized alternatives must remain bit-identical.
- No rendering, file access, allocation, or telemetry occurs inside timed kernels.
- Simulation time is independent of wall-clock time.

## Physical units

The vertical workload uses:

| Quantity | Unit |
|---|---|
| Time | second |
| Altitude | kilometre |
| Velocity | kilometre per second |
| Acceleration | kilometre per second squared |
| Mass | tonne |
| Force and thrust | meganewton |
| Density | kilogram per cubic metre |
| Aerodynamic area times coefficient | square metre |

These units provide an exact dimensional convenience:

    1 MN / 1 tonne = 1 km/s²

For drag:

    drag_MN = 0.5 * density_kg_m3 * velocity_km_s² * CdA_m2

The factors introduced by converting kilometres to metres and newtons to meganewtons cancel.

## Fixed-point formats

Every stored numeric value is a signed 32-bit two's-complement integer. A value with F fractional bits represents:

    real value = raw integer / 2^F

| Quantity | Format | Fraction bits | Approximate signed range | Resolution |
|---|---:|---:|---:|---:|
| Time | Q20.12 | 12 | ±524,288 s | 0.000244140625 s |
| Altitude | Q20.12 | 12 | ±524,288 km | 0.244140625 m |
| Velocity | Q8.24 | 24 | ±128 km/s | 0.0000000596 km/s |
| Acceleration | Q4.28 | 28 | ±8 km/s² | 0.00000000373 km/s² |
| Total and propellant mass | Q20.12 | 12 | ±524,288 t | 0.000244140625 t |
| Force and thrust | Q20.12 | 12 | ±524,288 MN | 0.000244140625 MN |
| Density | Q4.28 | 28 | ±8 kg/m³ | 0.00000000373 kg/m³ |
| CdA | Q16.16 | 16 | ±32,768 m² | 0.000015258789 m² |
| Interpolation fraction | unsigned Q0.16 | 16 | 0 through almost 1 | 0.000015258789 |

The formats are selected to exercise mixed-scale operations. They remain provisional for the eventual simulator.

## Conversion and rounding

Decimal constants are converted to raw integers by rounding to nearest, with exact half-way cases rounded away from zero.

All scaled multiply and divide results use the same rule:

1. Compute the exact sign.
2. Operate on the non-negative magnitude.
3. Round to nearest.
4. Resolve exact half-way cases away from zero.
5. Restore the sign.
6. Saturate the final result to the signed 32-bit range.

This rule avoids language-specific signed right-shift and signed-division behavior.

## Saturation

The final result of a public arithmetic primitive saturates to:

    minimum = -2147483648
    maximum =  2147483647

Intermediate multiplication and shifted-division numerators must fit in a signed 64-bit integer for every valid Phase 0 input.

Addition and subtraction in the vertical workload are range-proven by the reference generator. Candidate debug builds may check them. Timed builds need not add saturation branches where the contract proves the range.

## Scaled multiplication

Given signed raw integers a and b and a non-negative right shift S:

    exact result = (a * b) / 2^S

The baseline returns the rounded, saturated signed 32-bit result.

For inputs with FA and FB fractional bits and an output with FO fractional bits:

    S = FA + FB - FO

The v1 workload never requests a negative S.

## Scaled division

Given signed raw numerator n, non-zero signed raw denominator d, and a non-negative left shift S:

    exact result = (n * 2^S) / d

The baseline returns the rounded, saturated signed 32-bit result.

For a numerator with FN fractional bits, denominator with FD fractional bits, and output with FO fractional bits:

    S = FO + FD - FN

Division by zero is invalid input and is excluded from timed kernels. Correctness-mode APIs must report it rather than inventing a numeric result.

## Interpolation

Environment tables use clamped piecewise-linear interpolation.

For x between knots x0 and x1:

    fraction_q16 = divide_scaled(x - x0, x1 - x0, 16)
    y = y0 + multiply_scaled(y1 - y0, fraction_q16, 16)

The fraction is clamped to the unsigned range 0 through 65535. Inputs below the first knot return the first value. Inputs at or above the last knot return the last value.

## Environment table

Altitude knots, in kilometres:

    0, 2, 5, 10, 15, 20, 30, 40, 50, 70,
    100, 120, 200, 300, 500, 750, 1000, 1500, 2000

Density values, in kilograms per cubic metre:

    1.225
    1.00649
    0.736116
    0.41351
    0.194755
    0.08891
    0.01841
    0.003996
    0.001027
    0.00008283
    0.000000532
    0
    0
    0
    0
    0
    0
    0
    0

Gravity at each knot is generated from:

    g(h) = g0 * (EarthRadius / (EarthRadius + h))²

with:

    g0 = 0.00980665 km/s²
    EarthRadius = 6371 km

The timed workload interpolates the generated gravity table. It does not evaluate the gravity formula each step.

## Benchmark vehicle

| Parameter | Value |
|---|---:|
| Initial total mass | 500 t |
| Dry mass | 120 t |
| Initial propellant | 380 t |
| Thrust while firing | 7.6 MN |
| Propellant mass flow | 2.5 t/s |
| Nominal burn duration | 152 s |
| CdA | 10 m² |
| Initial altitude | 0 km |
| Initial velocity | 0 km/s |
| Fixed timestep | 0.125 s |
| Total steps | 2048 |
| Simulated duration | 256 s |

The vehicle is fictional. These values exist to exercise ascent, atmospheric interpolation, changing mass, engine cutoff, and vacuum coast.

## Vertical step order

State is sampled after each complete step. Each step performs:

1. Engine is active when propellant is positive and mission time is below 152 seconds.
2. Interpolate density and gravity at current altitude.
3. Compute signed velocity squared as velocity times its absolute value.
4. Compute signed drag force:

       speed2_q24 = multiply_scaled(velocity_q24, abs(velocity_q24), 24)
       rho_v2_q24 = multiply_scaled(density_q28, speed2_q24, 28)
       drag_q12 = multiply_scaled(rho_v2_q24, CdA_q16, 28)
       drag_q12 = round_divide_by_two(drag_q12)

5. Compute weight:

       weight_q12 = multiply_scaled(mass_q12, gravity_q28, 28)

6. Compute net force:

       net_force = thrust_if_active - weight - signed_drag

7. Compute acceleration:

       acceleration_q28 = divide_scaled(net_force_q12, mass_q12, 28)

8. Semi-implicit velocity update:

       delta_velocity_q24 =
           multiply_scaled(acceleration_q28, timestep_q12, 16)
       velocity_q24 += delta_velocity_q24

9. Semi-implicit altitude update using the new velocity:

       delta_altitude_q12 =
           multiply_scaled(velocity_q24, timestep_q12, 24)
       altitude_q12 += delta_altitude_q12

10. If the engine was active, subtract one timestep of mass flow from propellant and total mass. Clamp propellant to zero and total mass to dry mass.
11. If that subtraction reaches zero propellant, increment the cutoff event count once.
12. Advance mission time by one timestep.

Altitude is not clamped. The selected scenario does not return to the ground during the benchmark.

## Checkpoints

Correctness mode records states after these step counts:

    0, 1, 8, 64, 128, 256, 512, 1024,
    1216, 1280, 1600, 2048

Step 1216 corresponds to the nominal 152-second cutoff boundary.

Each checkpoint includes raw and interpreted values for:

- Time.
- Altitude.
- Velocity.
- Acceleration.
- Total mass.
- Propellant.
- Engine-active flag for the next step.
- Cutoff event count.

## Rolling checksum

Correctness mode computes 32-bit FNV-1a after every step. Each state contributes these signed 32-bit fields in little-endian byte order:

1. Time raw.
2. Altitude raw.
3. Velocity raw.
4. Acceleration raw.
5. Total mass raw.
6. Propellant raw.
7. Engine-active flag as 0 or 1.
8. Cutoff event count.

The checksum is excluded from the primary dynamics-only timing. A separate validation timing may include it.

## Benchmark modes

### Correctness mode

- Bounds and preconditions may be checked.
- Checkpoints and checksum are produced.
- Exact agreement with the generated fixed-point vectors is required.

### Dynamics timing mode

- Runs the 2048 steps.
- Excludes setup, table loading, rendering, checksum, and output.
- Exposes final state so the compiler cannot discard the loop.
- Uses the same arithmetic and step order as correctness mode.

### Primitive timing mode

- Executes 512 calls per primitive and requires identical results across three PAL VICE runs.
- Loads arithmetic operands from volatile C64 memory so whole-program optimization cannot replace the loop with a constant.
- Keeps the representative Q-format shift at compile time, matching the flight-dynamics call sites.
- Measures scaled multiplication with `2,048,000 * 2,632,453`, shift 28.
- Measures general scaled division with `11,059 / 2,048,000`, shift 28.
- Measures the specialized fraction path with Q12 values `4,096 / 8,192`.
- Uses a deterministic 32-bit wrapping accumulator outside the primitive call and validates it after timing.
- Subtracts the empty synchronized CIA boundary cost but reports loop, volatile-load, and accumulator overhead as part of the isolated runner.

## Fairness rule

An optimization is eligible only when it passes every exact vector and vertical checkpoint. If it changes the result, it is a different numeric model and must be reported separately rather than compared as the same benchmark.

