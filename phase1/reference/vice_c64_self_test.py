#!/usr/bin/env python3
"""Run the Phase 1 C64 self-test PRG and verify its hardware-visible result."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(PROJECT_ROOT / "phase0" / "reference"))

from vice_timing import run_prg_until_result  # noqa: E402


def parse_result(memory: bytes) -> dict[str, int] | None:
    raw_border, raw_background = memory
    border = raw_border & 0x0F
    background = raw_background & 0x0F
    if border == 2 and background == 2:
        raise RuntimeError("C64 self-test reported failure")
    if border == 5 and background == 0:
        return {"border_color": border, "background_color": background, "failures": 0}
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    arguments = parser.parse_args()
    result = run_prg_until_result(
        arguments.vice.resolve(strict=True),
        arguments.prg.resolve(strict=True),
        arguments.timeout,
        0xD020,
        0xD021,
        parse_result,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
