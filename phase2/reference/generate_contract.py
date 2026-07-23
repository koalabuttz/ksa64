#!/usr/bin/env python3
"""Generate the reviewed Phase 2 numeric and source-data contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from decimal import Decimal, ROUND_HALF_UP, getcontext
from pathlib import Path
from typing import Any


getcontext().prec = 80
I32_MIN = -(1 << 31)
I32_MAX = (1 << 31) - 1
CONTRACT = "ksa64.numeric.phase2-v1"
EARTH_ID = "earth.rotating-equatorial-simple-atmosphere.v1"


def dec(value: str | int | Decimal) -> Decimal:
    return value if isinstance(value, Decimal) else Decimal(str(value))


def round_away(value: Decimal) -> int:
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def raw(value: str | Decimal, fractional_bits: int) -> int:
    result = round_away(dec(value) * (1 << fractional_bits))
    if not I32_MIN <= result <= I32_MAX:
        raise OverflowError((value, fractional_bits, result))
    return result


def fnv1a(text: str) -> int:
    value = 2_166_136_261
    for byte in text.encode("utf-8"):
        value ^= byte
        value = value * 16_777_619 & 0xFFFF_FFFF
    return value


FORMATS: dict[str, dict[str, str | int]] = {
    "time": {"fractional_bits": 16, "unit": "s", "minimum": "0", "maximum": "4096"},
    "radius": {"fractional_bits": 12, "unit": "km", "minimum": "6376", "maximum": "8379"},
    "altitude": {"fractional_bits": 12, "unit": "km", "minimum": "-2", "maximum": "2000"},
    "velocity": {"fractional_bits": 24, "unit": "km/s", "minimum": "-16", "maximum": "16"},
    "acceleration": {"fractional_bits": 28, "unit": "km/s^2", "minimum": "-0.2", "maximum": "0.2"},
    "specific_angular_momentum": {"fractional_bits": 14, "unit": "km^2/s", "minimum": "-120000", "maximum": "120000"},
    "mass": {"fractional_bits": 12, "unit": "t", "minimum": "0", "maximum": "5000"},
    "mass_flow": {"fractional_bits": 16, "unit": "t/s", "minimum": "0", "maximum": "100"},
    "force": {"fractional_bits": 12, "unit": "MN", "minimum": "-1000", "maximum": "1000"},
    "density": {"fractional_bits": 28, "unit": "kg/m^3", "minimum": "0", "maximum": "1.5"},
    "speed_squared": {"fractional_bits": 20, "unit": "km^2/s^2", "minimum": "0", "maximum": "256"},
    "mach": {"fractional_bits": 16, "unit": "dimensionless", "minimum": "0", "maximum": "64"},
    "dynamic_pressure": {"fractional_bits": 16, "unit": "kPa", "minimum": "0", "maximum": "1000"},
    "coefficient": {"fractional_bits": 14, "unit": "dimensionless", "minimum": "0", "maximum": "4"},
    "area": {"fractional_bits": 16, "unit": "m^2", "minimum": "0", "maximum": "2000"},
    "specific_energy": {"fractional_bits": 24, "unit": "km^2/s^2", "minimum": "-120", "maximum": "120"},
    "gravitational_parameter": {"fractional_bits": 12, "unit": "km^3/s^2", "minimum": "398000", "maximum": "399000"},
}


def analyze_formats() -> dict[str, Any]:
    result: dict[str, Any] = {}
    for name, spec in FORMATS.items():
        bits = int(spec["fractional_bits"])
        scale = 1 << bits
        minimum = dec(str(spec["minimum"]))
        maximum = dec(str(spec["maximum"]))
        result[name] = {
            **spec,
            "storage": "i32",
            "scale": scale,
            "resolution": format(Decimal(1) / scale, "f"),
            "declared_raw_minimum": raw(minimum, bits),
            "declared_raw_maximum": raw(maximum, bits),
            "storage_minimum": format(Decimal(I32_MIN) / scale, "f"),
            "storage_maximum": format(Decimal(I32_MAX) / scale, "f"),
        }
    result["downrange_angle"] = {
        "storage": "i32 binary turn",
        "unit": "turn",
        "scale": 1 << 32,
        "resolution_turn": format(Decimal(1) / (1 << 32), "f"),
    }
    result["pitch_angle"] = {
        "storage": "u16 binary turn",
        "unit": "turn",
        "scale": 1 << 16,
        "accepted_raw_range": [0, 1 << 14],
    }
    result["trig"] = {
        "storage": "i16 Q1.15",
        "unit": "dimensionless",
        "fractional_bits": 15,
    }
    return result


def product(name: str, a: str, b: str) -> dict[str, Any]:
    left = FORMATS[a]
    right = FORMATS[b]
    left_raw = max(abs(raw(str(left["minimum"]), int(left["fractional_bits"]))), abs(raw(str(left["maximum"]), int(left["fractional_bits"]))))
    right_raw = max(abs(raw(str(right["minimum"]), int(right["fractional_bits"]))), abs(raw(str(right["maximum"]), int(right["fractional_bits"]))))
    value = left_raw * right_raw
    return {
        "name": name,
        "operands": [a, b],
        "maximum_raw_product": value,
        "required_bits_including_sign": value.bit_length() + 1,
    }


def range_proof() -> dict[str, Any]:
    products = [
        product("speed squared", "velocity", "velocity"),
        product("radius times velocity", "radius", "velocity"),
        product("radius times acceleration", "radius", "acceleration"),
        product("specific angular momentum squared", "specific_angular_momentum", "specific_angular_momentum"),
        product("density times speed squared", "density", "speed_squared"),
        product("dynamic pressure times area", "dynamic_pressure", "area"),
        product("mass flow times timestep envelope", "mass_flow", "time"),
    ]
    return {
        "coupled_constraints": [
            "6376 <= radius <= 8379 km",
            "radial and tangential velocity magnitudes are <= 16 km/s",
            "abs(specific angular momentum) <= 120000 km^2/s",
            "air-relative speed squared <= 256 km^2/s^2",
            "dynamic pressure <= 1000 kPa",
            "abs(net acceleration) <= 0.2 km/s^2",
            "0 < timestep <= 0.125 s",
            "all discrete events and pitch knots align to timestep boundaries",
        ],
        "widened_products": products,
        "maximum_required_bits_including_sign": max(item["required_bits_including_sign"] for item in products),
        "explicit_two_word_64_bit_intermediate_sufficient": max(item["required_bits_including_sign"] for item in products) <= 64,
    }


def trig_table() -> list[int]:
    return [round_away(Decimal(str(math.sin((math.pi / 2) * index / 256))) * 32767) for index in range(257)]


def sqrt_vectors() -> list[dict[str, int]]:
    values = [0, 1, 2, 3, 4, 15, 16, 17, 65535, 65536, 0x7FFF_FFFF, 0xFFFF_FFFF]
    return [{"input": value, "floor": math.isqrt(value)} for value in values]


def scenario_source() -> dict[str, Any]:
    return {
        "schema": "ksa64.scenario.phase2-v2",
        "scenario_id": "ksa64.phase2.ksa2a-200km.v1",
        "numeric_contract": CONTRACT,
        "environment": EARTH_ID,
        "timestep_s": "0.125",
        "duration_s": "900",
        "telemetry_stride_steps": 8,
        "payload_mass_t": "15",
        "initial": {"altitude_km": "0", "radial_velocity_km_s": "0", "surface_relative_velocity_km_s": "0"},
        "stages": [
            {"id": "s1", "dry_mass_t": "30", "propellant_mass_t": "400", "thrust_mn": "7.6", "mass_flow_t_s": "2.58", "max_burn_s": "155", "separation_delay_s": "1", "ignition_delay_s": "0", "reference_area_m2": "28", "aero_table": "booster.simple.v1", "separate": True},
            {"id": "s2", "dry_mass_t": "8", "propellant_mass_t": "84", "thrust_mn": "1.1", "mass_flow_t_s": "0.35", "max_burn_s": "240", "separation_delay_s": "0", "ignition_delay_s": "0.5", "reference_area_m2": "10", "aero_table": "upper.simple.v1", "separate": False},
        ],
        "pitch_program": [
            {"time_s": "0", "degrees_from_vertical": "0"},
            {"time_s": "10", "degrees_from_vertical": "0"},
            {"time_s": "30", "degrees_from_vertical": "17.357"},
            {"time_s": "70", "degrees_from_vertical": "39.181"},
            {"time_s": "120", "degrees_from_vertical": "49.798"},
            {"time_s": "155", "degrees_from_vertical": "72.872"},
            {"time_s": "220", "degrees_from_vertical": "89.100"},
            {"time_s": "400", "degrees_from_vertical": "90"},
        ],
        "aerodynamics": {
            "booster.simple.v1": [{"mach": "0", "cd": "0.30"}, {"mach": "0.8", "cd": "0.36"}, {"mach": "1", "cd": "0.55"}, {"mach": "1.2", "cd": "0.62"}, {"mach": "2", "cd": "0.40"}, {"mach": "5", "cd": "0.25"}, {"mach": "25", "cd": "0.20"}],
            "upper.simple.v1": [{"mach": "0", "cd": "0.25"}, {"mach": "1", "cd": "0.45"}, {"mach": "2", "cd": "0.30"}, {"mach": "5", "cd": "0.20"}, {"mach": "25", "cd": "0.18"}],
        },
    }


def schema() -> dict[str, Any]:
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "ksa64.scenario.phase2-v2",
        "type": "object",
        "required": ["schema", "scenario_id", "numeric_contract", "environment", "timestep_s", "duration_s", "telemetry_stride_steps", "payload_mass_t", "initial", "stages", "pitch_program", "aerodynamics"],
        "properties": {
            "schema": {"const": "ksa64.scenario.phase2-v2"},
            "scenario_id": {"type": "string", "minLength": 1},
            "numeric_contract": {"const": CONTRACT},
            "environment": {"const": EARTH_ID},
            "timestep_s": {"type": "string"},
            "duration_s": {"type": "string"},
            "telemetry_stride_steps": {"type": "integer", "minimum": 1},
            "payload_mass_t": {"type": "string"},
            "initial": {"type": "object"},
            "stages": {"type": "array", "minItems": 1, "maxItems": 4},
            "pitch_program": {"type": "array", "minItems": 2, "maxItems": 16},
            "aerodynamics": {"type": "object", "maxProperties": 4},
        },
        "additionalProperties": False,
    }


def rust_source(contract: dict[str, Any]) -> str:
    table = contract["trig_quarter_wave_q15"]
    rows = [", ".join(str(value) for value in table[index:index + 8]) for index in range(0, len(table), 8)]
    return "\n".join([
        "// Generated by phase2/reference/generate_contract.py.",
        "// Do not edit by hand.",
        "",
        f'pub const PHASE2_NUMERIC_CONTRACT: &str = "{CONTRACT}";',
        f"pub const PHASE2_NUMERIC_CONTRACT_ID: u32 = {fnv1a(CONTRACT)};",
        f"pub const PHASE2_ENVIRONMENT_ID: u32 = {fnv1a(EARTH_ID)};",
        f"pub const EARTH_RADIUS_Q12: i32 = {raw('6378.137', 12)};",
        f"pub const EARTH_MU_Q12: i32 = {raw('398600.4418', 12)};",
        f"pub const EARTH_ROTATION_TURNS_Q32: u32 = {round_away(dec('0.00007292115') / dec(str(2 * math.pi)) * (1 << 32))};",
        f"pub const INV_TWO_PI_Q30: i32 = {round_away(dec(1) / dec(str(2 * math.pi)) * (1 << 30))};",
        f"pub const EARTH_ROTATION_RAD_Q30: i32 = {round_away(dec('0.00007292115') * (1 << 30))};",
        "pub const SIN_QUARTER_Q15: &[i16; 257] = &[",
        *(f"    {row}," for row in rows),
        "];",
        "",
    ])


def outputs(root: Path) -> dict[Path, bytes]:
    contract = {
        "contract": CONTRACT,
        "contract_id": fnv1a(CONTRACT),
        "environment": {
            "id": EARTH_ID,
            "id_hash": fnv1a(EARTH_ID),
            "radius_km": "6378.137",
            "mu_km3_s2": "398600.4418",
            "rotation_rad_s": "0.00007292115",
        },
        "limits": {"maximum_stages": 4, "maximum_pitch_knots": 16, "maximum_aero_tables": 4, "maximum_mach_knots_per_table": 16},
        "formats": analyze_formats(),
        "range_proof": range_proof(),
        "sqrt_floor_vectors": sqrt_vectors(),
        "trig_quarter_wave_q15": trig_table(),
        "event_alignment": "All propulsion and guidance times are integral multiples of the physics timestep.",
    }
    result = {
        root / "phase2" / "contract-v1.json": (json.dumps(contract, indent=2) + "\n").encode(),
        root / "phase2" / "scenario-v2.schema.json": (json.dumps(schema(), indent=2) + "\n").encode(),
        root / "phase2" / "examples" / "ksa2a-200km.json": (json.dumps(scenario_source(), indent=2) + "\n").encode(),
        root / "phase2" / "generated" / "contract_v1.rs": rust_source(contract).encode(),
    }
    for path, data in list(result.items()):
        result[path.with_name(path.name + ".sha256")] = (hashlib.sha256(data).hexdigest() + "\n").encode()
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    failed = False
    for path, data in outputs(root).items():
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                print(f"stale or missing: {path.relative_to(root)}", file=sys.stderr)
                failed = True
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            print(path.relative_to(root))
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
