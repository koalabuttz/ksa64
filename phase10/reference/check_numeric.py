#!/usr/bin/env python3
"""Independent Phase 10 range/time contract checker."""

from __future__ import annotations

import argparse
import csv
import json
from datetime import date
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "phase10" / "numeric-contract.json"
LEAPS = ROOT / "phase10" / "source-data" / "leap-seconds-v1.csv"
EOP = ROOT / "phase10" / "source-data" / "eop-accepted-epoch-v1.csv"


def load_rows(path: Path) -> list[dict[str, str]]:
    lines = [line for line in path.read_text(encoding="utf-8").splitlines()
             if line and not line.startswith("#")]
    return list(csv.DictReader(lines))


def unix_day(iso: str) -> int:
    value = date.fromisoformat(iso[:10])
    return (value - date(1970, 1, 1)).days


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    leaps = load_rows(LEAPS)
    eop = load_rows(EOP)

    assert contract["mission_time"]["maximum_raw"] == 14_400 * (1 << 16)
    assert contract["steps"] == {
        "powered_q16": (1 << 16) // 128,
        "coast_q16": (1 << 16) // 32,
        "rcs_quantum_q16": (1 << 16) // 256,
        "avionics_period_q16": (1 << 16) // 32,
    }
    assert contract["velocity"]["maximum_raw"] == 12 * (1 << 24)
    assert len(leaps) == 27
    assert [int(row["tai_minus_utc_after"]) for row in leaps] == list(range(11, 38))
    assert unix_day(leaps[-1]["effective_utc"]) == 17_167
    assert len(eop) == 3
    assert eop[1]["utc"] == "2024-01-01T00:00:00Z"
    assert eop[1]["ut1_minus_utc_s"] == "0.0087572"

    report = {
        "schema": "ksa64.phase10.numeric-check-v1",
        "status": "pass",
        "leap_transition_count": len(leaps),
        "last_tai_minus_utc": int(leaps[-1]["tai_minus_utc_after"]),
        "accepted_epoch_unix_day": unix_day("2024-01-01"),
        "accepted_dut1_s": eop[1]["ut1_minus_utc_s"],
        "four_hour_q16": contract["mission_time"]["maximum_raw"],
    }
    if not args.check:
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
