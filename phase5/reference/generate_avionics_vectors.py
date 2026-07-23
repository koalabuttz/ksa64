#!/usr/bin/env python3
"""Generate independent Phase 5 transport and controller vectors."""

from __future__ import annotations
import argparse
import hashlib
import json
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "phase5" / "generated"

SENSOR_LEN = 128
COMMAND_LEN = 32
INITIAL_Q = [1040703765, 0, -264305086, 0]
GIMBAL_LIMIT = 6863
RCS_LIMIT = 32767


def sensor_bytes() -> bytes:
    b = bytearray(SENSOR_LEN)
    struct.pack_into("<IiHH", b, 0, 17, 139264, 0x3F, 0x601)
    struct.pack_into("<3i", b, 12, -1, 2, -3)
    struct.pack_into("<3i", b, 24, 4, -5, 6)
    struct.pack_into("<i", b, 36, 7)
    struct.pack_into("<3i", b, 40, 8, -9, 10)
    struct.pack_into("<3i", b, 52, -11, 12, -13)
    struct.pack_into("<4i", b, 64, 1 << 30, 14, -15, 16)
    struct.pack_into("<2iiBBB", b, 80, 17, -18, 19, 1, 1, 1)
    struct.pack_into("<I", b, 124, zlib.crc32(b[:124]) & 0xFFFFFFFF)
    return bytes(b)


def command_bytes() -> bytes:
    b = bytearray(COMMAND_LEN)
    struct.pack_into("<I2i3iBBBB", b, 0, 4, 123, -456, 1, -2, 3, 1, 1, 0, 0)
    struct.pack_into("<I", b, 28, zlib.crc32(b[:28]) & 0xFFFFFFFF)
    return bytes(b)


def product(a: int, b: int) -> int:
    return (a * b) >> 30


def quaternion_error(desired: list[int], current: list[int]) -> list[int]:
    c = [current[0], -current[1], -current[2], -current[3]]
    return [
        product(desired[0], c[0]) - product(desired[1], c[1]) - product(desired[2], c[2]) - product(desired[3], c[3]),
        product(desired[0], c[1]) + product(desired[1], c[0]) + product(desired[2], c[3]) - product(desired[3], c[2]),
        product(desired[0], c[2]) - product(desired[1], c[3]) + product(desired[2], c[0]) + product(desired[3], c[1]),
        product(desired[0], c[3]) + product(desired[1], c[2]) - product(desired[2], c[1]) + product(desired[3], c[0]),
    ]


def clamp(value: int, limit: int) -> int:
    return min(limit, max(-limit, value))


def controller(desired: list[int], rate: list[int]) -> list[int]:
    e = quaternion_error(desired, INITIAL_Q)
    if e[0] < 0:
        e = [-v for v in e]
    rate_error = [-v for v in rate]
    pitch_raw = -(e[2] >> 13) - (rate_error[1] >> 9)
    yaw_raw = -(e[3] >> 13) - (rate_error[2] >> 9)
    pitch = 0 if abs(pitch_raw) <= 1 else clamp(pitch_raw, GIMBAL_LIMIT)
    yaw = 0 if abs(yaw_raw) <= 1 else clamp(yaw_raw, GIMBAL_LIMIT)
    rcs = [
        clamp((e[1] >> 14) + (rate_error[0] >> 9), RCS_LIMIT),
        clamp((e[2] >> 14) + (rate_error[1] >> 9), RCS_LIMIT),
        clamp((e[3] >> 14) + (rate_error[2] >> 9), RCS_LIMIT),
    ]
    return [pitch, yaw, *rcs]


def axis_target(axis: int, degrees: float) -> list[int]:
    half = math.radians(degrees) / 2.0
    q = [round(math.cos(half) * (1 << 30)), 0, 0, 0]
    q[axis + 1] = round(math.sin(half) * (1 << 30))
    # Desired is a body-frame offset from the accepted initial attitude.
    w, x, y, z = INITIAL_Q
    a, b, c, d = q
    return [
        (a*w - b*x - c*y - d*z) >> 30,
        (a*x + b*w + c*z - d*y) >> 30,
        (a*y - b*z + c*w + d*x) >> 30,
        (a*z + b*y - c*x + d*w) >> 30,
    ]


def fnv(data: bytes, h: int = 2166136261) -> int:
    for byte in data:
        h ^= byte
        h = (h * 16777619) & 0xFFFFFFFF
    return h


def rust_array(data: bytes) -> str:
    return ", ".join(str(v) for v in data)


def write_or_check(path: Path, text: str, check: bool) -> None:
    payload = text.encode("utf-8")
    digest = hashlib.sha256(payload).hexdigest()
    digest_text = f"{digest}  {path.name}\n"
    digest_path = path.with_suffix(path.suffix + ".sha256")
    if check:
        if not path.exists() or path.read_bytes() != payload:
            raise SystemExit(f"stale generated artifact: {path.relative_to(ROOT)}")
        if not digest_path.exists() or digest_path.read_text(encoding="ascii") != digest_text:
            raise SystemExit(f"stale generated digest: {digest_path.relative_to(ROOT)}")
        return
    path.write_bytes(payload)
    digest_path.write_text(digest_text, encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    sensor = sensor_bytes()
    command = command_bytes()
    cases = [
        {"name": "hold", "target_q30": INITIAL_Q, "rate_q24": [0, 0, 0]},
        {"name": "pitch_10deg", "target_q30": axis_target(1, 10.0), "rate_q24": [0, 0, 0]},
        {"name": "roll_6deg_with_rate", "target_q30": axis_target(0, 6.0), "rate_q24": [1000, -2000, 3000]},
        {"name": "yaw_minus_8deg", "target_q30": axis_target(2, -8.0), "rate_q24": [0, 0, 0]},
    ]
    for case in cases:
        case["expected"] = controller(case["target_q30"], case["rate_q24"])
    signature = fnv(sensor)
    signature = fnv(command, signature)
    for case in cases:
        signature = fnv(struct.pack("<5i", *case["expected"]), signature)
    payload = {
        "contract": "phase5-avionics-v1",
        "sensor_hex": sensor.hex(),
        "sensor_crc32": struct.unpack_from("<I", sensor, 124)[0],
        "command_hex": command.hex(),
        "command_crc32": struct.unpack_from("<I", command, 28)[0],
        "controller_cases": cases,
        "signature": signature,
    }
    json_text = json.dumps(payload, indent=2) + "\n"
    lines = [
        "// Generated by phase5/reference/generate_avionics_vectors.py.",
        "// Do not edit by hand.",
        "",
        f"pub const AVIONICS_SIGNATURE: u32 = 0x{signature:08x};",
        f"pub const SENSOR_BYTES: [u8; 128] = [{rust_array(sensor)}];",
        f"pub const COMMAND_BYTES: [u8; 32] = [{rust_array(command)}];",
        f"pub const CONTROLLER_TARGET_Q30: [[i32; 4]; {len(cases)}] = [",
    ]
    lines += ["    [" + ", ".join(map(str, c["target_q30"])) + "]," for c in cases]
    lines += ["];"]
    lines += [f"pub const CONTROLLER_RATE_Q24: [[i32; 3]; {len(cases)}] = ["]
    lines += ["    [" + ", ".join(map(str, c["rate_q24"])) + "]," for c in cases]
    lines += ["];"]
    lines += [f"pub const CONTROLLER_EXPECTED: [[i32; 5]; {len(cases)}] = ["]
    lines += ["    [" + ", ".join(map(str, c["expected"])) + "]," for c in cases]
    lines += ["];", ""]
    rust_text = "\n".join(lines)
    write_or_check(OUT / "avionics-v1.json", json_text, args.check)
    write_or_check(OUT / "avionics_v1.rs", rust_text, args.check)
    if not args.check:
        print(f"phase5 avionics signature 0x{signature:08x}")


if __name__ == "__main__":
    main()