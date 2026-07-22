#!/usr/bin/env python3
"""Measure Phase 0 arithmetic primitives through the VICE binary monitor."""

from __future__ import annotations

import argparse
import json
import struct
from dataclasses import asdict, dataclass
from pathlib import Path

from vice_timing import run_prg_until_result

RESULT_MAGIC = 0x5250534B
RESULT_START = 0xC100
RESULT_END = 0xC127


@dataclass(frozen=True)
class PrimitiveTimingResult:
    candidate: str
    iterations: int
    boundary_overhead_cycles: int
    multiply_cycles: int
    divide_cycles: int
    fraction_cycles: int
    multiply_cycles_per_call: float
    divide_cycles_per_call: float
    fraction_cycles_per_call: float
    multiply_accumulator: int
    divide_accumulator: int
    fraction_accumulator: int


def parse_result(
    memory: bytes, expected_candidate: int, name: str
) -> PrimitiveTimingResult | None:
    if struct.unpack_from("<I", memory, 0)[0] != RESULT_MAGIC:
        return None
    schema, candidate, status, iterations = struct.unpack_from("<HHHH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"Unsupported primitive timing schema {schema}")
    if candidate != expected_candidate:
        raise RuntimeError(f"Expected candidate {expected_candidate}, received {candidate}")
    if status != 0:
        raise RuntimeError(f"{name} reported primitive status {status}")
    overhead, multiply, divide, fraction = struct.unpack_from("<IIII", memory, 12)
    multiply_accumulator, divide_accumulator, fraction_accumulator = struct.unpack_from(
        "<III", memory, 28
    )
    multiply_net = (multiply - overhead) & 0xFFFFFFFF
    divide_net = (divide - overhead) & 0xFFFFFFFF
    fraction_net = (fraction - overhead) & 0xFFFFFFFF
    return PrimitiveTimingResult(
        candidate=name,
        iterations=iterations,
        boundary_overhead_cycles=overhead,
        multiply_cycles=multiply_net,
        divide_cycles=divide_net,
        fraction_cycles=fraction_net,
        multiply_cycles_per_call=multiply_net / iterations,
        divide_cycles_per_call=divide_net / iterations,
        fraction_cycles_per_call=fraction_net / iterations,
        multiply_accumulator=multiply_accumulator,
        divide_accumulator=divide_accumulator,
        fraction_accumulator=fraction_accumulator,
    )


def run_once(
    vice: Path,
    prg: Path,
    candidate_id: int,
    candidate_name: str,
    timeout: float,
) -> PrimitiveTimingResult:
    return run_prg_until_result(
        vice,
        prg,
        timeout,
        RESULT_START,
        RESULT_END,
        lambda memory: parse_result(memory, candidate_id, candidate_name),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--candidate-id", type=int, choices=(1, 2), required=True)
    parser.add_argument("--candidate-name", required=True)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    results = [
        run_once(vice, prg, args.candidate_id, args.candidate_name, args.timeout)
        for _ in range(args.runs)
    ]
    signatures = {
        (result.multiply_cycles, result.divide_cycles, result.fraction_cycles)
        for result in results
    }
    payload = {
        "vice": str(vice),
        "prg": str(prg),
        "runs": [asdict(result) for result in results],
        "stable": len(signatures) == 1,
    }
    print(json.dumps(payload, indent=2))
    return 0 if payload["stable"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
