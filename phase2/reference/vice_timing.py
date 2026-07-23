#!/usr/bin/env python3
"""Measure the Phase 2 powered-path fixture under PAL VICE."""

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

TIMING_MAGIC = 0x3250544B
RESULT_START = 0xC000
RESULT_END = 0xC047
STEPS = 8


@dataclass(frozen=True)
class Phase2TimingResult:
    raw_elapsed_cycles: int
    raw_net_cycles: int
    raw_cycles_per_step: float
    recorded_elapsed_cycles: int
    recorded_net_cycles: int
    recorded_cycles_per_step: float
    recorded_overhead_cycles: int
    recorded_overhead_per_step: float
    boundary_overhead_cycles: int
    step: int
    frames_written: int
    checksum: int
    radius_q12: int
    downrange_q32: int
    radial_velocity_q24: int
    angular_momentum_q14: int
    mass_q12: int
    propellant_q12: int
    final_frame_crc32: int
    bytes_written: int


def parse_result(memory: bytes) -> Phase2TimingResult | None:
    if struct.unpack_from("<I", memory, 0)[0] != TIMING_MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"Unsupported Phase 2 timing schema {schema}")
    if status != 0:
        raise RuntimeError(f"Phase 2 timing runner reported status 0x{status:04x}")
    raw_elapsed, raw_net, recorded_elapsed, recorded_net, overhead = struct.unpack_from(
        "<IIIII", memory, 8
    )
    step, frames, checksum = struct.unpack_from("<III", memory, 28)
    radius, downrange, radial_velocity, angular_momentum, mass, propellant = (
        struct.unpack_from("<iiiiii", memory, 40)
    )
    frame_crc, bytes_written = struct.unpack_from("<II", memory, 64)
    if raw_net != (raw_elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError("Raw timing has inconsistent boundary subtraction")
    if recorded_net != (recorded_elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError("Recorded timing has inconsistent boundary subtraction")
    if step != STEPS:
        raise RuntimeError(f"Expected {STEPS} steps, received {step}")
    recorded_overhead = (recorded_net - raw_net) & 0xFFFFFFFF
    return Phase2TimingResult(
        raw_elapsed_cycles=raw_elapsed,
        raw_net_cycles=raw_net,
        raw_cycles_per_step=raw_net / STEPS,
        recorded_elapsed_cycles=recorded_elapsed,
        recorded_net_cycles=recorded_net,
        recorded_cycles_per_step=recorded_net / STEPS,
        recorded_overhead_cycles=recorded_overhead,
        recorded_overhead_per_step=recorded_overhead / STEPS,
        boundary_overhead_cycles=overhead,
        step=step,
        frames_written=frames,
        checksum=checksum,
        radius_q12=radius,
        downrange_q32=downrange,
        radial_velocity_q24=radial_velocity,
        angular_momentum_q14=angular_momentum,
        mass_q12=mass,
        propellant_q12=propellant,
        final_frame_crc32=frame_crc,
        bytes_written=bytes_written,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=300.0)
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
        {(item.raw_net_cycles, item.recorded_net_cycles) for item in results}
    ) == 1
    print(
        json.dumps(
            {
                "vice": str(vice),
                "prg": str(prg),
                "runs": [asdict(item) for item in results],
                "stable": stable,
            },
            indent=2,
        )
    )
    return 0 if stable else 2


if __name__ == "__main__":
    raise SystemExit(main())
