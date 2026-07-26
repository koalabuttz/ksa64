#!/usr/bin/env python3
"""Build the bounded stock-C64 KPH10 replay from canonical mission evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "phase10" / "evidence" / "ksa-g10r-nominal.kph10"
OUTPUT = ROOT / "phase10" / "evidence" / "ksa-g10r-stock.kph10"
MANIFEST = ROOT / "phase10" / "evidence" / "stock-replay-manifest-v1.json"
HEADER_LENGTH = 64
POINT_LENGTH = 48
STOCK_IDENTITY = 0x10A10003
TARGET_POINTS = 128


def crc32(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFFFFFF


def validate_point(point: bytes) -> None:
    if len(point) != POINT_LENGTH:
        raise ValueError("bad KPH10 point length")
    if struct.unpack_from("<I", point, 44)[0] != crc32(point[:44]):
        raise ValueError("bad KPH10 point CRC")
    if any(point[36:44]):
        raise ValueError("bad KPH10 point reserved bytes")


def build() -> tuple[bytes, dict]:
    source = SOURCE.read_bytes()
    if len(source) < HEADER_LENGTH or source[:5] != b"KPH10":
        raise ValueError("bad KPH10 source")
    if struct.unpack_from("<I", source, 60)[0] != crc32(source[:60]):
        raise ValueError("bad KPH10 header CRC")
    count = struct.unpack_from("<H", source, 40)[0]
    if len(source) != HEADER_LENGTH + count * POINT_LENGTH:
        raise ValueError("bad KPH10 stream length")
    points = [
        source[HEADER_LENGTH + index * POINT_LENGTH : HEADER_LENGTH + (index + 1) * POINT_LENGTH]
        for index in range(count)
    ]
    for point in points:
        validate_point(point)

    required = {0, count - 1}
    previous_frame = points[0][28]
    for index, point in enumerate(points):
        frame = point[28]
        events = struct.unpack_from("<H", point, 30)[0]
        if events:
            required.add(index)
        if frame != previous_frame:
            required.add(max(0, index - 1))
            required.add(index)
        previous_frame = frame
    if len(required) > TARGET_POINTS:
        raise ValueError("important KPH10 points exceed stock budget")
    selected = set(required)
    for slot in range(TARGET_POINTS):
        selected.add(round(slot * (count - 1) / (TARGET_POINTS - 1)))
    if len(selected) > TARGET_POINTS:
        optional = sorted(selected - required)
        selected = required | set(optional[: TARGET_POINTS - len(required)])
    candidate = 0
    while len(selected) < TARGET_POINTS:
        selected.add(candidate)
        candidate += 1
    indices = sorted(selected)

    header = bytearray(source[:HEADER_LENGTH])
    struct.pack_into("<I", header, 20, STOCK_IDENTITY)
    struct.pack_into("<H", header, 40, len(indices))
    struct.pack_into("<I", header, 60, crc32(header[:60]))
    output = bytes(header) + b"".join(points[index] for index in indices)
    manifest = {
        "schema": "ksa64.phase10.stock-replay-manifest-v1",
        "source": SOURCE.relative_to(ROOT).as_posix(),
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "output": OUTPUT.relative_to(ROOT).as_posix(),
        "output_sha256": hashlib.sha256(output).hexdigest(),
        "source_points": count,
        "retained_points": len(indices),
        "stock_identity": f"0x{STOCK_IDENTITY:08x}",
        "selection_sha256": hashlib.sha256(
            b"".join(struct.pack("<H", index) for index in indices)
        ).hexdigest(),
        "required_indices": sorted(required),
    }
    return output, manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    output, manifest = build()
    text = json.dumps(manifest, indent=2) + "\n"
    if args.check:
        if OUTPUT.read_bytes() != output:
            raise SystemExit(f"{OUTPUT} is stale")
        if MANIFEST.read_text() != text:
            raise SystemExit(f"{MANIFEST} is stale")
    else:
        OUTPUT.write_bytes(output)
        MANIFEST.write_text(text)
    print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
