#!/usr/bin/env python3
"""Measure finite Phase 5 PAL C64 kernels and freeze a stock-safe projection."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import run_prg_until_result  # noqa: E402

MAGIC = 0x3550544B
START = 0xC000
END = 0xC023
PAL_HZ = 985_248.0
MISSION_STEPS = 3_133
NAMES = {1: "vehicle", 2: "avionics", 3: "telemetry"}


def parse(kind: int):
    def inner(memory: bytes):
        if struct.unpack_from("<I", memory, 0)[0] != MAGIC:
            return None
        schema, actual, status, reserved = struct.unpack_from("<HHHH", memory, 4)
        if schema != 1 or actual != kind or reserved:
            raise RuntimeError("invalid Phase 5 timing result header")
        if status:
            raise RuntimeError(
                f"{NAMES[kind]} probe failed with status 0x{status:04x}"
            )
        overhead, cycles = struct.unpack_from("<II", memory, 12)
        values = struct.unpack_from("<IIII", memory, 20)
        return {"overhead": overhead, "cycles": cycles, "values": list(values)}

    return inner


def native_expected() -> dict:
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "ksa64-sim",
        "--features",
        "fixtures",
        "--example",
        "phase5_timing_native",
    ]
    output = subprocess.check_output(command, cwd=ROOT, text=True)
    return json.loads(output)


def signed(word: int) -> int:
    return struct.unpack("<i", struct.pack("<I", word))[0]


def check_values(kind: int, values: list[int], expected: dict) -> None:
    if kind == 1:
        actual = {
            "vehicle_step": values[0],
            "vehicle_position": [signed(value) for value in values[1:4]],
        }
    elif kind == 2:
        actual = {
            "sensor_checksum": values[0],
            "navigation_checksum": values[1],
            "flight_checksum": values[2],
            "command_crc32": values[3],
        }
    else:
        actual = {
            "observation_checksum": values[0],
            "telemetry_crc32": values[1],
            "telemetry_bytes": values[2],
        }
    for key, value in actual.items():
        if value != expected[key]:
            raise RuntimeError(
                f"{NAMES[kind]} native/MOS divergence at {key}: "
                f"{value} != {expected[key]}"
            )


def artifact_evidence(path: Path) -> dict:
    data = path.read_bytes()
    load_address = struct.unpack_from("<H", data, 0)[0]
    load_end_exclusive = load_address + len(data) - 2
    return {
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "load_address": load_address,
        "load_end_exclusive": load_end_exclusive,
        "stock_ram_fit": load_end_exclusive <= START,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--vehicle", type=Path, required=True)
    parser.add_argument("--avionics", type=Path, required=True)
    parser.add_argument("--telemetry", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=600)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    vice = args.vice.resolve(strict=True)
    expected = native_expected()
    results: dict[str, dict] = {}
    artifacts: dict[str, dict] = {}
    paths = ((1, args.vehicle), (2, args.avionics), (3, args.telemetry))
    for kind, path in paths:
        path = path.resolve(strict=True)
        samples = [
            run_prg_until_result(
                vice, path, args.timeout, START, END, parse(kind)
            )
            for _ in range(args.runs)
        ]
        if len({json.dumps(sample, sort_keys=True) for sample in samples}) != 1:
            raise RuntimeError(
                f"{NAMES[kind]} timing is not deterministic: {samples}"
            )
        result = samples[0]
        check_values(kind, result["values"], expected)
        results[NAMES[kind]] = {
            "cycles": result["cycles"],
            "boundary_overhead": result["overhead"],
            "runs": args.runs,
        }
        artifacts[NAMES[kind]] = artifact_evidence(path)

    per_step = sum(result["cycles"] for result in results.values())
    projected = per_step * MISSION_STEPS / PAL_HZ
    conservative = projected * 1.10
    versions = json.loads((ROOT / "toolchains/versions.json").read_text())
    stock_fit = all(artifact["stock_ram_fit"] for artifact in artifacts.values())
    eligible = stock_fit and conservative <= 1_800
    evidence = {
        "schema": "ksa64.phase5.c64-timing-v1",
        "target": "PAL C64 via pinned x64sc 3.10",
        "clock_hz": int(PAL_HZ),
        "mission_steps": MISSION_STEPS,
        "runs": args.runs,
        "stable": True,
        "rust_mos_image": versions["rustMos"]["repositoryDigest"],
        "vice_sha256": versions["vice"]["executableSha256"],
        "artifacts": artifacts,
        "cycles": {**results, "composed_per_mission_step": per_step},
        "full_nominal_decision": {
            "method": (
                "vehicle plus avionics plus canonical telemetry, "
                "with 10 percent projection margin"
            ),
            "projected_real_pal_seconds": projected,
            "conservative_seconds": conservative,
            "limit_seconds": 1_800,
            "eligible": eligible,
            "action": (
                "run full nominal only after explicit user confirmation"
                if eligible
                else "finite probes plus native mission evidence"
            ),
        },
        "routine_campaign_32_projected_days": conservative * 32 / 86_400,
        "reference_campaign_256_projected_days": conservative * 256 / 86_400,
        "native_expected": expected,
    }
    output = args.output if args.output.is_absolute() else ROOT / args.output
    text = json.dumps(evidence, indent=2) + "\n"
    if args.check:
        if not output.exists() or output.read_text() != text:
            raise RuntimeError("Phase 5 target timing differs from frozen evidence")
    else:
        output.write_text(text)
    print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())