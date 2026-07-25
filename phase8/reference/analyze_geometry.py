#!/usr/bin/env python3
"""Independently reconstruct Phase 8 geometry-derived aerodynamic evidence."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "phase8/source-data/firestorm54-spatial.json"
COMPILE_REPORT = ROOT / "phase8/examples/compile-report.json"
OUTPUT = ROOT / "phase8/aero-analysis-v1.json"


def number(field: dict) -> float:
    return float(field["value"])


def derive(source: dict, dry_cg: float) -> dict[str, float]:
    length = number(source["length_m"])
    diameter = number(source["diameter_m"])
    nose_length = number(source["nose"]["length_m"])
    factors = {"conical": 2.0 / 3.0, "tangent_ogive": 0.466, "elliptical": 1.0 / 3.0}
    slope_total = 2.0
    weighted_cp = 2.0 * factors[source["nose"]["shape"]] * nose_length
    reference_area = math.pi * diameter**2 / 4.0
    roll_damping = 0.0

    for transition in source["transitions"]:
        fore = number(transition["fore_diameter_m"])
        aft = number(transition["aft_diameter_m"])
        transition_length = number(transition["length_m"])
        station = number(transition["fore_station_m"])
        slope = 2.0 * (aft**2 - fore**2) / diameter**2
        ratio = fore / aft
        denominator = 1.0 - ratio**2
        fraction = 0.5 if abs(denominator) < 1e-12 else (1.0 + (1.0 - ratio) / denominator) / 3.0
        slope_total += slope
        weighted_cp += slope * (station + transition_length * fraction)

    for fins in source["fin_sets"]:
        count = float(fins["count"])
        root = number(fins["root_chord_m"])
        tip = number(fins["tip_chord_m"])
        span = number(fins["span_m"])
        sweep = number(fins["leading_edge_sweep_m"])
        station = number(fins["leading_edge_from_nose_m"])
        mid_offset = sweep + 0.5 * (tip - root)
        mid_length = math.hypot(span, mid_offset)
        isolated = 4.0 * count * (span / diameter) ** 2 / (
            1.0 + math.sqrt(1.0 + (2.0 * mid_length / (root + tip)) ** 2)
        )
        interference = 1.0 + diameter / (2.0 * span + diameter)
        slope = isolated * interference
        cp = station + sweep * (root + 2.0 * tip) / (3.0 * (root + tip)) + (
            root + tip - root * tip / (root + tip)
        ) / 6.0
        slope_total += slope
        weighted_cp += slope * cp
        fin_area = 0.5 * (root + tip) * span
        roll_damping += 0.5 * count * (fin_area / reference_area) * (span / length) ** 2

    cp = weighted_cp / slope_total
    static_margin = (cp - dry_cg) / diameter
    pitch_yaw_damping = slope_total * abs((cp - dry_cg) / length) * diameter / length
    return {
        "derived_cp_from_nose_m": cp,
        "normal_force_slope_per_rad": slope_total,
        "pitch_yaw_damping": pitch_yaw_damping,
        "roll_damping": roll_damping,
        "dry_static_margin_calibers": static_margin,
    }


def verify_close(name: str, actual: float, expected: float, tolerance: float = 1e-12) -> None:
    if not math.isclose(actual, expected, rel_tol=tolerance, abs_tol=tolerance):
        raise SystemExit(f"{name}: independent={actual:.17g}, compiler={expected:.17g}")


def main() -> None:
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    compiler = json.loads(COMPILE_REPORT.read_text(encoding="utf-8"))
    derived = derive(source, compiler["dry_cg_from_nose_m"])
    for key, value in derived.items():
        verify_close(key, value, compiler[key])

    diameter = number(source["diameter_m"])
    qualified_delta_calibers = abs(derived["derived_cp_from_nose_m"] - compiler["qualified_cp_reference_m"]) / diameter
    fixtures = {
        "conical_nose_cp_fraction": 2.0 / 3.0,
        "tangent_ogive_cp_fraction": 0.466,
        "elliptical_nose_cp_fraction": 1.0 / 3.0,
        "body_tube_normal_force_slope": 0.0,
        "pitch_yaw_symmetry_error": 0.0,
        "qualified_cp_delta_calibers": qualified_delta_calibers,
        "mach_envelope": 0.8,
        "angle_of_attack_envelope_deg": 15.0,
    }
    if qualified_delta_calibers > 0.5:
        raise SystemExit("derived CP exceeds qualified manufacturer-reference gate")
    report = {
        "schema": "ksa64.phase8-aero-analysis-v1",
        "source": str(SOURCE.relative_to(ROOT)).replace("\\", "/"),
        "compiler_report": str(COMPILE_REPORT.relative_to(ROOT)).replace("\\", "/"),
        "derived": derived,
        "fixtures": fixtures,
        "method": "independent Python reconstruction of documented Barrowman-compatible equations",
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if "--check" in sys.argv:
        if OUTPUT.read_text(encoding="utf-8") != rendered:
            raise SystemExit(f"stale generated file: {OUTPUT}")
    else:
        OUTPUT.write_text(rendered, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
