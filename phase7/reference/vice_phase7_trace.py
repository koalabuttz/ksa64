#!/usr/bin/env python3
"""Compare the first 129 Phase 7 states between native Rust and rust-mos."""
from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import run_prg_until_result

MAGIC = 0x37524B53
START = 0xC000
TRACE_COUNT = 129
LAST_BASE = 12 + TRACE_COUNT * 4
END = START + LAST_BASE + 55


def parse(memory: bytes) -> dict[str, object] | None:
    if struct.unpack_from("<I", memory, 0)[0] != MAGIC:
        return None
    schema, status, count, reserved = struct.unpack_from("<HHHH", memory, 4)
    if schema != 1 or status != 0 or reserved != 0 or count != TRACE_COUNT:
        raise RuntimeError(
            f"invalid trace header schema={schema} status={status} count={count} reserved={reserved}"
        )
    checksums = list(struct.unpack_from(f"<{TRACE_COUNT}I", memory, 12))
    fields = struct.unpack_from("<12iII", memory, LAST_BASE)
    names = (
        "step",
        "time",
        "altitude",
        "velocity",
        "acceleration",
        "mass",
        "propellant",
        "impulse",
        "phase",
        "thrust",
        "dynamic_pressure",
        "mach",
        "events",
        "checksum",
    )
    return {"checksums": checksums, "last": dict(zip(names, fields, strict=True))}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    host = json.loads(args.host.read_text())
    target = run_prg_until_result(
        args.vice.resolve(strict=True),
        args.prg.resolve(strict=True),
        300.0,
        START,
        END,
        parse,
    )
    divergence = next(
        (
            index
            for index, (host_value, target_value) in enumerate(
                zip(host["checksums"], target["checksums"], strict=True)
            )
            if host_value != target_value
        ),
        None,
    )
    data = {
        "schema": "ksa64.phase7.exact-trace-v1",
        "count": TRACE_COUNT,
        "first_divergence": divergence,
        "host_last": host["last"],
        "target_last": target["last"],
        "exact": divergence is None and host["last"] == target["last"],
    }
    text = json.dumps(data, indent=2) + "\n"
    print(text, end="")
    if args.output is not None:
        args.output.write_text(text)
    return 0 if data["exact"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
