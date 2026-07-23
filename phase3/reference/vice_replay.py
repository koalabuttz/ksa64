#!/usr/bin/env python3
"""Run the validated Phase 3 KRP3 replay under VICE."""
from __future__ import annotations
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import run_prg_until_result  # noqa: E402


def ascii_code(code: int) -> str:
    if 1 <= code <= 26:
        return chr(ord("A") + code - 1)
    if 32 <= code <= 63:
        return chr(code)
    return "?"


def parse(memory: bytes) -> dict | None:
    rows = ["".join(ascii_code(c) for c in memory[i:i+40]).rstrip() for i in range(0, 1000, 40)]
    if rows[24].startswith("PHASE 3 REPLAY ERROR"):
        raise RuntimeError(rows[24])
    if rows[24] != "PHASE 3 REPLAY PASS":
        return None
    expected = {
        0: "KSA64 PHASE 3 REPLAY",
        2: "FRAMES 0906  STEP 7200",
        3: "ALT Q12 000BA23B  DOWN 1E105B2E",
        4: "MODE 0005 STAGE 0001 PITCH 4000",
        5: "EVENTS 0077  ALARMS 0000",
        6: "SID I02 C02 S01 E01 A00",
        7: "SOURCE CRC AF79B36E",
        8: "CONFIG CRC 2815EA66",
        24: "PHASE 3 REPLAY PASS",
    }
    for row, value in expected.items():
        if rows[row] != value:
            raise RuntimeError(f"row {row}: expected {value!r}, got {rows[row]!r}")
    if not rows[9].startswith("CUE HASH "):
        raise RuntimeError(f"unexpected cue hash row: {rows[9]!r}")
    return {"rows": {str(row): rows[row] for row in expected}, "cue_hash": rows[9], "passed": True}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()
    result = run_prg_until_result(args.vice.resolve(strict=True), args.prg.resolve(strict=True), args.timeout, 0x0400, 0x07E7, parse)
    print(json.dumps(result, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())