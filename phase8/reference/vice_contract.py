#!/usr/bin/env python3
"""Run the finite Phase 8 numeric-contract probe under VICE."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(PROJECT_ROOT / "phase0" / "reference"))
from vice_timing import run_prg_until_result  # noqa: E402

MAGIC = 0x38504B53
EXPECTED_SIGNATURE = 0x74557844


def parse_result(memory: bytes) -> dict[str, int] | None:
    magic, failures, signature = struct.unpack_from("<III", memory)
    if magic != MAGIC:
        return None
    if failures:
        raise RuntimeError(f"Phase 8 target probe reported {failures} failures")
    if signature != EXPECTED_SIGNATURE:
        raise RuntimeError(
            f"Phase 8 target signature 0x{signature:08x}, expected 0x{EXPECTED_SIGNATURE:08x}"
        )
    return {"failures": failures, "signature": signature}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=90.0)
    args = parser.parse_args()
    result = run_prg_until_result(
        args.vice.resolve(strict=True),
        args.prg.resolve(strict=True),
        args.timeout,
        0xC000,
        0xC00B,
        parse_result,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
