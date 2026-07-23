#!/usr/bin/env python3
"""Search KSA-5A local-pitch and upper-stage cutoff candidates."""
from __future__ import annotations
import copy
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("phase2_reference", ROOT / "phase2/reference/mission_reference.py")
REFERENCE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(REFERENCE)
ORIGINAL_LOAD = REFERENCE.load


def load_ksa5a():
    scenario, environment = ORIGINAL_LOAD()
    scenario = copy.deepcopy(scenario)
    scenario["payload_mass_t"] = "12"
    return scenario, environment


REFERENCE.load = load_ksa5a


def result_for(values):
    pitches = [0.0, 0.0, *values[:5], 90.0]
    return REFERENCE.simulate(pitches, stage2_scale=values[5])


def objective(values):
    pitches = [0.0, 0.0, *values[:5], 90.0]
    if any(right < left for left, right in zip(pitches, pitches[1:])):
        return 1e9
    result = result_for(values)
    orbit = result["cutoff_orbit"]
    if not orbit or "perigee_km" not in orbit:
        return 1e9
    perigee = orbit["perigee_km"]
    apogee = orbit["apogee_km"]
    limits = max(0.0, result["max_q_kpa"] - 60.0) ** 2 + max(0.0, result["max_proper_acceleration_m_s2"] - 60.0) ** 2
    return (perigee - 200.0) ** 2 + (apogee - 200.0) ** 2 + 100.0 * limits


def main():
    from scipy.optimize import differential_evolution, minimize
    bounds = [(0, 40), (5, 70), (20, 88), (50, 90), (75, 90), (0.90, 1.0)]
    rough = differential_evolution(objective, bounds, seed=0x5A, popsize=8, maxiter=24, polish=False, workers=1)
    answer = minimize(objective, rough.x, method="Nelder-Mead", options={"maxiter": 1200, "xatol": 1e-9, "fatol": 1e-8})
    result = result_for(answer.x)
    print(json.dumps({"parameters": answer.x.tolist(), "objective": answer.fun, "result": result}, indent=2))


if __name__ == "__main__":
    main()