#!/usr/bin/env python3
"""Measure the checked Phase 1 production executor under PAL VICE."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(PROJECT_ROOT / "phase0" / "reference"))

from vice_timing import run_prg_until_result  # noqa: E402

TIMING_MAGIC = 0x3154534B
RESULT_START = 0xC000
RESULT_END = 0xC03D
TOTAL_STEPS = 2048


@dataclass(frozen=True)
class ProductionTimingResult:
    dynamics_elapsed_cycles: int
    dynamics_net_cycles: int
    dynamics_cycles_per_step: float
    mission_elapsed_cycles: int
    mission_net_cycles: int
    mission_cycles_per_step: float
    checksum_overhead_cycles: int
    checksum_overhead_per_step: float
    boundary_overhead_cycles: int
    step: int
    time_q16: int
    altitude_q12: int
    velocity_q24: int
    acceleration_q28: int
    mass_q12: int
    propellant_q12: int
    checksum: int
    cutoff_events: int


def parse_result(memory: bytes) -> ProductionTimingResult | None:
    if struct.unpack_from("<I", memory, 0)[0] != TIMING_MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"Unsupported production timing schema {schema}")
    if status != 0:
        raise RuntimeError(f"Production timing runner reported status 0x{status:04x}")

    dynamics_elapsed, dynamics_net, mission_elapsed, mission_net, overhead = (
        struct.unpack_from("<IIIII", memory, 8)
    )
    step = struct.unpack_from("<I", memory, 28)[0]
    time_q16, altitude, velocity, acceleration, mass, propellant = struct.unpack_from(
        "<iiiiii", memory, 32
    )
    checksum = struct.unpack_from("<I", memory, 56)[0]
    cutoff_events = struct.unpack_from("<H", memory, 60)[0]

    if dynamics_net != (dynamics_elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError("Inconsistent dynamics net cycle count")
    if mission_net != (mission_elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError("Inconsistent mission net cycle count")
    checksum_overhead = (mission_net - dynamics_net) & 0xFFFFFFFF
    return ProductionTimingResult(
        dynamics_elapsed_cycles=dynamics_elapsed,
        dynamics_net_cycles=dynamics_net,
        dynamics_cycles_per_step=dynamics_net / TOTAL_STEPS,
        mission_elapsed_cycles=mission_elapsed,
        mission_net_cycles=mission_net,
        mission_cycles_per_step=mission_net / TOTAL_STEPS,
        checksum_overhead_cycles=checksum_overhead,
        checksum_overhead_per_step=checksum_overhead / TOTAL_STEPS,
        boundary_overhead_cycles=overhead,
        step=step,
        time_q16=time_q16,
        altitude_q12=altitude,
        velocity_q24=velocity,
        acceleration_q28=acceleration,
        mass_q12=mass,
        propellant_q12=propellant,
        checksum=checksum,
        cutoff_events=cutoff_events,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=240.0)
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    results = [
        run_prg_until_result(
            vice,
            prg,
            args.timeout,
            RESULT_START,
            RESULT_END,
            parse_result,
        )
        for _ in range(args.runs)
    ]
    stable = len(
        {(result.dynamics_net_cycles, result.mission_net_cycles) for result in results}
    ) == 1
    print(
        json.dumps(
            {
                "vice": str(vice),
                "prg": str(prg),
                "runs": [asdict(result) for result in results],
                "stable": stable,
            },
            indent=2,
        )
    )
    return 0 if stable else 2


if __name__ == "__main__":
    raise SystemExit(main())
