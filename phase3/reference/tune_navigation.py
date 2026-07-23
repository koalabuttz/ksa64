#!/usr/bin/env python3
"""Deterministic bounded shift-gain search for the Phase 3 aided navigator."""
from __future__ import annotations
import json
from itertools import product
from pathlib import Path

DT = 0.125
STEPS = 3172  # T+396.5 s cutoff
SEEDS = range(1, 17)


def noise(seed: int, step: int, scale: float) -> float:
    x = (seed * 0x9E3779B1 + step * 0x85EBCA6B) & 0xFFFFFFFF
    x ^= x >> 16
    a = (x & 0xFFFF) / 65535.0
    b = ((x >> 16) & 0xFFFF) / 65535.0
    return (a - b) * scale


def run(gains: tuple[int, int, int, int], seed: int) -> tuple[float, float]:
    alt_shift, beta_shift, gps_pos_shift, gps_vel_shift = gains
    x = 0.0
    v = 0.0
    truth_x = 0.0
    truth_v = 0.0
    truth_history = [(truth_x, truth_v)]
    for step in range(1, STEPS + 1):
        t = step * DT
        accel = 0.018 if t < 155.0 else (0.010 if t < 396.5 else 0.0)
        truth_v += accel * DT
        truth_x += truth_v * DT
        truth_history.append((truth_x, truth_v))
        v += accel * DT
        x += v * DT
        if step % 2 == 1 and t <= 80.0 and not (45.0 <= t < 60.0):
            measured = truth_x + 0.020 + noise(seed, step, 0.010)
            error = measured - x
            x += error / (1 << alt_shift)
            v += (error / 0.25) / (1 << (beta_shift + 2))
        if step >= 962 and step % 8 == 2 and not (260.0 <= t < 320.0):
            delayed_x, delayed_v = truth_history[step - 2]
            measured_x = delayed_x + noise(seed, step, 0.020)
            measured_v = delayed_v + noise(seed + 31, step, 0.0002)
            x += (measured_x - x) / (1 << gps_pos_shift)
            v += (measured_v - v) / (1 << gps_vel_shift)
    return abs(x - truth_x), abs(v - truth_v)


def main() -> None:
    candidates = []
    for gains in product(range(1, 4), range(1, 5), range(1, 4), range(1, 6)):
        errors = [run(gains, seed) for seed in SEEDS]
        score = (max(p for p, _ in errors), max(v for _, v in errors), gains)
        candidates.append(score)
    candidates.sort()
    best = candidates[0]
    payload = {
        "schema": "ksa64.phase3.navigation-gains-v1",
        "objective": "lexicographic worst cutoff position error then velocity error",
        "cases": len(SEEDS),
        "candidate_count": len(candidates),
        "selected": {
            "alt_alpha_shift": best[2][0],
            "alt_beta_shift": best[2][1],
            "gps_position_shift": best[2][2],
            "gps_velocity_shift": best[2][3],
        },
        "worst_cutoff_position_error_km": best[0],
        "worst_cutoff_velocity_error_km_s": best[1],
        "runner_up": {
            "shifts": candidates[1][2],
            "position_error_km": candidates[1][0],
            "velocity_error_km_s": candidates[1][1],
        },
    }
    out = Path(__file__).resolve().parents[1] / "navigation-gains-v1.json"
    out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(payload, indent=2))

if __name__ == "__main__":
    main()
