#!/usr/bin/env python3
"""Independent floating-point reference and pitch-search helper for KSA-2A."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
R_EARTH = 6378.137
MU = 398600.4418
OMEGA = 0.00007292115


def interpolate(x: float, xs: list[float], ys: list[float]) -> float:
    if x <= xs[0]:
        return ys[0]
    if x >= xs[-1]:
        return ys[-1]
    for left in range(len(xs) - 1):
        if x < xs[left + 1]:
            f = (x - xs[left]) / (xs[left + 1] - xs[left])
            return ys[left] + f * (ys[left + 1] - ys[left])
    raise AssertionError("unreachable interpolation interval")


def load() -> tuple[dict[str, Any], dict[str, Any]]:
    scenario = json.loads((ROOT / "phase2/examples/ksa2a-200km.json").read_text())
    environment = json.loads((ROOT / "phase2/environment-v1.json").read_text())
    return scenario, environment


def orbit(r: float, vr: float, h: float) -> dict[str, float | str]:
    vt = h / r
    energy = 0.5 * (vr * vr + vt * vt) - MU / r
    e_cos = vt * h / MU - 1.0
    e_sin = vr * h / MU
    eccentricity = math.hypot(e_cos, e_sin)
    if energy >= 0:
        return {"class": "escape", "energy": energy, "eccentricity": eccentricity}
    semi_major = -MU / (2.0 * energy)
    perigee = semi_major * (1.0 - eccentricity) - R_EARTH
    apogee = semi_major * (1.0 + eccentricity) - R_EARTH
    classification = "stable" if perigee >= 120.0 else "suborbital" if perigee > 0 else "impact"
    return {
        "class": classification,
        "energy": energy,
        "eccentricity": eccentricity,
        "perigee_km": perigee,
        "apogee_km": apogee,
    }


def simulate(pitches: list[float] | None = None, stage2_scale: float = 1.0) -> dict[str, Any]:
    scenario, environment = load()
    dt = float(scenario["timestep_s"])
    pitch_times = [float(knot["time_s"]) for knot in scenario["pitch_program"]]
    default_pitches = [float(knot["degrees_from_vertical"]) for knot in scenario["pitch_program"]]
    pitch_values = pitches or default_pitches
    if len(pitch_values) != len(pitch_times):
        raise ValueError("pitch vector length")
    altitudes = [float(value) for value in environment["altitude_km"]]
    densities = [float(value) for value in environment["density_kg_m3"]]
    sounds = [float(value) for value in environment["sound_speed_km_s"]]
    stages = scenario["stages"]
    dry = [float(stage["dry_mass_t"]) for stage in stages]
    prop = [float(stage["propellant_mass_t"]) for stage in stages]
    thrust = [float(stage["thrust_mn"]) for stage in stages]
    flow = [float(stage["mass_flow_t_s"]) for stage in stages]
    burn = [float(stage["max_burn_s"]) for stage in stages]
    burn[1] *= stage2_scale
    separation = [float(stage["separation_delay_s"]) for stage in stages]
    ignition = [float(stage["ignition_delay_s"]) for stage in stages]
    area = [float(stage["reference_area_m2"]) for stage in stages]
    aero_names = [stage["aero_table"] for stage in stages]
    payload = float(scenario["payload_mass_t"])

    r = R_EARTH + float(scenario["initial"]["altitude_km"])
    vr = float(scenario["initial"]["radial_velocity_km_s"])
    vt = OMEGA * r + float(scenario["initial"]["surface_relative_velocity_km_s"])
    h = r * vt
    mass = payload + sum(dry) + sum(prop)
    active_prop = prop[0]
    stage = 0
    phase = "ignition" if ignition[0] else "burning"
    phase_elapsed = 0.0
    time = 0.0
    max_q = 0.0
    max_accel = 0.0
    events: list[dict[str, float | str | int]] = []
    cutoff_state: dict[str, float] | None = None

    steps = round(float(scenario["duration_s"]) / dt)
    for _ in range(steps):
        pitch_deg = interpolate(time, pitch_times, pitch_values)
        pitch = math.radians(pitch_deg)
        density = interpolate(r - R_EARTH, altitudes, densities)
        sound = interpolate(r - R_EARTH, altitudes, sounds)
        vt = h / r
        air_r = vr
        air_t = vt - OMEGA * r
        air_speed = math.hypot(air_r, air_t)
        mach = air_speed / sound
        table = scenario["aerodynamics"][aero_names[min(stage, len(stages) - 1)]]
        cd = interpolate(mach, [float(k["mach"]) for k in table], [float(k["cd"]) for k in table])
        q = 0.5 * density * (air_speed * 1000.0) ** 2 / 1000.0
        drag_mn = q * area[min(stage, len(stages) - 1)] * cd / 1000.0
        if air_speed:
            drag_r = -drag_mn * air_r / air_speed
            drag_t = -drag_mn * air_t / air_speed
        else:
            drag_r = drag_t = 0.0
        burning = stage < len(stages) and phase == "burning"
        applied_thrust = thrust[stage] if burning else 0.0
        force_r = applied_thrust * math.cos(pitch) + drag_r
        force_t = applied_thrust * math.sin(pitch) + drag_t
        accel_r_proper = force_r / mass
        accel_t = force_t / mass
        accel_r = h * h / (r * r * r) - MU / (r * r) + accel_r_proper
        vr += accel_r * dt
        r += vr * dt
        h += r * accel_t * dt
        proper = math.hypot(accel_r_proper, accel_t) * 1000.0
        max_q = max(max_q, q)
        max_accel = max(max_accel, proper)

        if burning:
            consumed = min(active_prop, flow[stage] * dt)
            active_prop -= consumed
            mass -= consumed
        time += dt
        phase_elapsed += dt

        if stage < len(stages):
            if phase == "ignition" and phase_elapsed + 1e-12 >= ignition[stage]:
                phase = "burning"
                phase_elapsed = 0.0
                events.append({"time_s": time, "event": "ignition", "stage": stage + 1})
            elif phase == "burning" and (phase_elapsed + 1e-12 >= burn[stage] or active_prop <= 1e-12):
                phase = "separation" if stages[stage]["separate"] else "complete"
                phase_elapsed = 0.0
                events.append({"time_s": time, "event": "cutoff", "stage": stage + 1})
                if stage == len(stages) - 1:
                    cutoff_state = {"time_s": time, "radius_km": r, "radial_velocity_km_s": vr, "specific_angular_momentum_km2_s": h}
            elif phase == "separation" and phase_elapsed + 1e-12 >= separation[stage]:
                mass -= dry[stage] + active_prop
                events.append({"time_s": time, "event": "separation", "stage": stage + 1})
                stage += 1
                active_prop = prop[stage]
                phase = "ignition" if ignition[stage] else "burning"
                phase_elapsed = 0.0
                if phase == "burning":
                    events.append({"time_s": time, "event": "ignition", "stage": stage + 1})

    return {
        "pitch_degrees": pitch_values,
        "failure_stage2_scale": stage2_scale,
        "final": {"time_s": time, "radius_km": r, "altitude_km": r - R_EARTH, "radial_velocity_km_s": vr, "specific_angular_momentum_km2_s": h, "mass_t": mass},
        "orbit": orbit(r, vr, h),
        "cutoff": cutoff_state,
        "cutoff_orbit": orbit(cutoff_state["radius_km"], cutoff_state["radial_velocity_km_s"], cutoff_state["specific_angular_momentum_km2_s"]) if cutoff_state else None,
        "max_q_kpa": max_q,
        "max_proper_acceleration_m_s2": max_accel,
        "events": events,
    }


def objective(values: list[float]) -> float:
    pitches = [0.0, 0.0, *values, 90.0]
    if any(right < left for left, right in zip(pitches, pitches[1:])):
        return 1e6 + sum(max(0.0, left - right) ** 2 for left, right in zip(pitches, pitches[1:]))
    result = simulate(pitches)
    orbit_result = result["cutoff_orbit"]
    if not orbit_result or "perigee_km" not in orbit_result:
        return 1e6
    perigee = float(orbit_result["perigee_km"])
    apogee = float(orbit_result["apogee_km"])
    eccentricity = float(orbit_result["eccentricity"])
    limits = max(0.0, float(result["max_q_kpa"]) - 60.0) ** 2 + max(0.0, float(result["max_proper_acceleration_m_s2"]) - 60.0) ** 2
    return (perigee - 200.0) ** 2 + (apogee - 200.0) ** 2 + 1e5 * eccentricity * eccentricity + 100.0 * limits


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--optimize", action="store_true")
    parser.add_argument("--failure", action="store_true")
    args = parser.parse_args()
    if args.optimize:
        from scipy.optimize import differential_evolution, minimize

        bounds = [(0, 40), (5, 70), (20, 88), (50, 90), (75, 90)]
        rough = differential_evolution(objective, bounds, seed=64, popsize=12, maxiter=60, polish=False, workers=1)
        answer = minimize(objective, rough.x, method="Nelder-Mead", options={"maxiter": 1000, "xatol": 1e-7, "fatol": 1e-6})
        result = simulate([0.0, 0.0, *answer.x.tolist(), 90.0])
    else:
        result = simulate(stage2_scale=0.95 if args.failure else 1.0)
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
