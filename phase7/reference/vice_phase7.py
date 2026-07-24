#!/usr/bin/env python3
"""Run the finite Phase 7 stock-C64 replay and full-mission acceptance probes."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0" / "reference"))
from vice_timing import run_prg_until_result

PAL_CLOCK_HZ = 985_248
FULL_MAGIC = 0x37544B53
REPLAY_MAGIC = 0x37554B53
FULL_START = 0xC000
FULL_END = 0xC029
REPLAY_START = 0x0400
REPLAY_END = 0xC017
RESULT_OFFSET = 0xC000 - REPLAY_START
EXPECTED_CHECKSUM = 0xA61C5720
EXPECTED_EVENTS = 0x1FF
EXPECTED_STEPS = 2_702
EXPECTED_APOGEE_RAW = 8_012_317
EXPECTED_IMPACT_RAW = -3_227_800


def decode_screen(screen: bytes) -> list[str]:
    def char(value: int) -> str:
        if 1 <= value <= 26:
            return chr(64 + value)
        if 32 <= value <= 63:
            return chr(value)
        return "?"

    return [
        "".join(char(value) for value in screen[offset : offset + 40]).rstrip()
        for offset in range(0, 1_000, 40)
    ]


def parse_replay(memory: bytes) -> dict[str, object] | None:
    result = memory[RESULT_OFFSET : RESULT_OFFSET + 24]
    if struct.unpack_from("<I", result, 0)[0] != REPLAY_MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", result, 4)
    if schema != 1 or status != 0:
        raise RuntimeError(f"invalid replay result schema={schema} status={status}")
    screen = memory[:1_000]
    screen_crc, state_checksum, points, stream_identity = struct.unpack_from(
        "<IIII", result, 8
    )
    actual_crc = zlib.crc32(screen)
    if screen_crc != actual_crc:
        raise RuntimeError(
            f"screen CRC mismatch: target={screen_crc:08x} host={actual_crc:08x}"
        )
    if state_checksum != EXPECTED_CHECKSUM or points != 124:
        raise RuntimeError(
            f"replay identity mismatch checksum={state_checksum:08x} points={points}"
        )
    rows = decode_screen(screen)
    expected = {
        0: "    KSA64 PHASE 7 MISSION CONTROL",
        2: "  FIRESTORM 54 / AEROTECH I211W",
        3: "  STATUS: COMPLETE - RECOVERED",
        24: "        PHASE 7 REPLAY PASS",
    }
    for row, value in expected.items():
        if rows[row] != value:
            raise RuntimeError(f"row {row}: expected {value!r}, got {rows[row]!r}")
    plot_cells = sum(value != 32 for value in screen[9 * 40 : 22 * 40])
    if plot_cells < 40:
        raise RuntimeError(f"trajectory plot contains only {plot_cells} visible cells")
    return {
        "screen_crc32": f"{screen_crc:08x}",
        "screen_sha256": hashlib.sha256(screen).hexdigest(),
        "state_checksum": f"{state_checksum:08x}",
        "point_count": points,
        "plot_cells": plot_cells,
        "stream_identity": f"{stream_identity:08x}",
        "rows": {str(row): rows[row] for row in expected},
    }


def parse_full(memory: bytes) -> dict[str, object] | None:
    if struct.unpack_from("<I", memory, 0)[0] != FULL_MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"invalid full-mission schema={schema}")
    elapsed, overhead, net, steps = struct.unpack_from("<IIII", memory, 8)
    apogee, impact = struct.unpack_from("<ii", memory, 24)
    checksum, events = struct.unpack_from("<II", memory, 32)
    faults, outcome = memory[40], memory[41]
    if net != (elapsed - overhead) & 0xFFFFFFFF:
        raise RuntimeError("target reported inconsistent cycle accounting")
    expected = (
        steps == EXPECTED_STEPS
        and apogee == EXPECTED_APOGEE_RAW
        and impact == EXPECTED_IMPACT_RAW
        and checksum == EXPECTED_CHECKSUM
        and events == EXPECTED_EVENTS
        and faults == 0
        and outcome == 0
    )
    if status != 0 or not expected:
        raise RuntimeError(
            "full mission disagrees with exact host result: "
            f"status={status} steps={steps} apogee={apogee} impact={impact} checksum={checksum:08x} "
            f"events={events:08x} faults={faults} outcome={outcome}"
        )
    cpu_seconds = net / PAL_CLOCK_HZ
    return {
        "elapsed_cycles": elapsed,
        "boundary_overhead_cycles": overhead,
        "net_cycles": net,
        "projected_pal_cpu_seconds": cpu_seconds,
        "steps": steps,
        "cycles_per_step": net / steps,
        "apogee_raw_q13": apogee,
        "impact_velocity_raw_q19": impact,
        "state_checksum": f"{checksum:08x}",
        "event_history": f"{events:08x}",
        "numeric_faults": faults,
        "outcome": "landed",
        "under_30_minute_policy": cpu_seconds <= 1_800.0,
    }


def artifact(path: Path) -> dict[str, object]:
    payload = path.read_bytes()
    load = struct.unpack_from("<H", payload, 0)[0]
    end = load + len(payload) - 2
    return {
        "path": path.name,
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "load_address": f"{load:04x}",
        "end_address": f"{end:04x}",
        "stock_fit": end <= 0xC000,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--full-prg", type=Path, required=True)
    parser.add_argument("--replay-prg", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=1_800.0)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--replay-only", action="store_true")
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    full_prg = args.full_prg.resolve(strict=True)
    replay_prg = args.replay_prg.resolve(strict=True)
    replay = run_prg_until_result(
        vice, replay_prg, 180.0, REPLAY_START, REPLAY_END, parse_replay
    )
    if args.replay_only:
        data = {"replay": replay, "artifact": artifact(replay_prg)}
        print(json.dumps(data, indent=2))
        if not args.check or args.output is None:
            return 0
        expected = json.loads(args.output.read_text())
        if expected["replay"] != replay or expected["artifacts"]["replay"] != data["artifact"]:
            raise RuntimeError(f"replay evidence differs from {args.output}")
        return 0

    full = run_prg_until_result(
        vice, full_prg, args.timeout, FULL_START, FULL_END, parse_full
    )
    data = {
        "schema": "ksa64.phase7.c64-execution-v1",
        "target": "PAL stock C64 via pinned x64sc 3.10",
        "vice": "x64sc 3.10 (pinned)",
        "replay": replay,
        "full_mission": full,
        "artifacts": {
            "replay": artifact(replay_prg),
            "full_mission": artifact(full_prg),
        },
    }
    text = json.dumps(data, indent=2) + "\n"
    print(text, end="")
    if args.check:
        if args.output is None:
            raise RuntimeError("--check requires --output")
        if json.loads(args.output.read_text()) != data:
            raise RuntimeError(f"C64 evidence differs from {args.output}")
    elif args.output is not None:
        args.output.write_text(text)
    return 0 if full["under_30_minute_policy"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
