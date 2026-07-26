#!/usr/bin/env python3
"""Freeze and verify the accepted Phase 10 nominal artifact manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / "phase10" / "evidence"
OUT = EVIDENCE / "artifact-manifest-v1.json"
NAMES = (
    "ksa-g10r-nominal.csv",
    "ksa-g10r-nominal.html",
    "ksa-g10r-nominal.kmr10.json",
    "ksa-g10r-nominal.kph10",
    "ksa-g10r-nominal.ksr10",
    "ksa-g10r-nominal.ktt10",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def render() -> bytes:
    recording = json.loads((EVIDENCE / "ksa-g10r-nominal.kmr10.json").read_text())
    report = {
        "schema": "ksa64.phase10.artifact-manifest-v1",
        "mission": "KSA-G10R nominal",
        "case_seed": recording["case_seed"],
        "evaluation_identity": recording["evaluation_identity"],
        "files": {name: sha256(EVIDENCE / name) for name in NAMES},
        "metrics": {
            "duration_s": recording["points"][-1]["time_s"],
            "releases": recording["releases"],
            "apogee_km": recording["apogee_km"],
            "downrange_km": recording["downrange_km"],
            "crossrange_km": recording["crossrange_km"],
            "frame_transitions": recording["transition_count"],
        },
    }
    return (json.dumps(report, indent=2) + "\n").encode()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = render()
    if args.check:
        if not OUT.exists() or OUT.read_bytes() != content:
            print("phase10 nominal artifact manifest: stale")
            return 1
        print("phase10 nominal artifact manifest: PASS")
        return 0
    OUT.write_bytes(content)
    print(content.decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
