#!/usr/bin/env python3
"""Measure both Phase 2 vacuum integrator candidates under PAL VICE."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import run_prg_until_result  # noqa: E402

MAGIC = 0x3256534B
STEPS = 256


def parse(memory: bytes):
    if struct.unpack_from("<I", memory)[0] != MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", memory, 4)
    if schema != 1 or status != 0:
        raise RuntimeError((schema, status))
    semi, midpoint, overhead = struct.unpack_from("<III", memory, 8)
    semi_r, semi_vr, mid_r, mid_vr = struct.unpack_from("<iiii", memory, 20)
    return {
        "semi_implicit_net_cycles": semi,
        "semi_implicit_cycles_per_step": semi / STEPS,
        "midpoint_net_cycles": midpoint,
        "midpoint_cycles_per_step": midpoint / STEPS,
        "midpoint_cost_ratio": midpoint / semi,
        "boundary_overhead": overhead,
        "semi_final": [semi_r, semi_vr],
        "midpoint_final": [mid_r, mid_vr],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    results = [run_prg_until_result(args.vice.resolve(), args.prg.resolve(), 300, 0xC000, 0xC023, parse) for _ in range(args.runs)]
    stable = all(item == results[0] for item in results[1:])
    payload = json.dumps({"runs": results, "stable": stable}, indent=2) + "\n"
    if args.output:
        args.output.write_text(payload, encoding="utf-8")
    else:
        print(payload, end="")
    return 0 if stable else 2


if __name__ == "__main__":
    raise SystemExit(main())
