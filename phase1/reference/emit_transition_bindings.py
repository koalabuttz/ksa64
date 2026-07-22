#!/usr/bin/env python3
"""Emit independent exact single-step transition cases for the Phase 1 core."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from emit_force_bindings import (
    divide_scaled,
    halve_nonnegative,
    interpolate,
    multiply_scaled,
    scaled,
    scenario_values,
)


INVALID_INPUT = 0x08
MIN_ALTITUDE_Q12 = -8_192
MAX_ALTITUDE_Q12 = 8_192_000
MIN_VELOCITY_Q24 = -134_217_728
MAX_VELOCITY_Q24 = 134_217_728
MAX_NET_FORCE_Q12 = 2_048_000
MAX_ACCELERATION_Q28 = 26_843_546


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
        ("initial_powered", 0, "0", "0", "0", "500", "380"),
        ("upward_drag", 80, "10", "3.5", "0.5", "475", "355"),
        ("downward_drag", 80, "10", "3.5", "-0.5", "475", "355"),
        ("propellant_exhaustion", 1208, "151", "100", "1", "120.1", "0.1"),
        ("burn_boundary", 1215, "151.875", "100", "1", "121", "1"),
        ("already_cut_off", 1216, "152", "100", "1", "120.6875", "0.6875"),
        ("force_envelope_escape", 0, "0", "0", "8", "500", "380"),
    ]
    cases: list[dict[str, int | str]] = []
    for name, step, time, altitude, velocity, mass, propellant in definitions:
        current = {
            "step": step,
            "time": scaled(time, 16),
            "altitude": scaled(altitude, 12),
            "velocity": scaled(velocity, 24),
            "mass": scaled(mass, 12),
            "propellant": scaled(propellant, 12),
        }
        density = interpolate(current["altitude"], altitudes, densities)
        gravity = interpolate(current["altitude"], altitudes, gravities)
        engine_active = (
            current["propellant"] > 0 and current["time"] < scenario["burn_duration"]
        )

        speed_squared = multiply_scaled(abs(current["velocity"]), abs(current["velocity"]), 28)
        density_speed_squared = multiply_scaled(density, speed_squared, 28)
        twice_drag = multiply_scaled(density_speed_squared, scenario["cda"], 24)
        drag_magnitude = halve_nonnegative(twice_drag)
        drag = (
            -drag_magnitude
            if current["velocity"] > 0
            else drag_magnitude if current["velocity"] < 0 else 0
        )
        weight = multiply_scaled(current["mass"], gravity, 28)
        thrust = scenario["thrust"] if engine_active else 0
        net_force = thrust - weight + drag
        acceleration = divide_scaled(net_force, current["mass"], 28)
        faults = (
            INVALID_INPUT
            if abs(net_force) > MAX_NET_FORCE_Q12
            or abs(acceleration) > MAX_ACCELERATION_Q28
            else 0
        )

        next_state = dict(current)
        consumed = 0
        engine_cutoff = False
        if faults == 0:
            delta_velocity = multiply_scaled(acceleration, scenario["timestep"], 20)
            next_state["velocity"] += delta_velocity
            delta_altitude = multiply_scaled(
                next_state["velocity"], scenario["timestep"], 28
            )
            next_state["altitude"] += delta_altitude
            next_state["time"] += scenario["timestep"]
            requested = (
                multiply_scaled(scenario["mass_flow"], scenario["timestep"], 20)
                if engine_active
                else 0
            )
            consumed = min(requested, current["propellant"])
            next_state["propellant"] -= consumed
            next_state["mass"] -= consumed
            next_state["step"] += 1
            if (
                not MIN_ALTITUDE_Q12 <= next_state["altitude"] <= MAX_ALTITUDE_Q12
                or not MIN_VELOCITY_Q24 <= next_state["velocity"] <= MAX_VELOCITY_Q24
                or next_state["mass"] < scenario["dry_mass"]
                or next_state["propellant"] < 0
                or next_state["propellant"] > next_state["mass"]
            ):
                faults = INVALID_INPUT
                next_state = dict(current)
                consumed = 0
            else:
                engine_cutoff = engine_active and (
                    next_state["propellant"] == 0
                    or next_state["time"] >= scenario["burn_duration"]
                )

        cases.append(
            {
                "name": name,
                "step": current["step"],
                "time_q16": current["time"],
                "altitude_q12": current["altitude"],
                "velocity_q24": current["velocity"],
                "mass_q12": current["mass"],
                "propellant_q12": current["propellant"],
                "succeeds": 1 if faults == 0 else 0,
                "next_step": next_state["step"],
                "next_time_q16": next_state["time"],
                "next_altitude_q12": next_state["altitude"],
                "next_velocity_q24": next_state["velocity"],
                "next_acceleration_q28": acceleration if faults == 0 else 0,
                "next_mass_q12": next_state["mass"],
                "next_propellant_q12": next_state["propellant"],
                "consumed_q12": consumed,
                "engine_cutoff": 1 if engine_cutoff else 0,
                "expected_faults": faults,
            }
        )
    return cases


def build_source() -> str:
    cases = build_cases()
    fields = [
        ("step", "u32"),
        ("time_q16", "i32"),
        ("altitude_q12", "i32"),
        ("velocity_q24", "i32"),
        ("mass_q12", "i32"),
        ("propellant_q12", "i32"),
        ("succeeds", "u8"),
        ("next_step", "u32"),
        ("next_time_q16", "i32"),
        ("next_altitude_q12", "i32"),
        ("next_velocity_q24", "i32"),
        ("next_acceleration_q28", "i32"),
        ("next_mass_q12", "i32"),
        ("next_propellant_q12", "i32"),
        ("consumed_q12", "i32"),
        ("engine_cutoff", "u8"),
        ("expected_faults", "u8"),
    ]
    lines = [
        "// Generated by phase1/reference/emit_transition_bindings.py.",
        "// Do not edit by hand.",
        "",
        "#[derive(Clone, Copy)]",
        "pub struct TransitionCase {",
    ]
    lines.extend(f"    pub {name}: {kind}," for name, kind in fields)
    lines.extend(["}", "", "pub const TRANSITION_CASES: &[TransitionCase] = &["])
    for case in cases:
        lines.append(f'    // {case["name"]}')
        lines.append("    TransitionCase {")
        lines.extend(f'        {name}: {case[name]},' for name, _ in fields)
        lines.append("    },")
    lines.extend(["];", ""])
    return "\n".join(lines)


def output_paths() -> tuple[Path, Path]:
    root = Path(__file__).resolve().parents[2]
    source = root / "phase1" / "generated" / "transition_v1.rs"
    digest = root / "phase1" / "generated" / "transition_v1.rs.sha256"
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
            print("Phase 1 transition bindings are stale or missing:", file=sys.stderr)
            for path in stale:
                print(f"  {path}", file=sys.stderr)
            return 1
        print(f"Phase 1 transition bindings are current: {digest}")
        return 0
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_bytes(payload)
    digest_path.write_bytes(digest_payload)
    print(f"Wrote {source_path}")
    print(f"Wrote {digest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
