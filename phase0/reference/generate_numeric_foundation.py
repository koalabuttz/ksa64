#!/usr/bin/env python3
"""Generate and verify the KSA64 Phase 1 numeric-foundation artifacts."""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import struct
import sys
from decimal import Decimal, ROUND_HALF_UP, getcontext
from pathlib import Path
from typing import Any


getcontext().prec = 80

I32_MIN = -(1 << 31)
I32_MAX = (1 << 31) - 1
U16_MAX = (1 << 16) - 1
CONTRACT = "ksa64.numeric.phase1-v1"
SCENARIO_SCHEMA = "ksa64.scenario"
SCENARIO_VERSION = 1
TELEMETRY_SCHEMA = "ksa64.telemetry"
TELEMETRY_VERSION = 1


def dec(value: str | int) -> Decimal:
    return Decimal(str(value))


def round_away(value: Decimal) -> int:
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def raw(value: str | Decimal, fractional_bits: int) -> int:
    result = round_away(dec(value) * (1 << fractional_bits))
    if not I32_MIN <= result <= I32_MAX:
        raise OverflowError(f"{value} does not fit signed Q{32 - fractional_bits}.{fractional_bits}")
    return result


def rounded_ratio(numerator: int, denominator: int) -> int:
    if denominator == 0:
        raise ZeroDivisionError
    negative = (numerator < 0) ^ (denominator < 0)
    quotient, remainder = divmod(abs(numerator), abs(denominator))
    if remainder * 2 >= abs(denominator):
        quotient += 1
    return -quotient if negative else quotient


def multiply_scaled(a: int, b: int, shift: int) -> int:
    result = rounded_ratio(a * b, 1 << shift)
    if not I32_MIN <= result <= I32_MAX:
        raise OverflowError(f"scaled product {result} does not fit i32")
    return result


def fnv1a(text: str) -> int:
    value = 2166136261
    for byte in text.encode("utf-8"):
        value ^= byte
        value = (value * 16777619) & 0xFFFFFFFF
    return value


FORMATS: dict[str, dict[str, Any]] = {
    "time": {"fractional_bits": 16, "unit": "s", "minimum": "0", "maximum": "4096"},
    "altitude": {"fractional_bits": 12, "unit": "km", "minimum": "-2", "maximum": "2000"},
    "velocity": {"fractional_bits": 24, "unit": "km/s", "minimum": "-8", "maximum": "8"},
    "acceleration": {"fractional_bits": 28, "unit": "km/s^2", "minimum": "-0.1", "maximum": "0.1"},
    "mass": {"fractional_bits": 12, "unit": "t", "minimum": "0", "maximum": "5000"},
    "mass_flow": {"fractional_bits": 16, "unit": "t/s", "minimum": "0", "maximum": "100"},
    "force": {"fractional_bits": 12, "unit": "MN", "minimum": "-200000", "maximum": "200000"},
    "net_force": {"fractional_bits": 12, "unit": "MN", "minimum": "-500", "maximum": "500"},
    "density": {"fractional_bits": 28, "unit": "kg/m^3", "minimum": "0", "maximum": "1.5"},
    "cda": {"fractional_bits": 16, "unit": "m^2", "minimum": "0", "maximum": "2000"},
    "speed_squared": {"fractional_bits": 20, "unit": "km^2/s^2", "minimum": "0", "maximum": "64"},
    "density_speed_squared": {"fractional_bits": 20, "unit": "MN/m^2 before CdA", "minimum": "0", "maximum": "96"},
}


def format_analysis() -> dict[str, Any]:
    output: dict[str, Any] = {}
    for name, spec in FORMATS.items():
        bits = spec["fractional_bits"]
        scale = 1 << bits
        storage_min = Decimal(I32_MIN) / scale
        storage_max = Decimal(I32_MAX) / scale
        declared_min = dec(spec["minimum"])
        declared_max = dec(spec["maximum"])
        largest_magnitude = max(abs(declared_min), abs(declared_max))
        positive_headroom = storage_max / largest_magnitude if largest_magnitude else Decimal("Infinity")
        output[name] = {
            "storage": "i32",
            "unit": spec["unit"],
            "fractional_bits": bits,
            "scale": scale,
            "resolution": format(Decimal(1) / scale, "f"),
            "storage_minimum": format(storage_min, "f"),
            "storage_maximum": format(storage_max, "f"),
            "declared_minimum": spec["minimum"],
            "declared_maximum": spec["maximum"],
            "declared_raw_minimum": raw(declared_min, bits),
            "declared_raw_maximum": raw(declared_max, bits),
            "positive_headroom_factor": format(positive_headroom, ".9f"),
        }
    output["fraction"] = {
        "storage": "u16",
        "unit": "dimensionless",
        "fractional_bits": 16,
        "scale": 1 << 16,
        "resolution": format(Decimal(1) / (1 << 16), "f"),
        "storage_minimum": "0",
        "storage_maximum": format(Decimal(U16_MAX) / (1 << 16), "f"),
        "declared_raw_minimum": 0,
        "declared_raw_maximum": U16_MAX,
    }
    return output


def product_case(name: str, left: str, right: str, output: str) -> dict[str, Any]:
    left_spec = FORMATS[left]
    right_spec = FORMATS[right]
    output_spec = FORMATS[output]
    left_raw = max(abs(raw(left_spec["minimum"], left_spec["fractional_bits"])), abs(raw(left_spec["maximum"], left_spec["fractional_bits"])))
    right_raw = max(abs(raw(right_spec["minimum"], right_spec["fractional_bits"])), abs(raw(right_spec["maximum"], right_spec["fractional_bits"])))
    product = left_raw * right_raw
    shift = left_spec["fractional_bits"] + right_spec["fractional_bits"] - output_spec["fractional_bits"]
    scaled = rounded_ratio(product, 1 << shift)
    return {
        "name": name,
        "left": left,
        "right": right,
        "output": output,
        "right_shift": shift,
        "maximum_raw_product": product,
        "required_product_bits_including_sign": product.bit_length() + 1,
        "maximum_scaled_raw": scaled,
        "scaled_result_fits_i32": scaled <= I32_MAX,
    }


def range_analysis() -> dict[str, Any]:
    products = [
        product_case("velocity squared", "velocity", "velocity", "speed_squared"),
        product_case("density times speed squared", "density", "speed_squared", "density_speed_squared"),
        product_case("density-speed term times CdA", "density_speed_squared", "cda", "force"),
        product_case("mass times acceleration", "mass", "acceleration", "force"),
        product_case("acceleration times timestep", "acceleration", "time", "velocity"),
        product_case("velocity times timestep", "velocity", "time", "altitude"),
        product_case("mass flow times timestep", "mass_flow", "time", "mass"),
    ]

    # Product envelopes involving time use the integration step, not the mission-duration range.
    timestep = raw("0.125", FORMATS["time"]["fractional_bits"])
    for case, source in zip(products[-3:], ("acceleration", "velocity", "mass_flow"), strict=True):
        source_spec = FORMATS[source]
        source_raw = max(abs(raw(source_spec["minimum"], source_spec["fractional_bits"])), abs(raw(source_spec["maximum"], source_spec["fractional_bits"])))
        product = source_raw * timestep
        case["maximum_raw_product"] = product
        case["required_product_bits_including_sign"] = product.bit_length() + 1
        case["maximum_scaled_raw"] = rounded_ratio(product, 1 << case["right_shift"])
        case["scaled_result_fits_i32"] = case["maximum_scaled_raw"] <= I32_MAX
        case["time_operand"] = "fixed timestep 0.125 s"

    division_cases = [
        {
            "name": "net force divided by mass",
            "shifted_numerator_bits_including_sign": (raw("500", 12) << 28).bit_length() + 1,
            "shifted_numerator_fits_signed_64": (raw("500", 12) << 28) <= (1 << 63) - 1,
            "final_result_requires_coupled_acceleration_constraint": True,
        },
        {
            "name": "interpolation fraction",
            "shifted_numerator_bits_including_sign": (raw("2002", 12) << 16).bit_length() + 1,
            "shifted_numerator_fits_signed_64": (raw("2002", 12) << 16) <= (1 << 63) - 1,
        },
    ]

    return {
        "phase1_envelope": {
            "maximum_duration_s": "4096",
            "altitude_km": ["-2", "2000"],
            "velocity_km_s": ["-8", "8"],
            "acceleration_km_s2": ["-0.1", "0.1"],
            "mass_t": ["0", "5000"],
            "density_kg_m3": ["0", "1.5"],
            "cda_m2": ["0", "2000"],
            "component_force_mn": ["-100000", "100000"],
            "pre_halving_drag_term_mn": ["-200000", "200000"],
            "net_force_mn": ["-500", "500"],
        },
        "coupled_constraints": [
            "density * velocity^2 <= 96 kg/(m*s^2)",
            "abs(net_force / mass) <= 0.1 km/s^2 while mass > 0",
            "total_mass - propellant >= dry_mass > 0",
            "burn_duration is an integer multiple of timestep",
            "0 <= propellant <= total_mass",
            "0 < timestep <= 0.125 s for the Phase 1 baseline",
        ],
        "widened_products": products,
        "widened_divisions": division_cases,
        "all_scaled_results_fit_i32": all(case["scaled_result_fits_i32"] for case in products),
        "all_shifted_division_numerators_fit_signed_64": all(case["shifted_numerator_fits_signed_64"] for case in division_cases),
        "maximum_required_product_bits_including_sign": max(case["required_product_bits_including_sign"] for case in products),
        "maximum_shifted_division_bits_including_sign": max(case["shifted_numerator_bits_including_sign"] for case in division_cases),
    }


def integrate_constant_velocity(dt: Decimal, steps: int, altitude: Decimal, velocity: Decimal) -> list[dict[str, Any]]:
    dt_raw = raw(dt, 16)
    altitude_raw = raw(altitude, 12)
    velocity_raw = raw(velocity, 24)
    checkpoints = {0, 1, 8, steps}
    output = []
    for step in range(steps + 1):
        if step in checkpoints:
            output.append({"step": step, "altitude_q12": altitude_raw, "velocity_q24": velocity_raw})
        if step != steps:
            altitude_raw += multiply_scaled(velocity_raw, dt_raw, 28)
    return output


def integrate_constant_acceleration(dt: Decimal, steps: int, acceleration: Decimal) -> dict[str, Any]:
    dt_raw = raw(dt, 16)
    acceleration_raw = raw(acceleration, 28)
    altitude_raw = 0
    velocity_raw = 0
    checkpoints = {0, 1, 8, steps}
    output = []
    for step in range(steps + 1):
        if step in checkpoints:
            output.append({"step": step, "altitude_q12": altitude_raw, "velocity_q24": velocity_raw})
        if step != steps:
            velocity_raw += multiply_scaled(acceleration_raw, dt_raw, 20)
            altitude_raw += multiply_scaled(velocity_raw, dt_raw, 28)

    duration = dt * steps
    exact_altitude = acceleration * duration * duration / 2
    fixed_altitude = Decimal(altitude_raw) / (1 << 12)
    return {
        "timestep_s": format(dt, "f"),
        "steps": steps,
        "duration_s": format(duration, "f"),
        "acceleration_km_s2": format(acceleration, "f"),
        "checkpoints": output,
        "continuous_solution": {
            "altitude_km": format(exact_altitude, "f"),
            "velocity_km_s": format(acceleration * duration, "f"),
        },
        "fixed_final": {
            "altitude_km": format(fixed_altitude, "f"),
            "velocity_km_s": format(Decimal(velocity_raw) / (1 << 24), "f"),
        },
        "altitude_error_m": format((fixed_altitude - exact_altitude) * 1000, "f"),
        "predicted_semi_implicit_bias_m": format(acceleration * duration * dt * 500, "f"),
    }


def integrate_constant_mass_flow() -> dict[str, Any]:
    dt_raw = raw("0.125", 16)
    flow_raw = raw("2.5", 16)
    mass_raw = raw("500", 12)
    propellant_raw = raw("380", 12)
    dry_raw = raw("120", 12)
    checkpoints = {0, 1, 8, 1216}
    output = []
    for step in range(1217):
        if step in checkpoints:
            output.append({"step": step, "mass_q12": mass_raw, "propellant_q12": propellant_raw})
        if step != 1216:
            consumed = min(multiply_scaled(flow_raw, dt_raw, 20), propellant_raw)
            propellant_raw -= consumed
            mass_raw = max(dry_raw, mass_raw - consumed)
    return {"timestep_s": "0.125", "steps": 1216, "checkpoints": output}


def analytic_cases() -> dict[str, Any]:
    constant_acceleration = integrate_constant_acceleration(dec("0.125"), 64, dec("0.01"))
    convergence = [
        integrate_constant_acceleration(dec(dt), round_away(dec("8") / dec(dt)), dec("0.01"))
        for dt in ("0.25", "0.125", "0.0625")
    ]
    return {
        "constant_velocity": {
            "description": "No-force motion must preserve velocity and advance position exactly for this representable case.",
            "timestep_s": "0.125",
            "steps": 64,
            "initial_altitude_km": "0.25",
            "velocity_km_s": "0.125",
            "checkpoints": integrate_constant_velocity(dec("0.125"), 64, dec("0.25"), dec("0.125")),
        },
        "constant_acceleration": constant_acceleration,
        "negative_acceleration": integrate_constant_acceleration(dec("0.125"), 64, dec("-0.01")),
        "constant_mass_flow": integrate_constant_mass_flow(),
        "semi_implicit_convergence": convergence,
    }


def crc32(payload: bytes) -> int:
    return binascii.crc32(payload) & 0xFFFFFFFF


def load_example() -> dict[str, Any]:
    path = Path(__file__).resolve().parents[1] / "numeric" / "examples" / "phase1-vertical.json"
    return json.loads(path.read_text(encoding="utf-8"))


def validate_example(data: dict[str, Any]) -> None:
    required = {"schema", "version", "id", "model", "numeric_contract", "timestep_s", "steps", "telemetry_stride", "seed", "initial", "vehicle", "environment"}
    if set(data) != required:
        raise ValueError(f"scenario keys differ: expected {sorted(required)}, got {sorted(data)}")
    if data["schema"] != SCENARIO_SCHEMA or data["version"] != SCENARIO_VERSION:
        raise ValueError("unsupported scenario schema")
    if data["model"] != "vertical-v1" or data["numeric_contract"] != CONTRACT:
        raise ValueError("scenario model or numeric contract mismatch")
    for group, fields in {
        "initial": {"altitude_km", "velocity_km_s", "mass_t", "propellant_t"},
        "vehicle": {"dry_mass_t", "thrust_mn", "mass_flow_t_s", "burn_duration_s", "cda_m2"},
        "environment": {"id"},
    }.items():
        if set(data[group]) != fields:
            raise ValueError(f"scenario {group} keys differ")

    def bounded(value: str, quantity: str) -> Decimal:
        parsed = dec(value)
        specification = FORMATS[quantity]
        if not dec(specification["minimum"]) <= parsed <= dec(specification["maximum"]):
            raise ValueError(f"{quantity} value {value} escapes the Phase 1 envelope")
        raw(parsed, specification["fractional_bits"])
        return parsed

    timestep = bounded(data["timestep_s"], "time")
    altitude = bounded(data["initial"]["altitude_km"], "altitude")
    velocity = bounded(data["initial"]["velocity_km_s"], "velocity")
    total_mass = bounded(data["initial"]["mass_t"], "mass")
    propellant = bounded(data["initial"]["propellant_t"], "mass")
    dry_mass = bounded(data["vehicle"]["dry_mass_t"], "mass")
    thrust = dec(data["vehicle"]["thrust_mn"])
    if not dec("0") <= thrust <= dec("100000"):
        raise ValueError("thrust escapes the Phase 1 component-force envelope")
    bounded(data["vehicle"]["mass_flow_t_s"], "mass_flow")
    burn_duration = bounded(data["vehicle"]["burn_duration_s"], "time")
    bounded(data["vehicle"]["cda_m2"], "cda")
    if not (
        total_mass >= dry_mass > 0
        and 0 <= propellant <= total_mass
        and total_mass - propellant >= dry_mass
    ):
        raise ValueError("scenario mass invariants failed")
    if not (0 < timestep <= dec("0.125")):
        raise ValueError("scenario timestep is outside the Phase 1 baseline")
    if timestep * data["steps"] > dec("4096"):
        raise ValueError("scenario duration escapes the Phase 1 envelope")
    if data["telemetry_stride"] > data["steps"]:
        raise ValueError("telemetry stride exceeds scenario length")
    if burn_duration > timestep * data["steps"]:
        raise ValueError("burn duration exceeds scenario duration")
    if burn_duration % timestep != 0:
        raise ValueError("burn duration must align to a physics-step boundary")
    if thrust / dry_mass + dec("0.012") > dec("0.1"):
        raise ValueError("conservative powered acceleration bound failed")
    _ = altitude, velocity


def scenario_image() -> tuple[bytes, dict[str, Any]]:
    data = load_example()
    validate_example(data)
    body = bytearray()

    def append(fmt: str, value: Any) -> None:
        body.extend(struct.pack("<" + fmt, value))

    body.extend(b"KSC1")
    append("H", SCENARIO_VERSION)
    append("H", 76)
    append("I", fnv1a(CONTRACT))
    append("I", fnv1a(data["id"]))
    append("i", raw(data["timestep_s"], 16))
    append("I", data["steps"])
    append("H", data["telemetry_stride"])
    append("H", 0)
    append("I", data["seed"])
    append("i", raw(data["initial"]["altitude_km"], 12))
    append("i", raw(data["initial"]["velocity_km_s"], 24))
    append("i", raw(data["initial"]["mass_t"], 12))
    append("i", raw(data["initial"]["propellant_t"], 12))
    append("i", raw(data["vehicle"]["dry_mass_t"], 12))
    append("i", raw(data["vehicle"]["thrust_mn"], 12))
    append("i", raw(data["vehicle"]["mass_flow_t_s"], 16))
    append("i", raw(data["vehicle"]["burn_duration_s"], 16))
    append("i", raw(data["vehicle"]["cda_m2"], 16))
    append("I", fnv1a(data["environment"]["id"]))
    if len(body) != 72:
        raise AssertionError(len(body))
    append("I", crc32(body))
    return bytes(body), {"length": len(body), "crc32": f"{crc32(body[:-4]):08x}", "hex": body.hex()}


def telemetry_image() -> tuple[bytes, dict[str, Any]]:
    scenario = load_example()
    header = bytearray()
    header.extend(b"KST1")
    header.extend(struct.pack("<HHHHIIiHH", TELEMETRY_VERSION, 32, 40, 0, fnv1a(CONTRACT), fnv1a(scenario["id"]), raw(scenario["timestep_s"], 16), scenario["telemetry_stride"], 0))
    if len(header) != 28:
        raise AssertionError(len(header))
    header.extend(struct.pack("<I", crc32(header)))

    frames = []
    frame_values = [
        (0, 0, 0, 0, 0, raw("500", 12), raw("380", 12), 1, 0, 0x811C9DC5),
        (8, raw("1", 16), raw("0.004", 12), raw("0.008", 24), raw("0.008", 28), raw("497.5", 12), raw("377.5", 12), 1, 0, 0x12345678),
    ]
    for values in frame_values:
        frame = bytearray(struct.pack("<IiiiiiiHHI", *values))
        if len(frame) != 36:
            raise AssertionError(len(frame))
        frame.extend(struct.pack("<I", crc32(frame)))
        frames.append(bytes(frame))
    payload = bytes(header) + b"".join(frames)
    return payload, {
        "header_length": 32,
        "frame_length": 40,
        "frame_count": len(frames),
        "length": len(payload),
        "header_crc32": f"{crc32(header[:-4]):08x}",
        "frame_crc32": [f"{crc32(frame[:-4]):08x}" for frame in frames],
        "hex": payload.hex(),
    }


def build_document() -> dict[str, Any]:
    scenario_bytes, scenario_metadata = scenario_image()
    telemetry_bytes, telemetry_metadata = telemetry_image()
    analysis = range_analysis()
    if (
        not analysis["all_scaled_results_fit_i32"]
        or not analysis["all_shifted_division_numerators_fit_signed_64"]
        or analysis["maximum_required_product_bits_including_sign"] > 64
    ):
        raise AssertionError("numeric envelope escaped the selected arithmetic")
    return {
        "contract": CONTRACT,
        "rounding": "nearest, exact halves away from zero",
        "storage_byte_order": "little-endian",
        "overflow_policy": "range-proven hot path; saturating public primitive sets a sticky fault; abort at step boundary",
        "integrator": {"name": "semi-implicit Euler", "timestep_s": "0.125", "physics_rate_hz": 8, "telemetry_stride": 8},
        "formats": format_analysis(),
        "range_analysis": analysis,
        "analytic_cases": analytic_cases(),
        "binary_fixtures": {"scenario": scenario_metadata, "telemetry": telemetry_metadata},
        "artifact_sha256": {
            "scenario-v1.bin": hashlib.sha256(scenario_bytes).hexdigest(),
            "telemetry-v1.bin": hashlib.sha256(telemetry_bytes).hexdigest(),
        },
    }


def encoded_json() -> bytes:
    return (json.dumps(build_document(), indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode("utf-8")


def outputs() -> dict[Path, bytes]:
    root = Path(__file__).resolve().parents[1] / "numeric"
    scenario_bytes, _ = scenario_image()
    telemetry_bytes, _ = telemetry_image()
    json_payload = encoded_json()
    result = {
        root / "numeric-v1.json": json_payload,
        root / "scenario-v1.bin": scenario_bytes,
        root / "telemetry-v1.bin": telemetry_bytes,
    }
    for path, payload in list(result.items()):
        result[path.with_suffix(path.suffix + ".sha256")] = f"{hashlib.sha256(payload).hexdigest()}  {path.name}\n".encode("ascii")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when checked-in artifacts are stale")
    parser.add_argument("--stdout", action="store_true", help="write numeric-v1.json to stdout")
    arguments = parser.parse_args()
    generated = outputs()
    if arguments.stdout:
        sys.stdout.buffer.write(generated[next(path for path in generated if path.name == "numeric-v1.json")])
        return 0
    if arguments.check:
        stale = [str(path) for path, payload in generated.items() if not path.exists() or path.read_bytes() != payload]
        if stale:
            print("Numeric-foundation artifacts are stale or missing:", file=sys.stderr)
            for path in stale:
                print(f"  {path}", file=sys.stderr)
            return 1
        print("Numeric-foundation artifacts are current: PASS")
        return 0
    for path, payload in generated.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
