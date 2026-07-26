#!/usr/bin/env python3
"""Independent float64 Phase 11 prediction vector generator/checker."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIXTURE = ROOT / "prediction_vectors.json"


def acceleration(position: list[float]) -> list[float]:
    semi_major_km = 6378.137
    mu_km3_s2 = 398600.44140625
    j2 = 1162465 / (1 << 30)
    radius = math.sqrt(sum(value * value for value in position))
    z_squared = (position[2] / radius) ** 2
    factor = 1.5 * j2 * (semi_major_km / radius) ** 2
    correction_xy = 1.0 + factor - 5.0 * factor * z_squared
    correction_z = 1.0 + 3.0 * factor - 5.0 * factor * z_squared
    common = -mu_km3_s2 / radius**3
    return [
        common * position[0] * correction_xy,
        common * position[1] * correction_xy,
        common * position[2] * correction_z,
    ]


def generate() -> dict[str, object]:
    semi_major_km = 6378.137
    position = [semi_major_km + 100.0, 0.0, 0.0]
    velocity = [100000 / (1 << 24), 120000000 / (1 << 24), 0.0]
    selected: list[dict[str, object]] = []
    for second in range(16):
        if second in (0, 1, 5, 15):
            selected.append(
                {
                    "second": second,
                    "position_km": position.copy(),
                    "velocity_km_s": velocity.copy(),
                    "altitude_km": math.sqrt(sum(value * value for value in position))
                    - semi_major_km,
                }
            )
        start_acceleration = acceleration(position)
        midpoint_position = [
            position[axis] + 0.5 * velocity[axis] for axis in range(3)
        ]
        midpoint_velocity = [
            velocity[axis] + 0.5 * start_acceleration[axis] for axis in range(3)
        ]
        midpoint_acceleration = acceleration(midpoint_position)
        position = [
            position[axis] + midpoint_velocity[axis] for axis in range(3)
        ]
        velocity = [
            velocity[axis] + midpoint_acceleration[axis] for axis in range(3)
        ]
    return {
        "schema": "ksa64.phase11.prediction-vector-v1",
        "reference": "independent Python float64 central-plus-J2 midpoint RK2",
        "cadence_seconds": 1.0,
        "points": selected,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generated = generate()
    encoded = json.dumps(generated, indent=2, sort_keys=True) + "\n"
    if args.check:
        if not FIXTURE.exists() or FIXTURE.read_text(encoding="utf-8") != encoded:
            raise SystemExit("prediction_vectors.json is stale")
    else:
        FIXTURE.write_text(encoded, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
