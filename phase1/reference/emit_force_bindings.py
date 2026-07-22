#!/usr/bin/env python3
"""Emit independent exact force-evaluation cases for the Phase 1 core."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from decimal import Decimal, ROUND_HALF_UP
from pathlib import Path


def scaled(value: str, fractional_bits: int) -> int:
    decimal = Decimal(value)
    magnitude = (abs(decimal) * (1 << fractional_bits)).to_integral_value(
        rounding=ROUND_HALF_UP
    )
    return -int(magnitude) if decimal < 0 else int(magnitude)


def rounded_ratio(numerator: int, denominator: int) -> int:
    if denominator <= 0 or numerator < 0:
        raise ValueError("rounded_ratio requires nonnegative numerator and positive denominator")
    quotient, remainder = divmod(numerator, denominator)
    return quotient + (1 if remainder * 2 >= denominator else 0)


def multiply_scaled(a: int, b: int, shift: int) -> int:
    magnitude = rounded_ratio(abs(a) * abs(b), 1 << shift)
    return -magnitude if (a < 0) != (b < 0) else magnitude


def divide_scaled(numerator: int, denominator: int, shift: int) -> int:
    if denominator == 0:
        raise ValueError("force fixture division by zero")
    magnitude = rounded_ratio(abs(numerator) << shift, abs(denominator))
    return -magnitude if (numerator < 0) != (denominator < 0) else magnitude


def halve_nonnegative(value: int) -> int:
    if value < 0:
        raise ValueError("drag magnitude must be nonnegative")
    return (value >> 1) + (value & 1)


def interpolate(x: int, xs: list[int], ys: list[int]) -> int:
    if x <= xs[0]:
        return ys[0]
    if x >= xs[-1]:
        return ys[-1]
    for index, upper in enumerate(xs[1:]):
        if x < upper:
            lower = xs[index]
            fraction = min(divide_scaled(x - lower, upper - lower, 16), 65_535)
            return ys[index] + multiply_scaled(ys[index + 1] - ys[index], fraction, 16)
    raise AssertionError("interpolation interval not found")


def scenario_values(root: Path) -> dict[str, int]:
    image = (root / "phase0" / "numeric" / "scenario-v1.bin").read_bytes()
    if len(image) != 76 or image[:4] != b"KSC1":
        raise ValueError("golden Phase 1 scenario image is invalid")
    return {
        "timestep": struct.unpack_from("<i", image, 16)[0],
        "dry_mass": struct.unpack_from("<i", image, 48)[0],
        "thrust": struct.unpack_from("<i", image, 52)[0],
        "mass_flow": struct.unpack_from("<i", image, 56)[0],
        "burn_duration": struct.unpack_from("<i", image, 60)[0],
        "cda": struct.unpack_from("<i", image, 64)[0],
    }


def build_cases() -> list[dict[str, int | str]]:
    root = Path(__file__).resolve().parents[2]
    scenario = scenario_values(root)
    vectors = json.loads(
        (root / "phase0" / "vectors" / "phase0-v1.json").read_text(encoding="utf-8")
    )
    environment = vectors["environment"]
    altitudes = environment["altitude_knots_q12"]
    densities = environment["density_q28"]
    gravities = environment["gravity_q28"]

    definitions = [
        ("pad_powered", "0", "0", "0", "500", "380"),
        ("upward_drag", "10", "3.5", "0.5", "475", "355"),
        ("downward_drag", "10", "3.5", "-0.5", "475", "355"),
        ("burn_time_cutoff", "152", "0", "0", "120", "1"),
        ("propellant_cutoff_vacuum", "100", "120", "2", "120", "0"),
        ("acceleration_envelope_escape", "0", "0", "8", "500", "380"),
    ]
    cases: list[dict[str, int | str]] = []
    for name, time, altitude, velocity, mass, propellant in definitions:
        time_raw = scaled(time, 16)
        altitude_raw = scaled(altitude, 12)
        velocity_raw = scaled(velocity, 24)
        mass_raw = scaled(mass, 12)
        propellant_raw = scaled(propellant, 12)
        density = interpolate(altitude_raw, altitudes, densities)
        gravity = interpolate(altitude_raw, altitudes, gravities)
        engine_active = propellant_raw > 0 and time_raw < scenario["burn_duration"]

        speed_squared = multiply_scaled(abs(velocity_raw), abs(velocity_raw), 28)
        density_speed_squared = multiply_scaled(density, speed_squared, 28)
        twice_drag = multiply_scaled(density_speed_squared, scenario["cda"], 24)
        drag_magnitude = halve_nonnegative(twice_drag)
        drag = -drag_magnitude if velocity_raw > 0 else drag_magnitude if velocity_raw < 0 else 0
        weight = multiply_scaled(mass_raw, gravity, 28)
        thrust = scenario["thrust"] if engine_active else 0
        net_force = thrust - weight + drag
        acceleration = divide_scaled(net_force, mass_raw, 28)
        expected_faults = (
            0x08
            if abs(net_force) > 2_048_000 or abs(acceleration) > 26_843_546
            else 0
        )
        cases.append(
            {
                "name": name,
                "time_q16": time_raw,
                "altitude_q12": altitude_raw,
                "velocity_q24": velocity_raw,
                "mass_q12": mass_raw,
                "propellant_q12": propellant_raw,
                "density_q28": density,
                "gravity_q28": gravity,
                "engine_active": 1 if engine_active else 0,
                "thrust_q12": thrust,
                "weight_q12": weight,
                "drag_q12": drag,
                "net_force_q12": net_force,
                "acceleration_q28": acceleration,
                "expected_faults": expected_faults,
            }
        )
    return cases


def build_source() -> str:
    cases = build_cases()
    lines = [
        "// Generated by phase1/reference/emit_force_bindings.py.",
        "// Do not edit by hand.",
        "",
        "#[derive(Clone, Copy)]",
        "pub struct ForceCase {",
        "    pub time_q16: i32,",
        "    pub altitude_q12: i32,",
        "    pub velocity_q24: i32,",
        "    pub mass_q12: i32,",
        "    pub propellant_q12: i32,",
        "    pub density_q28: i32,",
        "    pub gravity_q28: i32,",
        "    pub engine_active: u8,",
        "    pub thrust_q12: i32,",
        "    pub weight_q12: i32,",
        "    pub drag_q12: i32,",
        "    pub net_force_q12: i32,",
        "    pub acceleration_q28: i32,",
        "    pub expected_faults: u8,",
        "}",
        "",
        "pub const FORCE_CASES: &[ForceCase] = &[",
    ]
    for case in cases:
        lines.extend(
            [
                f'    // {case["name"]}',
                "    ForceCase {",
                f'        time_q16: {case["time_q16"]},',
                f'        altitude_q12: {case["altitude_q12"]},',
                f'        velocity_q24: {case["velocity_q24"]},',
                f'        mass_q12: {case["mass_q12"]},',
                f'        propellant_q12: {case["propellant_q12"]},',
                f'        density_q28: {case["density_q28"]},',
                f'        gravity_q28: {case["gravity_q28"]},',
                f'        engine_active: {case["engine_active"]},',
                f'        thrust_q12: {case["thrust_q12"]},',
                f'        weight_q12: {case["weight_q12"]},',
                f'        drag_q12: {case["drag_q12"]},',
                f'        net_force_q12: {case["net_force_q12"]},',
                f'        acceleration_q28: {case["acceleration_q28"]},',
                f'        expected_faults: {case["expected_faults"]},',
                "    },",
            ]
        )
    lines.extend(["];", ""])
    return "\n".join(lines)


def output_paths() -> tuple[Path, Path]:
    root = Path(__file__).resolve().parents[2]
    source = root / "phase1" / "generated" / "force_v1.rs"
    digest = root / "phase1" / "generated" / "force_v1.rs.sha256"
    return source, digest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when generated cases are stale")
    parser.add_argument("--stdout", action="store_true", help="write generated Rust to stdout")
    arguments = parser.parse_args()
    payload = build_source().encode("utf-8")
    digest = hashlib.sha256(payload).hexdigest()
    source_path, digest_path = output_paths()
    digest_payload = f"{digest}  {source_path.name}\n".encode("ascii")
    if arguments.stdout:
        sys.stdout.buffer.write(payload)
        return 0
    if arguments.check:
        stale = []
        if not source_path.exists() or source_path.read_bytes() != payload:
            stale.append(source_path)
        if not digest_path.exists() or digest_path.read_bytes() != digest_payload:
            stale.append(digest_path)
        if stale:
            print("Phase 1 force bindings are stale or missing:", file=sys.stderr)
            for path in stale:
                print(f"  {path}", file=sys.stderr)
            return 1
        print(f"Phase 1 force bindings are current: {digest}")
        return 0
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_bytes(payload)
    digest_path.write_bytes(digest_payload)
    print(f"Wrote {source_path}")
    print(f"Wrote {digest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
