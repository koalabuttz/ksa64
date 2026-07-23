#!/usr/bin/env python3
"""Measure finite Phase 3 probes under PAL VICE and compare native evidence."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "phase0/reference"))
from vice_timing import run_prg_until_result  # noqa: E402

MAGIC = 0x3350544B
START = 0xC000
END = 0xC05B
PAL_HZ = 985_248.0
STEPS = 64
MISSION_STEPS = 7200


@dataclass(frozen=True)
class ProbeResult:
    boundary_overhead_cycles: int
    composed_cycles: int
    guidance_cycles: int
    fault_cycles: int
    coast_cycles: int
    actuator_cycles: int
    truth_checksum: int
    sensor_checksum: int
    nav_checksum: int
    flight_checksum: int
    radius_q12: int
    guidance_nav_checksum: int
    guidance_flight_checksum: int
    fault_nav_checksum: int
    fault_flight_checksum: int
    coast_radius_q12: int
    coast_radial_velocity_q24: int
    actuator_hash: int
    guidance_mode: int
    fault_mode: int
    alarms: int
    composed_steps: int
    coast_steps: int


def parse(memory: bytes) -> ProbeResult | None:
    if struct.unpack_from("<I", memory, 0)[0] != MAGIC:
        return None
    schema, status = struct.unpack_from("<HH", memory, 4)
    if schema != 1:
        raise RuntimeError(f"unsupported Phase 3 probe schema {schema}")
    if status:
        raise RuntimeError(f"Phase 3 probe runner status 0x{status:04x}")
    overhead, composed, guidance, fault, coast, actuator = struct.unpack_from("<IIIIII", memory, 8)
    truth, sensor, nav, flight = struct.unpack_from("<IIII", memory, 32)
    radius = struct.unpack_from("<i", memory, 48)[0]
    guidance_nav, guidance_flight, fault_nav, fault_flight = struct.unpack_from("<IIII", memory, 52)
    coast_radius, coast_vr = struct.unpack_from("<ii", memory, 68)
    actuator_hash = struct.unpack_from("<I", memory, 76)[0]
    modes, alarms = struct.unpack_from("<HH", memory, 80)
    composed_steps, coast_steps = struct.unpack_from("<II", memory, 84)
    if composed_steps != STEPS or coast_steps != STEPS:
        raise RuntimeError("finite probe step count changed")
    return ProbeResult(
        boundary_overhead_cycles=overhead,
        composed_cycles=composed,
        guidance_cycles=guidance,
        fault_cycles=fault,
        coast_cycles=coast,
        actuator_cycles=actuator,
        truth_checksum=truth,
        sensor_checksum=sensor,
        nav_checksum=nav,
        flight_checksum=flight,
        radius_q12=radius,
        guidance_nav_checksum=guidance_nav,
        guidance_flight_checksum=guidance_flight,
        fault_nav_checksum=fault_nav,
        fault_flight_checksum=fault_flight,
        coast_radius_q12=coast_radius,
        coast_radial_velocity_q24=coast_vr,
        actuator_hash=actuator_hash,
        guidance_mode=modes & 0xFF,
        fault_mode=modes >> 8,
        alarms=alarms,
        composed_steps=composed_steps,
        coast_steps=coast_steps,
    )


def native_expected() -> dict:
    command = [
        "cargo", "run", "-q", "-p", "ksa64-sim", "--features", "fixtures",
        "--example", "phase3_probe_native",
    ]
    output = subprocess.check_output(command, cwd=ROOT, text=True)
    return json.loads(output.strip())


def compare(result: ProbeResult, expected: dict) -> None:
    fields = (
        "truth_checksum", "sensor_checksum", "nav_checksum", "flight_checksum",
        "radius_q12", "guidance_nav_checksum", "guidance_flight_checksum",
        "fault_nav_checksum", "fault_flight_checksum", "coast_radius_q12",
        "coast_radial_velocity_q24", "actuator_hash", "guidance_mode", "fault_mode",
    )
    for field in fields:
        if getattr(result, field) != expected[field]:
            raise RuntimeError(
                f"native/MOS first divergence at {field}: native={expected[field]} MOS={getattr(result, field)}"
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vice", type=Path, required=True)
    parser.add_argument("--prg", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=600.0)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    vice = args.vice.resolve(strict=True)
    prg = args.prg.resolve(strict=True)
    expected = native_expected()
    results = [run_prg_until_result(vice, prg, args.timeout, START, END, parse) for _ in range(args.runs)]
    for result in results:
        compare(result, expected)
    stable = len({tuple(asdict(result).values()) for result in results}) == 1
    if not stable:
        raise RuntimeError("Phase 3 target probe timing or state was not deterministic")
    result = results[0]
    artifact = prg.read_bytes()
    load_address = struct.unpack_from("<H", artifact, 0)[0]
    load_end_exclusive = load_address + len(artifact) - 2
    stock_ram_fit = load_end_exclusive <= START
    composed_per_step = result.composed_cycles / STEPS
    guidance_per_step = result.guidance_cycles / STEPS
    conservative_per_step = composed_per_step + guidance_per_step
    projected_seconds = conservative_per_step * MISSION_STEPS / PAL_HZ
    eligible = stock_ram_fit and projected_seconds <= 30.0 * 60.0
    versions = json.loads((ROOT / "toolchains/versions.json").read_text())
    evidence = {
        "schema": "ksa64.phase3.c64-timing-v1",
        "target": "PAL C64 via pinned x64sc 3.10",
        "clock_hz": int(PAL_HZ),
        "probe_steps": STEPS,
        "mission_steps": MISSION_STEPS,
        "runs": args.runs,
        "stable": stable,
        "rust_mos_image": versions["rustMos"]["repositoryDigest"],
        "vice_sha256": versions["vice"]["executableSha256"],
        "artifact": {
            "bytes": len(artifact),
            "sha256": hashlib.sha256(artifact).hexdigest(),
            "load_address": load_address,
            "load_end_exclusive": load_end_exclusive,
            "result_buffer_address": START,
            "stock_ram_fit": stock_ram_fit,
        },
        "cycles": {
            "boundary_overhead": result.boundary_overhead_cycles,
            "composed_64_steps": result.composed_cycles,
            "composed_per_step": composed_per_step,
            "gps_guidance_64_steps": result.guidance_cycles,
            "gps_guidance_per_step": guidance_per_step,
            "stuck_fault_64_steps": result.fault_cycles,
            "coast_64_steps": result.coast_cycles,
            "actuator_64_steps": result.actuator_cycles,
        },
        "full_nominal_decision": {
            "method": "conservative composed plus GPS-guidance cycles per step",
            "projected_real_pal_seconds": projected_seconds,
            "limit_seconds": 1800,
            "eligible": eligible,
            "action": "run full nominal" if eligible else "representative probes plus native and replay validation",
        },
        "native_expected": expected,
        "target_result": asdict(result),
    }
    output = args.output if args.output.is_absolute() else ROOT / args.output
    data = json.dumps(evidence, indent=2) + "\n"
    if args.check:
        if not output.exists() or output.read_text() != data:
            raise RuntimeError("Phase 3 C64 timing differs from frozen timing-v1.json")
    else:
        output.write_text(data)
    print(data, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())