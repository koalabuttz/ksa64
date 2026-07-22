#!/usr/bin/env python3
"""Generate deterministic Phase 0 arithmetic and vertical-flight vectors."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from decimal import Decimal, ROUND_HALF_EVEN, ROUND_HALF_UP, getcontext
from pathlib import Path
from typing import Any


getcontext().prec = 60

I32_MIN = -(1 << 31)
I32_MAX = (1 << 31) - 1
FNV_OFFSET = 2166136261
FNV_PRIME = 16777619
CONTRACT_VERSION = "phase0-v1"

FORMATS = {
    "time": 12,
    "altitude": 12,
    "velocity": 24,
    "acceleration": 28,
    "mass": 12,
    "force": 12,
    "density": 28,
    "cda": 16,
    "fraction": 16,
}

ALTITUDE_KNOTS = [
    "0",
    "2",
    "5",
    "10",
    "15",
    "20",
    "30",
    "40",
    "50",
    "70",
    "100",
    "120",
    "200",
    "300",
    "500",
    "750",
    "1000",
    "1500",
    "2000",
]

DENSITY_VALUES = [
    "1.225",
    "1.00649",
    "0.736116",
    "0.41351",
    "0.194755",
    "0.08891",
    "0.01841",
    "0.003996",
    "0.001027",
    "0.00008283",
    "0.000000532",
    "0",
    "0",
    "0",
    "0",
    "0",
    "0",
    "0",
    "0",
]

PHYSICAL_CONSTANTS = {
    "g0_km_s2": "0.00980665",
    "earth_radius_km": "6371",
    "initial_mass_t": "500",
    "dry_mass_t": "120",
    "initial_propellant_t": "380",
    "thrust_mn": "7.6",
    "mass_flow_t_s": "2.5",
    "burn_duration_s": "152",
    "cda_m2": "10",
    "timestep_s": "0.125",
}

CHECKPOINT_STEPS = [0, 1, 8, 64, 128, 256, 512, 1024, 1216, 1280, 1600, 2048]
TOTAL_STEPS = 2048

SATURATION_COUNT = 0


def d(value: str | int) -> Decimal:
    return Decimal(str(value))


def round_decimal(value: Decimal) -> int:
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def raw_from_decimal(value: Decimal, fractional_bits: int) -> int:
    return saturate_i32(round_decimal(value * (1 << fractional_bits)))


def saturate_i32(value: int) -> int:
    global SATURATION_COUNT
    if value > I32_MAX:
        SATURATION_COUNT += 1
        return I32_MAX
    if value < I32_MIN:
        SATURATION_COUNT += 1
        return I32_MIN
    return value


def rounded_ratio(numerator: int, denominator: int) -> int:
    if denominator == 0:
        raise ZeroDivisionError("Phase 0 division denominator must be non-zero")

    negative = (numerator < 0) ^ (denominator < 0)
    magnitude_numerator = abs(numerator)
    magnitude_denominator = abs(denominator)
    quotient, remainder = divmod(magnitude_numerator, magnitude_denominator)

    if remainder * 2 >= magnitude_denominator:
        quotient += 1

    return -quotient if negative else quotient


def multiply_scaled(a: int, b: int, shift: int) -> int:
    if shift < 0:
        raise ValueError("Phase 0 multiply shift must be non-negative")
    return saturate_i32(rounded_ratio(a * b, 1 << shift))


def divide_scaled(numerator: int, denominator: int, shift: int) -> int:
    if shift < 0:
        raise ValueError("Phase 0 divide shift must be non-negative")
    return saturate_i32(rounded_ratio(numerator << shift, denominator))


def divide_by_two(value: int) -> int:
    return saturate_i32(rounded_ratio(value, 2))


def exact_decimal(raw: int, fractional_bits: int) -> str:
    return format(Decimal(raw) / Decimal(1 << fractional_bits), "f")


def readable_decimal(value: Decimal) -> str:
    quantum = Decimal("0.000000000000000001")
    return format(value.quantize(quantum, rounding=ROUND_HALF_EVEN), "f")


def interpolate_fixed(x: int, xs: list[int], ys: list[int]) -> int:
    if len(xs) != len(ys) or not xs:
        raise ValueError("Interpolation tables must be non-empty and equally sized")
    if x <= xs[0]:
        return ys[0]
    if x >= xs[-1]:
        return ys[-1]

    for index in range(len(xs) - 1):
        x0 = xs[index]
        x1 = xs[index + 1]
        if x < x1:
            fraction = divide_scaled(x - x0, x1 - x0, FORMATS["fraction"])
            fraction = max(0, min(65535, fraction))
            delta = multiply_scaled(
                ys[index + 1] - ys[index],
                fraction,
                FORMATS["fraction"],
            )
            return saturate_i32(ys[index] + delta)

    raise AssertionError("Interpolation interval was not found")


def interpolate_decimal(
    x: Decimal,
    xs: list[Decimal],
    ys: list[Decimal],
) -> Decimal:
    if x <= xs[0]:
        return ys[0]
    if x >= xs[-1]:
        return ys[-1]

    for index in range(len(xs) - 1):
        x0 = xs[index]
        x1 = xs[index + 1]
        if x < x1:
            fraction = (x - x0) / (x1 - x0)
            return ys[index] + (ys[index + 1] - ys[index]) * fraction

    raise AssertionError("Interpolation interval was not found")


def gravity_at(altitude_km: Decimal) -> Decimal:
    g0 = d(PHYSICAL_CONSTANTS["g0_km_s2"])
    radius = d(PHYSICAL_CONSTANTS["earth_radius_km"])
    return g0 * (radius / (radius + altitude_km)) ** 2


ALTITUDES_DECIMAL = [d(value) for value in ALTITUDE_KNOTS]
DENSITIES_DECIMAL = [d(value) for value in DENSITY_VALUES]
GRAVITIES_DECIMAL = [gravity_at(value) for value in ALTITUDES_DECIMAL]

ALTITUDES_RAW = [
    raw_from_decimal(value, FORMATS["altitude"]) for value in ALTITUDES_DECIMAL
]
DENSITIES_RAW = [
    raw_from_decimal(value, FORMATS["density"]) for value in DENSITIES_DECIMAL
]
GRAVITIES_RAW = [
    raw_from_decimal(value, FORMATS["acceleration"])
    for value in GRAVITIES_DECIMAL
]


RAW_CONSTANTS = {
    "g0_q28": raw_from_decimal(d(PHYSICAL_CONSTANTS["g0_km_s2"]), 28),
    "earth_radius_q12": raw_from_decimal(
        d(PHYSICAL_CONSTANTS["earth_radius_km"]), 12
    ),
    "initial_mass_q12": raw_from_decimal(
        d(PHYSICAL_CONSTANTS["initial_mass_t"]), 12
    ),
    "dry_mass_q12": raw_from_decimal(d(PHYSICAL_CONSTANTS["dry_mass_t"]), 12),
    "initial_propellant_q12": raw_from_decimal(
        d(PHYSICAL_CONSTANTS["initial_propellant_t"]), 12
    ),
    "thrust_q12": raw_from_decimal(d(PHYSICAL_CONSTANTS["thrust_mn"]), 12),
    "mass_flow_q12": raw_from_decimal(
        d(PHYSICAL_CONSTANTS["mass_flow_t_s"]), 12
    ),
    "burn_duration_q12": raw_from_decimal(
        d(PHYSICAL_CONSTANTS["burn_duration_s"]), 12
    ),
    "cda_q16": raw_from_decimal(d(PHYSICAL_CONSTANTS["cda_m2"]), 16),
    "timestep_q12": raw_from_decimal(d(PHYSICAL_CONSTANTS["timestep_s"]), 12),
}


@dataclass
class FixedState:
    time: int
    altitude: int
    velocity: int
    acceleration: int
    mass: int
    propellant: int
    cutoff_events: int


@dataclass
class DecimalState:
    time: Decimal
    altitude: Decimal
    velocity: Decimal
    acceleration: Decimal
    mass: Decimal
    propellant: Decimal
    cutoff_events: int


def fixed_engine_active(state: FixedState) -> bool:
    return (
        state.propellant > 0
        and state.time < RAW_CONSTANTS["burn_duration_q12"]
    )


def decimal_engine_active(state: DecimalState) -> bool:
    return (
        state.propellant > 0
        and state.time < d(PHYSICAL_CONSTANTS["burn_duration_s"])
    )


def step_fixed(state: FixedState) -> FixedState:
    engine_active = fixed_engine_active(state)
    density = interpolate_fixed(
        state.altitude,
        ALTITUDES_RAW,
        DENSITIES_RAW,
    )
    gravity = interpolate_fixed(
        state.altitude,
        ALTITUDES_RAW,
        GRAVITIES_RAW,
    )

    speed_squared = multiply_scaled(
        state.velocity,
        abs(state.velocity),
        FORMATS["velocity"],
    )
    rho_v2 = multiply_scaled(
        density,
        speed_squared,
        FORMATS["density"],
    )
    drag = multiply_scaled(
        rho_v2,
        RAW_CONSTANTS["cda_q16"],
        28,
    )
    drag = divide_by_two(drag)

    weight = multiply_scaled(
        state.mass,
        gravity,
        FORMATS["acceleration"],
    )
    thrust = RAW_CONSTANTS["thrust_q12"] if engine_active else 0
    net_force = thrust - weight - drag
    if not I32_MIN <= net_force <= I32_MAX:
        raise OverflowError("Net force escaped signed 32-bit range")

    acceleration = divide_scaled(
        net_force,
        state.mass,
        FORMATS["acceleration"],
    )
    delta_velocity = multiply_scaled(
        acceleration,
        RAW_CONSTANTS["timestep_q12"],
        16,
    )
    velocity = state.velocity + delta_velocity
    if not I32_MIN <= velocity <= I32_MAX:
        raise OverflowError("Velocity escaped signed 32-bit range")

    delta_altitude = multiply_scaled(
        velocity,
        RAW_CONSTANTS["timestep_q12"],
        FORMATS["velocity"],
    )
    altitude = state.altitude + delta_altitude
    if not I32_MIN <= altitude <= I32_MAX:
        raise OverflowError("Altitude escaped signed 32-bit range")

    mass = state.mass
    propellant = state.propellant
    cutoff_events = state.cutoff_events

    if engine_active:
        consumed = multiply_scaled(
            RAW_CONSTANTS["mass_flow_q12"],
            RAW_CONSTANTS["timestep_q12"],
            FORMATS["mass"],
        )
        consumed = min(consumed, propellant)
        propellant -= consumed
        mass = max(RAW_CONSTANTS["dry_mass_q12"], mass - consumed)
        if propellant == 0:
            cutoff_events += 1

    time = state.time + RAW_CONSTANTS["timestep_q12"]

    return FixedState(
        time=time,
        altitude=altitude,
        velocity=velocity,
        acceleration=acceleration,
        mass=mass,
        propellant=propellant,
        cutoff_events=cutoff_events,
    )


def step_decimal(state: DecimalState) -> DecimalState:
    engine_active = decimal_engine_active(state)
    density = interpolate_decimal(
        state.altitude,
        ALTITUDES_DECIMAL,
        DENSITIES_DECIMAL,
    )
    gravity = interpolate_decimal(
        state.altitude,
        ALTITUDES_DECIMAL,
        GRAVITIES_DECIMAL,
    )

    cda = d(PHYSICAL_CONSTANTS["cda_m2"])
    drag = d("0.5") * density * state.velocity * abs(state.velocity) * cda
    weight = state.mass * gravity
    thrust = d(PHYSICAL_CONSTANTS["thrust_mn"]) if engine_active else d(0)
    acceleration = (thrust - weight - drag) / state.mass
    velocity = state.velocity + acceleration * d(PHYSICAL_CONSTANTS["timestep_s"])
    altitude = state.altitude + velocity * d(PHYSICAL_CONSTANTS["timestep_s"])

    mass = state.mass
    propellant = state.propellant
    cutoff_events = state.cutoff_events
    if engine_active:
        consumed = min(
            d(PHYSICAL_CONSTANTS["mass_flow_t_s"])
            * d(PHYSICAL_CONSTANTS["timestep_s"]),
            propellant,
        )
        propellant -= consumed
        mass = max(d(PHYSICAL_CONSTANTS["dry_mass_t"]), mass - consumed)
        if propellant == 0:
            cutoff_events += 1

    time = state.time + d(PHYSICAL_CONSTANTS["timestep_s"])
    return DecimalState(
        time=time,
        altitude=altitude,
        velocity=velocity,
        acceleration=acceleration,
        mass=mass,
        propellant=propellant,
        cutoff_events=cutoff_events,
    )


def fnv1a_word(hash_value: int, value: int) -> int:
    unsigned = value & 0xFFFFFFFF
    for shift in (0, 8, 16, 24):
        hash_value ^= (unsigned >> shift) & 0xFF
        hash_value = (hash_value * FNV_PRIME) & 0xFFFFFFFF
    return hash_value


def hash_fixed_state(hash_value: int, state: FixedState) -> int:
    fields = [
        state.time,
        state.altitude,
        state.velocity,
        state.acceleration,
        state.mass,
        state.propellant,
        1 if fixed_engine_active(state) else 0,
        state.cutoff_events,
    ]
    for field in fields:
        hash_value = fnv1a_word(hash_value, field)
    return hash_value


def fixed_checkpoint(step: int, state: FixedState) -> dict[str, Any]:
    raw = {
        "time_q12": state.time,
        "altitude_q12": state.altitude,
        "velocity_q24": state.velocity,
        "acceleration_q28": state.acceleration,
        "mass_q12": state.mass,
        "propellant_q12": state.propellant,
        "engine_active": 1 if fixed_engine_active(state) else 0,
        "cutoff_events": state.cutoff_events,
    }
    interpreted = {
        "time_s": exact_decimal(state.time, 12),
        "altitude_km": exact_decimal(state.altitude, 12),
        "velocity_km_s": exact_decimal(state.velocity, 24),
        "acceleration_km_s2": exact_decimal(state.acceleration, 28),
        "mass_t": exact_decimal(state.mass, 12),
        "propellant_t": exact_decimal(state.propellant, 12),
    }
    return {"step": step, "raw": raw, "interpreted": interpreted}


def decimal_checkpoint(step: int, state: DecimalState) -> dict[str, Any]:
    return {
        "step": step,
        "time_s": readable_decimal(state.time),
        "altitude_km": readable_decimal(state.altitude),
        "velocity_km_s": readable_decimal(state.velocity),
        "acceleration_km_s2": readable_decimal(state.acceleration),
        "mass_t": readable_decimal(state.mass),
        "propellant_t": readable_decimal(state.propellant),
        "engine_active": 1 if decimal_engine_active(state) else 0,
        "cutoff_events": state.cutoff_events,
    }


def generate_arithmetic_vectors() -> dict[str, Any]:
    multiply_specs = [
        ("q16_identity", 65536, 65536, 16),
        ("q16_positive", 98304, 147456, 16),
        ("q16_negative_a", -98304, 147456, 16),
        ("q16_negative_b", 98304, -147456, 16),
        ("q16_double_negative", -98304, -147456, 16),
        ("half_away_positive", 1, 1, 1),
        ("half_away_negative", -1, 1, 1),
        ("below_half_positive", 1, 1, 2),
        ("exact_half_positive", 2, 1, 2),
        ("exact_half_negative", -2, 1, 2),
        (
            "representative_weight",
            RAW_CONSTANTS["initial_mass_q12"],
            RAW_CONSTANTS["g0_q28"],
            28,
        ),
        (
            "representative_delta_velocity",
            raw_from_decimal(d("0.0054"), 28),
            RAW_CONSTANTS["timestep_q12"],
            16,
        ),
        ("saturate_positive", I32_MAX, I32_MAX, 0),
        ("saturate_negative", I32_MIN, I32_MAX, 0),
    ]

    divide_specs = [
        ("q16_identity", 65536, 65536, 16),
        ("q16_half", 32768, 65536, 16),
        ("q16_negative", -32768, 65536, 16),
        ("integer_half_away_positive", 1, 2, 0),
        ("integer_half_away_negative", -1, 2, 0),
        ("integer_below_half", 1, 3, 0),
        (
            "representative_acceleration",
            raw_from_decimal(d("2.7"), 12),
            RAW_CONSTANTS["initial_mass_q12"],
            28,
        ),
        ("negative_denominator", 7, -3, 4),
        ("double_negative", -7, -3, 4),
        ("saturate_positive", I32_MAX, 1, 31),
        ("saturate_negative", I32_MIN, 1, 31),
    ]

    multiply_vectors = [
        {
            "name": name,
            "a": a,
            "b": b,
            "shift": shift,
            "expected": multiply_scaled(a, b, shift),
        }
        for name, a, b, shift in multiply_specs
    ]
    divide_vectors = [
        {
            "name": name,
            "numerator": numerator,
            "denominator": denominator,
            "shift": shift,
            "expected": divide_scaled(numerator, denominator, shift),
        }
        for name, numerator, denominator, shift in divide_specs
    ]

    query_altitudes = [
        "-1",
        "0",
        "1",
        "2",
        "3.5",
        "10",
        "55",
        "100",
        "119.999",
        "120",
        "500",
        "2500",
    ]
    interpolation_vectors = []
    for altitude_text in query_altitudes:
        altitude_raw = raw_from_decimal(d(altitude_text), FORMATS["altitude"])
        interpolation_vectors.append(
            {
                "altitude_km": altitude_text,
                "altitude_q12": altitude_raw,
                "density_q28": interpolate_fixed(
                    altitude_raw,
                    ALTITUDES_RAW,
                    DENSITIES_RAW,
                ),
                "gravity_q28": interpolate_fixed(
                    altitude_raw,
                    ALTITUDES_RAW,
                    GRAVITIES_RAW,
                ),
            }
        )

    return {
        "multiply_scaled": multiply_vectors,
        "divide_scaled": divide_vectors,
        "interpolation": interpolation_vectors,
        "invalid_inputs": {
            "division_by_zero": "error",
            "negative_shift": "error",
        },
    }


def generate_vertical_vectors() -> dict[str, Any]:
    global SATURATION_COUNT
    SATURATION_COUNT = 0

    fixed = FixedState(
        time=0,
        altitude=0,
        velocity=0,
        acceleration=0,
        mass=RAW_CONSTANTS["initial_mass_q12"],
        propellant=RAW_CONSTANTS["initial_propellant_q12"],
        cutoff_events=0,
    )
    high_precision = DecimalState(
        time=d(0),
        altitude=d(0),
        velocity=d(0),
        acceleration=d(0),
        mass=d(PHYSICAL_CONSTANTS["initial_mass_t"]),
        propellant=d(PHYSICAL_CONSTANTS["initial_propellant_t"]),
        cutoff_events=0,
    )

    fixed_checkpoints = [fixed_checkpoint(0, fixed)]
    high_precision_checkpoints = [decimal_checkpoint(0, high_precision)]
    checksum = FNV_OFFSET

    for step in range(1, TOTAL_STEPS + 1):
        fixed = step_fixed(fixed)
        high_precision = step_decimal(high_precision)
        checksum = hash_fixed_state(checksum, fixed)

        if step in CHECKPOINT_STEPS:
            fixed_checkpoints.append(fixed_checkpoint(step, fixed))
            high_precision_checkpoints.append(
                decimal_checkpoint(step, high_precision)
            )

    if SATURATION_COUNT != 0:
        raise AssertionError(
            f"Vertical workload unexpectedly saturated {SATURATION_COUNT} result(s)"
        )
    if fixed.time != raw_from_decimal(d("256"), FORMATS["time"]):
        raise AssertionError("Fixed workload did not end at 256 seconds")
    if fixed.mass != RAW_CONSTANTS["dry_mass_q12"] or fixed.propellant != 0:
        raise AssertionError("Fixed workload did not consume the expected propellant")
    if fixed.cutoff_events != 1:
        raise AssertionError("Fixed workload must produce exactly one cutoff event")

    cutoff = next(
        checkpoint
        for checkpoint in fixed_checkpoints
        if checkpoint["step"] == 1216
    )
    if cutoff["raw"]["propellant_q12"] != 0:
        raise AssertionError("Step 1216 must be the propellant cutoff boundary")
    if cutoff["raw"]["engine_active"] != 0:
        raise AssertionError("Engine must be inactive after the cutoff boundary")

    return {
        "total_steps": TOTAL_STEPS,
        "timestep_q12": RAW_CONSTANTS["timestep_q12"],
        "checkpoint_steps": CHECKPOINT_STEPS,
        "fixed_checkpoints": fixed_checkpoints,
        "high_precision_checkpoints": high_precision_checkpoints,
        "final_fnv1a32": f"0x{checksum:08x}",
        "saturation_count": SATURATION_COUNT,
    }


def run_self_tests() -> None:
    global SATURATION_COUNT
    SATURATION_COUNT = 0

    assert multiply_scaled(1, 1, 1) == 1
    assert multiply_scaled(-1, 1, 1) == -1
    assert multiply_scaled(1, 1, 2) == 0
    assert divide_scaled(1, 2, 0) == 1
    assert divide_scaled(-1, 2, 0) == -1
    assert divide_scaled(1, 3, 0) == 0
    assert multiply_scaled(I32_MAX, I32_MAX, 0) == I32_MAX
    assert multiply_scaled(I32_MIN, I32_MAX, 0) == I32_MIN

    expected_saturations = 2
    if SATURATION_COUNT != expected_saturations:
        raise AssertionError("Arithmetic saturation self-test count changed")

    SATURATION_COUNT = 0
    one_mn_per_tonne_q28 = divide_scaled(1 << 12, 1 << 12, 28)
    if one_mn_per_tonne_q28 != 1 << 28:
        raise AssertionError("Scaled-unit dimensional identity failed")


def build_document() -> dict[str, Any]:
    run_self_tests()

    document = {
        "metadata": {
            "contract_version": CONTRACT_VERSION,
            "generator": "phase0/reference/generate_vectors.py",
            "decimal_precision_digits": getcontext().prec,
            "integer_encoding": "signed 32-bit two's complement",
            "rounding": "nearest, exact halves away from zero",
            "overflow": "saturate final public primitive result to signed 32-bit",
        },
        "formats": {
            name: {"fractional_bits": bits, "scale": 1 << bits}
            for name, bits in FORMATS.items()
        },
        "constants": {
            "physical": PHYSICAL_CONSTANTS,
            "raw": RAW_CONSTANTS,
        },
        "environment": {
            "altitude_knots_km": ALTITUDE_KNOTS,
            "altitude_knots_q12": ALTITUDES_RAW,
            "density_kg_m3": DENSITY_VALUES,
            "density_q28": DENSITIES_RAW,
            "gravity_km_s2": [
                readable_decimal(value) for value in GRAVITIES_DECIMAL
            ],
            "gravity_q28": GRAVITIES_RAW,
        },
        "arithmetic": generate_arithmetic_vectors(),
        "vertical": generate_vertical_vectors(),
    }
    return document


def encoded_document() -> bytes:
    return (
        json.dumps(
            build_document(),
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
        )
        + "\n"
    ).encode("utf-8")


def output_paths() -> tuple[Path, Path]:
    project_root = Path(__file__).resolve().parents[2]
    vector_path = project_root / "phase0" / "vectors" / "phase0-v1.json"
    hash_path = project_root / "phase0" / "vectors" / "phase0-v1.sha256"
    return vector_path, hash_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if checked-in vectors differ from freshly generated output",
    )
    parser.add_argument(
        "--stdout",
        action="store_true",
        help="write the generated JSON to stdout without changing files",
    )
    arguments = parser.parse_args()

    payload = encoded_document()
    digest = hashlib.sha256(payload).hexdigest()
    vector_path, hash_path = output_paths()
    hash_payload = f"{digest}  {vector_path.name}\n".encode("ascii")

    if arguments.stdout:
        sys.stdout.buffer.write(payload)
        return 0

    if arguments.check:
        failures = []
        if not vector_path.exists() or vector_path.read_bytes() != payload:
            failures.append(str(vector_path))
        if not hash_path.exists() or hash_path.read_bytes() != hash_payload:
            failures.append(str(hash_path))
        if failures:
            print("Generated Phase 0 artifacts are stale or missing:", file=sys.stderr)
            for failure in failures:
                print(f"  {failure}", file=sys.stderr)
            return 1
        print(f"Phase 0 vectors are current: {digest}")
        return 0

    vector_path.parent.mkdir(parents=True, exist_ok=True)
    vector_path.write_bytes(payload)
    hash_path.write_bytes(hash_payload)
    print(f"Wrote {vector_path}")
    print(f"Wrote {hash_path}")
    print(f"SHA-256 {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

