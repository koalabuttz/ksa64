#!/usr/bin/env python3
"""Generate independent floating-point evidence for the Phase 2 coast integrators."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

MU = 398600.4418
EARTH_RADIUS = 6378.137
DT = 0.125
PERIGEE_ALTITUDE = 180.0
APOGEE_ALTITUDE = 220.0


def derivative(state, h):
    radius, radial_velocity, angle = state
    return (
        radial_velocity,
        h * h / (radius ** 3) - MU / (radius * radius),
        h / (radius * radius),
    )


def energy(state, h):
    radius, radial_velocity, _ = state
    tangential = h / radius
    return 0.5 * (radial_velocity * radial_velocity + tangential * tangential) - MU / radius


def semi_step(state, h, dt):
    radius, radial_velocity, angle = state
    acceleration = h * h / (radius ** 3) - MU / (radius * radius)
    radial_velocity += acceleration * dt
    radius += radial_velocity * dt
    angle += h / (radius * radius) * dt
    return radius, radial_velocity, angle


def midpoint_step(state, h, dt):
    first = derivative(state, h)
    middle = tuple(value + 0.5 * dt * delta for value, delta in zip(state, first, strict=True))
    second = derivative(middle, h)
    return tuple(value + dt * delta for value, delta in zip(state, second, strict=True))


def rk4_step(state, h, dt):
    k1 = derivative(state, h)
    s2 = tuple(value + 0.5 * dt * delta for value, delta in zip(state, k1, strict=True))
    k2 = derivative(s2, h)
    s3 = tuple(value + 0.5 * dt * delta for value, delta in zip(state, k2, strict=True))
    k3 = derivative(s3, h)
    s4 = tuple(value + dt * delta for value, delta in zip(state, k3, strict=True))
    k4 = derivative(s4, h)
    return tuple(value + dt * (a + 2*b + 2*c + d) / 6 for value, a, b, c, d in zip(state, k1, k2, k3, k4, strict=True))


def run(stepper, initial, h, dt, steps):
    state = initial
    initial_energy = energy(state, h)
    max_energy_excursion = 0.0
    minimum_radius = state[0]
    maximum_radius = state[0]
    for _ in range(steps):
        state = stepper(state, h, dt)
        maximum_radius = max(maximum_radius, state[0])
        minimum_radius = min(minimum_radius, state[0])
        max_energy_excursion = max(max_energy_excursion, abs((energy(state, h) - initial_energy) / initial_energy))
    return {
        "final_radius_km": state[0],
        "final_radial_velocity_km_s": state[1],
        "final_angle_rad": state[2],
        "minimum_altitude_km": minimum_radius - EARTH_RADIUS,
        "maximum_altitude_km": maximum_radius - EARTH_RADIUS,
        "maximum_relative_energy_excursion": max_energy_excursion,
    }


def payload():
    rp = EARTH_RADIUS + PERIGEE_ALTITUDE
    ra = EARTH_RADIUS + APOGEE_ALTITUDE
    semi_major = (rp + ra) / 2
    h = math.sqrt(2 * MU * rp * ra / (rp + ra))
    period = 2 * math.pi * math.sqrt(semi_major ** 3 / MU)
    base_steps = round(period / DT)
    duration = base_steps * DT
    initial = (rp, 0.0, 0.0)
    semi = run(semi_step, initial, h, DT, base_steps)
    midpoint = run(midpoint_step, initial, h, DT, base_steps)
    rk4_32 = run(rk4_step, initial, h, DT / 32, base_steps * 32)
    rk4_64 = run(rk4_step, initial, h, DT / 64, base_steps * 64)
    convergence = {
        "radius_m": abs(rk4_32["final_radius_km"] - rk4_64["final_radius_km"]) * 1000,
        "radial_velocity_mm_s": abs(rk4_32["final_radial_velocity_km_s"] - rk4_64["final_radial_velocity_km_s"]) * 1_000_000,
        "angle_microrad": abs(rk4_32["final_angle_rad"] - rk4_64["final_angle_rad"]) * 1_000_000,
    }
    return {
        "model": "two-body equatorial polar coast",
        "earth_radius_km": EARTH_RADIUS,
        "mu_km3_s2": MU,
        "initial_perigee_altitude_km": PERIGEE_ALTITUDE,
        "initial_apogee_altitude_km": APOGEE_ALTITUDE,
        "specific_angular_momentum_km2_s": h,
        "period_s": period,
        "evaluated_duration_s": duration,
        "base_timestep_s": DT,
        "base_steps": base_steps,
        "semi_implicit_euler": semi,
        "midpoint_rk2": midpoint,
        "rk4_dt_over_32": rk4_32,
        "rk4_dt_over_64": rk4_64,
        "refined_rk4_convergence": convergence,
        "selection": {
            "production": "semi-implicit Euler",
            "reason": "Both fixed-point candidates produce identical raw one-orbit states at the accepted resolution; semi-implicit Euler uses one declared force evaluation and is no slower in the linked C64 measurement.",
            "hero_mission_confirmation_required": True,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    path = root / "phase2" / "integrator-v1.json"
    data = (json.dumps(payload(), indent=2) + "\n").encode()
    digest_path = path.with_name(path.name + ".sha256")
    digest = (hashlib.sha256(data).hexdigest() + "\n").encode()
    if args.check:
        stale = not path.exists() or path.read_bytes() != data or not digest_path.exists() or digest_path.read_bytes() != digest
        if stale:
            print("Phase 2 integrator evidence is stale", file=sys.stderr)
        return 1 if stale else 0
    path.write_bytes(data)
    digest_path.write_bytes(digest)
    print(path.relative_to(root))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
