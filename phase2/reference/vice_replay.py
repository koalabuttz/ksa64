#!/usr/bin/env python3
"""Run Phase 2's two-pass C64 replay and verify screen memory."""

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
class ReplayResult:
    title: str
    trajectory_cells: int
    time_and_step: str
    altitude_and_velocity: str
    mass_and_propellant: str
    stage_and_frames: str
    max_q_and_stride: str
    orbit: str
    checksum_and_cue_hash: str
    sid_schedule: str
    events_and_cues: str
    sink_memory: str
    footer: str


def parse_result(memory: bytes) -> ReplayResult | None:
    screen = rows(memory)
    if screen[24].startswith("REPLAY ERROR"):
        raise RuntimeError(f"C64 replay reported {screen[24]}")
    if screen[24] != "POST-RUN REPLAY - TIMING EXCLUDED":
        return None
    if screen[0] != "KSA64 KSA-2A POST-RUN REPLAY   ORBIT":
        raise RuntimeError(f"Unexpected replay title: {screen[0]!r}")
    if screen[1] != "ALTITUDE / DOWNRANGE TRAJECTORY":
        raise RuntimeError(f"Unexpected trajectory heading: {screen[1]!r}")
    trajectory_cells = sum(row.count("*") for row in screen[2:13])
    if trajectory_cells < 20:
        raise RuntimeError(f"Replay drew only {trajectory_cells} trajectory cells")
    expected = {
        14: "T+    900.000 S STEP          7200",
        15: "ALT     197.666 KM VR     -0.008",
        16: "MASS     23.094 T PROP     0.094",
        17: "STAGE  2 COMPLETE FRAMES          901",
        18: "MAX Q      40.779 KPA STRIDE        8",
        19: "ORBIT   188.169 X   188.169 KM",
        21: "SID IGN  1 CUT  2 SEP  1 END  1 ALM  0",
        22: "EVENT MASK  00000027  CUES     5",
        24: "POST-RUN REPLAY - TIMING EXCLUDED",
    }
    for row, text in expected.items():
        if screen[row] != text:
            raise RuntimeError(
                f"screen row {row} mismatch: expected {text!r}, received {screen[row]!r}"
            )
    if not screen[20].startswith("CHECKSUM  CC57612B   CUE HASH "):
        raise RuntimeError(f"Unexpected checksum/cue row: {screen[20]!r}")
    if not screen[23].startswith("REPLAY SINK") or not screen[23].endswith("BYTES"):
        raise RuntimeError(f"Unexpected sink-memory row: {screen[23]!r}")
    return ReplayResult(
        title=screen[0],
        trajectory_cells=trajectory_cells,
        time_and_step=screen[14],
        altitude_and_velocity=screen[15],
        mass_and_propellant=screen[16],
        stage_and_frames=screen[17],
        max_q_and_stride=screen[18],
        orbit=screen[19],
        checksum_and_cue_hash=screen[20],
        sid_schedule=screen[21],
        events_and_cues=screen[22],
        sink_memory=screen[23],
        footer=screen[24],
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=1200.0)
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
