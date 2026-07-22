#!/usr/bin/env python3
"""Run the Phase 1 C64 status PRG and verify its actual 40x25 screen memory."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(PROJECT_ROOT / "phase0" / "reference"))

from vice_timing import run_prg_until_result  # noqa: E402

SCREEN_START = 0x0400
SCREEN_END = 0x07E7


def screen_to_ascii(code: int) -> str:
    if 1 <= code <= 26:
        return chr(ord("A") + code - 1)
    if 32 <= code <= 63:
        return chr(code)
    return "?"


def rows(memory: bytes) -> list[str]:
    return [
        "".join(screen_to_ascii(code) for code in memory[offset : offset + 40]).rstrip()
        for offset in range(0, 1000, 40)
    ]


@dataclass(frozen=True)
class StatusDisplayResult:
    title: str
    mission_time: str
    altitude: str
    velocity: str
    acceleration: str
    mass: str
    propellant: str
    step: str
    frames: str
    stride: str
    checksum: str
    events: str
    altitude_error: str
    velocity_error: str
    raw_rate: str
    recorded_rate: str
    timing_note: str


def parse_result(memory: bytes) -> StatusDisplayResult | None:
    screen = rows(memory)
    if screen[23] != "POST-RUN DISPLAY - TIMING EXCLUDED":
        return None
    expected = {
        0: "KSA64 VERTICAL FLIGHT          COMPLETE",
        3: "MISSION TIME         256.000 S",
        4: "ALTITUDE             379.750 KM",
        5: "VELOCITY               1.874 KM/S",
        6: "ACCELERATION          -0.009 KM/S2",
        7: "MASS                 120.000 T",
        8: "PROPELLANT             0.000 T",
        10: "STEP                    2048",
        11: "FRAMES                   257",
        12: "STRIDE                     8",
        14: "STATE CHECKSUM      72BF6E0E",
        17: "CUTOFF  DEPLETED  END",
        18: "HP ALT DELTA        -279.355 M",
        19: "HP VEL DELTA          -2.857 M/S",
        20: "RAW PHYSICS       8.57 HZ",
        21: "RECORDED MODE     5.72 HZ",
        23: "POST-RUN DISPLAY - TIMING EXCLUDED",
    }
    for row, text in expected.items():
        if screen[row] != text:
            raise RuntimeError(
                f"screen row {row} mismatch: expected {text!r}, received {screen[row]!r}"
            )
    return StatusDisplayResult(
        title=screen[0],
        mission_time=screen[3],
        altitude=screen[4],
        velocity=screen[5],
        acceleration=screen[6],
        mass=screen[7],
        propellant=screen[8],
        step=screen[10],
        frames=screen[11],
        stride=screen[12],
        checksum=screen[14],
        events=screen[17],
        altitude_error=screen[18],
        velocity_error=screen[19],
        raw_rate=screen[20],
        recorded_rate=screen[21],
        timing_note=screen[23],
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()

    result = run_prg_until_result(
        args.vice.resolve(strict=True),
        args.prg.resolve(strict=True),
        args.timeout,
        SCREEN_START,
        SCREEN_END,
        parse_result,
    )
    print(json.dumps(asdict(result), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
