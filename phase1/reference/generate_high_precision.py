#!/usr/bin/env python3
"""Generate independent high-precision Phase 1 mission comparison evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from decimal import Decimal, ROUND_HALF_UP, getcontext
from pathlib import Path

from emit_mission_bindings import build_summary


getcontext().prec = 80

ROOT = Path(__file__).resolve().parents[2]
RESULT_PATH = ROOT / "phase1" / "high-precision-v1.json"
RESULT_DIGEST_PATH = ROOT / "phase1" / "high-precision-v1.json.sha256"
BINDING_PATH = ROOT / "phase1" / "generated" / "high_precision_v1.rs"
BINDING_DIGEST_PATH = ROOT / "phase1" / "generated" / "high_precision_v1.rs.sha256"


def dec(value: object) -> Decimal:
    return Decimal(str(value))


def formatted(value: Decimal) -> str:
    return format(value.quantize(Decimal("0.000000000000000001")), "f")


def q16(value: Decimal) -> int:
    magnitude = (abs(value) * (1 << 16)).to_integral_value(rounding=ROUND_HALF_UP)
    return -int(magnitude) if value < 0 else int(magnitude)


@dataclass(frozen=True)
class Inputs:
    timestep: Decimal
    steps: int
    initial_altitude: Decimal
    initial_velocity: Decimal
    initial_mass: Decimal
    initial_propellant: Decimal
    dry_mass: Decimal
    thrust: Decimal
    mass_flow: Decimal
    burn_duration: Decimal
    cda: Decimal
    altitude_knots: tuple[Decimal, ...]
    density_knots: tuple[Decimal, ...]
    gravity_knots: tuple[Decimal, ...]


@dataclass(frozen=True)
class State:
    time: Decimal
    altitude: Decimal
    velocity: Decimal
    acceleration: Decimal
    mass: Decimal
    propellant: Decimal


def load_inputs() -> Inputs:
    scenario = json.loads(
        (ROOT / "phase0" / "numeric" / "examples" / "phase1-vertical.json").read_text(
            encoding="utf-8"
        )
    )
    vectors = json.loads(
        (ROOT / "phase0" / "vectors" / "phase0-v1.json").read_text(encoding="utf-8")
    )
    environment = vectors["environment"]
    initial = scenario["initial"]
    vehicle = scenario["vehicle"]
    constants = vectors["constants"]["physical"]
    radius = dec(constants["earth_radius_km"])
    g0 = dec(constants["g0_km_s2"])
    altitudes = tuple(dec(value) for value in environment["altitude_knots_km"])
    gravities = tuple(g0 * (radius / (radius + altitude)) ** 2 for altitude in altitudes)
    return Inputs(
        timestep=dec(scenario["timestep_s"]),
        steps=int(scenario["steps"]),
        initial_altitude=dec(initial["altitude_km"]),
        initial_velocity=dec(initial["velocity_km_s"]),
        initial_mass=dec(initial["mass_t"]),
        initial_propellant=dec(initial["propellant_t"]),
        dry_mass=dec(vehicle["dry_mass_t"]),
        thrust=dec(vehicle["thrust_mn"]),
        mass_flow=dec(vehicle["mass_flow_t_s"]),
        burn_duration=dec(vehicle["burn_duration_s"]),
        cda=dec(vehicle["cda_m2"]),
        altitude_knots=altitudes,
        density_knots=tuple(dec(value) for value in environment["density_kg_m3"]),
        gravity_knots=gravities,
    )


def interpolate(value: Decimal, xs: tuple[Decimal, ...], ys: tuple[Decimal, ...]) -> Decimal:
    if value <= xs[0]:
        return ys[0]
    if value >= xs[-1]:
        return ys[-1]
    for index in range(len(xs) - 1):
        if value < xs[index + 1]:
            fraction = (value - xs[index]) / (xs[index + 1] - xs[index])
            return ys[index] + (ys[index + 1] - ys[index]) * fraction
    raise AssertionError("interpolation interval not found")


def mass_at(inputs: Inputs, time: Decimal) -> tuple[Decimal, Decimal]:
    consumed = min(inputs.mass_flow * min(time, inputs.burn_duration), inputs.initial_propellant)
    return max(inputs.dry_mass, inputs.initial_mass - consumed), inputs.initial_propellant - consumed


def acceleration_at(
    inputs: Inputs,
    time: Decimal,
    altitude: Decimal,
    velocity: Decimal,
    powered: bool | None = None,
) -> Decimal:
    mass, propellant = mass_at(inputs, time)
    active = time < inputs.burn_duration and propellant > 0 if powered is None else powered
    density = interpolate(altitude, inputs.altitude_knots, inputs.density_knots)
    gravity = interpolate(altitude, inputs.altitude_knots, inputs.gravity_knots)
    drag = dec("0.5") * density * velocity * abs(velocity) * inputs.cda
    thrust = inputs.thrust if active else Decimal(0)
    return (thrust - mass * gravity - drag) / mass


def run_same_step(inputs: Inputs) -> State:
    time = Decimal(0)
    altitude = inputs.initial_altitude
    velocity = inputs.initial_velocity
    acceleration = Decimal(0)
    mass = inputs.initial_mass
    propellant = inputs.initial_propellant
    for _ in range(inputs.steps):
        active = time < inputs.burn_duration and propellant > 0
        acceleration = acceleration_at(inputs, time, altitude, velocity, active)
        velocity += acceleration * inputs.timestep
        altitude += velocity * inputs.timestep
        if active:
            consumed = min(inputs.mass_flow * inputs.timestep, propellant)
            mass -= consumed
            propellant -= consumed
        time += inputs.timestep
    return State(time, altitude, velocity, acceleration, mass, propellant)


def run_rk4(inputs: Inputs, refinement: int) -> State:
    timestep = inputs.timestep / refinement
    total_steps = inputs.steps * refinement
    time = Decimal(0)
    altitude = inputs.initial_altitude
    velocity = inputs.initial_velocity
    two = Decimal(2)
    six = Decimal(6)
    for _ in range(total_steps):
        powered = time < inputs.burn_duration
        half = timestep / two

        h1 = velocity
        v1 = acceleration_at(inputs, time, altitude, velocity, powered)
        h2 = velocity + v1 * half
        v2 = acceleration_at(
            inputs,
            time + half,
            altitude + h1 * half,
            velocity + v1 * half,
            powered,
        )
        h3 = velocity + v2 * half
        v3 = acceleration_at(
            inputs,
            time + half,
            altitude + h2 * half,
            velocity + v2 * half,
            powered,
        )
        h4 = velocity + v3 * timestep
        v4 = acceleration_at(
            inputs,
            time + timestep,
            altitude + h3 * timestep,
            velocity + v3 * timestep,
            powered,
        )
        altitude += timestep * (h1 + two * h2 + two * h3 + h4) / six
        velocity += timestep * (v1 + two * v2 + two * v3 + v4) / six
        time += timestep

    mass, propellant = mass_at(inputs, time)
    acceleration = acceleration_at(inputs, time, altitude, velocity)
    return State(time, altitude, velocity, acceleration, mass, propellant)


def state_dict(state: State) -> dict[str, str]:
    return {
        "time_s": formatted(state.time),
        "altitude_km": formatted(state.altitude),
        "velocity_km_s": formatted(state.velocity),
        "acceleration_km_s2": formatted(state.acceleration),
        "mass_t": formatted(state.mass),
        "propellant_t": formatted(state.propellant),
    }


def delta_dict(left: State, right: State) -> dict[str, str]:
    return {
        "altitude_m": formatted((left.altitude - right.altitude) * 1_000),
        "velocity_m_s": formatted((left.velocity - right.velocity) * 1_000),
        "acceleration_m_s2": formatted((left.acceleration - right.acceleration) * 1_000),
    }


def fixed_state() -> State:
    summary = build_summary()
    return State(
        dec(summary["time"]) / (1 << 16),
        dec(summary["altitude"]) / (1 << 12),
        dec(summary["velocity"]) / (1 << 24),
        dec(summary["acceleration"]) / (1 << 28),
        dec(summary["mass"]) / (1 << 12),
        dec(summary["propellant"]) / (1 << 12),
    )


def build_evidence() -> tuple[dict[str, object], str]:
    inputs = load_inputs()
    fixed = fixed_state()
    same_step = run_same_step(inputs)
    rk4 = run_rk4(inputs, 32)
    rk4_confirmation = run_rk4(inputs, 64)
    convergence_altitude_m = abs(rk4.altitude - rk4_confirmation.altitude) * 1_000
    convergence_velocity_m_s = abs(rk4.velocity - rk4_confirmation.velocity) * 1_000
    if convergence_altitude_m >= dec("0.001") or convergence_velocity_m_s >= dec("0.00001"):
        raise ValueError("refined RK4 confirmation escaped the declared convergence tolerance")

    total_delta = delta_dict(fixed, rk4_confirmation)
    evidence: dict[str, object] = {
        "schema": "ksa64.phase1.high-precision",
        "version": 1,
        "method": {
            "decimal_precision_digits": getcontext().prec,
            "same_step": "semi-implicit Euler at 0.125 s using unquantized source values",
            "reference": "RK4 at 0.00390625 s using unquantized source values",
            "confirmation": "RK4 at 0.001953125 s using unquantized source values",
            "environment": "source density knots and analytic gravity evaluated at table knots, linearly interpolated",
        },
        "fixed_exact": state_dict(fixed),
        "decimal_same_step": state_dict(same_step),
        "decimal_rk4_reference": state_dict(rk4),
        "decimal_rk4_confirmation": state_dict(rk4_confirmation),
        "fixed_minus_same_step": delta_dict(fixed, same_step),
        "same_step_minus_rk4_confirmation": delta_dict(same_step, rk4_confirmation),
        "fixed_minus_rk4_confirmation": total_delta,
        "rk4_reference_minus_confirmation": delta_dict(rk4, rk4_confirmation),
        "convergence_tolerance": {
            "altitude_m_less_than": "0.001",
            "velocity_m_s_less_than": "0.00001",
        },
    }
    binding = "\n".join(
        [
            "// Generated by phase1/reference/generate_high_precision.py.",
            "// Do not edit by hand.",
            "",
            f"pub const FINAL_ALTITUDE_ERROR_M_Q16: i32 = {q16(dec(total_delta['altitude_m']))};",
            f"pub const FINAL_VELOCITY_ERROR_M_S_Q16: i32 = {q16(dec(total_delta['velocity_m_s']))};",
            "",
        ]
    )
    return evidence, binding


def checked_write(path: Path, payload: bytes, check: bool) -> bool:
    if check:
        return path.exists() and path.read_bytes() == payload
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when evidence is stale")
    arguments = parser.parse_args()
    evidence, binding = build_evidence()
    result_payload = (json.dumps(evidence, indent=2) + "\n").encode("utf-8")
    binding_payload = binding.encode("utf-8")
    result_digest = hashlib.sha256(result_payload).hexdigest()
    binding_digest = hashlib.sha256(binding_payload).hexdigest()
    artifacts = [
        (RESULT_PATH, result_payload),
        (RESULT_DIGEST_PATH, f"{result_digest}  {RESULT_PATH.name}\n".encode("ascii")),
        (BINDING_PATH, binding_payload),
        (BINDING_DIGEST_PATH, f"{binding_digest}  {BINDING_PATH.name}\n".encode("ascii")),
    ]
    stale = [path for path, payload in artifacts if not checked_write(path, payload, arguments.check)]
    if stale:
        print("Phase 1 high-precision artifacts are stale or missing:")
        for path in stale:
            print(f"  {path}")
        return 1
    if arguments.check:
        print(f"Phase 1 high-precision evidence is current: {result_digest}")
    else:
        for path, _ in artifacts:
            print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
