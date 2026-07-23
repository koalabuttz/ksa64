#!/usr/bin/env python3
"""Compare a GMAT KSA-2A report with the committed aligned-model evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def numeric_rows(path: Path) -> list[list[float]]:
    rows: list[list[float]] = []
    for line in path.read_text().splitlines():
        fields = line.split()
        if len(fields) < 5:
            continue
        try:
            rows.append([float(value) for value in fields[:5]])
        except ValueError:
            continue
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    expected = json.loads((root / "expected.json").read_text())
    rows = numeric_rows(args.report)
    if len(rows) < 2:
        raise SystemExit("report does not contain two numeric rows")
    initial, final = rows[0], rows[-1]
    analytic = expected["analytic_result"]
    radius = expected["earth_radius_km_for_altitude_comparison"]
    acceptance = expected["acceptance"]
    checks = {
        "one_period_radius_km": (abs(final[1] - initial[1]), acceptance["radius_difference_km"]),
        "perigee_radius_km": (abs(final[2] - (radius + analytic["perigee_altitude_km"])), acceptance["perigee_difference_km"]),
        "apogee_radius_km": (abs(final[3] - (radius + analytic["apogee_altitude_km"])), acceptance["apogee_difference_km"]),
        "eccentricity": (abs(final[4] - analytic["eccentricity"]), acceptance["eccentricity_difference"]),
    }
    failed = False
    for name, (error, tolerance) in checks.items():
        passed = error <= tolerance
        failed |= not passed
        print(f"{name}: error={error:.12g} tolerance={tolerance:.12g} {'PASS' if passed else 'FAIL'}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
