#!/usr/bin/env python3
"""Generate the strict Phase 10 compiled-atmosphere pack and evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "phase10" / "generated"
LENGTH = 128 + 64 * 40 + 4
EARTH_ID = 0x10EA_2024
ATMOSPHERE_ID = 0x4154_1001
CONTRACT_ID = 0x10E0_0001

# Representative U.S. Standard Atmosphere 1976 samples. The values through
# 100 km follow the published standard tables. Sparse thermospheric samples
# retain the standard's idealized character; KSA64 linearly interpolates the
# compiled values and declares zero density only beyond 200 km.
TABLE = [
    # altitude km, density kg/m3, pressure Pa, temperature K, east/north/up wind m/s
    (-1.0, 1.3470, 113929.0, 294.65, 0.0, 0.0, 0.0),
    (0.0, 1.225000, 101325.0, 288.15, 0.0, 0.0, 0.0),
    (5.0, 0.736116, 54019.9, 255.65, 1.0, 0.0, 0.0),
    (10.0, 0.413510, 26436.3, 223.15, 2.0, 0.0, 0.0),
    (15.0, 0.194755, 12044.6, 216.65, 3.0, 0.0, 0.0),
    (20.0, 0.088910, 5474.89, 216.65, 4.0, 0.0, 0.0),
    (25.0, 0.040084, 2511.02, 221.65, 5.0, 0.0, 0.0),
    (30.0, 0.018410, 1171.87, 226.51, 6.0, 0.0, 0.0),
    (40.0, 0.003996, 287.14, 250.35, 7.0, 0.0, 0.0),
    (50.0, 0.001027, 79.779, 270.65, 8.0, 0.0, 0.0),
    (60.0, 0.0003097, 21.958, 247.02, 9.0, 0.0, 0.0),
    (70.0, 0.00008283, 5.2209, 219.59, 10.0, 0.0, 0.0),
    (80.0, 0.00001846, 1.0525, 198.64, 8.0, 0.0, 0.0),
    (90.0, 0.000003416, 0.1836, 186.87, 5.0, 0.0, 0.0),
    (100.0, 0.0000005606, 0.03201, 195.08, 2.0, 0.0, 0.0),
    (110.0, 0.00000009658, 0.00710, 240.0, 0.0, 0.0, 0.0),
    (120.0, 0.00000002222, 0.00250, 360.0, 0.0, 0.0, 0.0),
    (140.0, 0.00000000388, 0.00072, 560.0, 0.0, 0.0, 0.0),
    (160.0, 0.00000000123, 0.00030, 720.0, 0.0, 0.0, 0.0),
    (180.0, 0.0000000005464, 0.00015, 850.0, 0.0, 0.0, 0.0),
    (200.0, 0.0000000002789, 0.000084, 950.0, 0.0, 0.0, 0.0),
]


def q(value: float, bits: int) -> int:
    raw = int(math.floor(abs(value) * (1 << bits) + 0.5))
    return -raw if value < 0 else raw


def sound_speed(temp_k: float) -> float:
    return math.sqrt(1.4 * 287.05287 * temp_k)


def build() -> tuple[bytes, dict]:
    out = bytearray(LENGTH)
    out[:5] = b"KAT10"
    struct.pack_into("<HHHIII", out, 6, 10, 128, 3, LENGTH, CONTRACT_ID, ATMOSPHERE_ID)
    source_text = json.dumps(TABLE, separators=(",", ":"), sort_keys=False).encode()
    source_hash = zlib.crc32(source_text) & 0xFFFFFFFF
    struct.pack_into("<II", out, 32, EARTH_ID, source_hash)
    out[40:43] = bytes((1, len(TABLE), 1))
    struct.pack_into("<ii", out, 44, q(TABLE[0][0], 12), q(TABLE[-1][0], 12))
    records = []
    for index, row in enumerate(TABLE):
        altitude, density, pressure, temperature, east, north, up = row
        values = (
            q(altitude, 12),
            q(density, 28),
            q(pressure, 14),
            q(temperature, 16),
            q(sound_speed(temperature), 16),
            q(east, 19),
            q(north, 19),
            q(up, 19),
        )
        struct.pack_into("<8i", out, 128 + index * 40, *values)
        records.append(
            {
                "altitude_km": altitude,
                "density_kg_m3": density,
                "pressure_pa": pressure,
                "temperature_k": temperature,
                "speed_of_sound_m_s": sound_speed(temperature),
                "raw": list(values),
            }
        )
    struct.pack_into("<I", out, LENGTH - 4, zlib.crc32(out[:-4]) & 0xFFFFFFFF)
    evidence = {
        "format": "KAT10",
        "identity": f"0x{ATMOSPHERE_ID:08x}",
        "earth_identity": f"0x{EARTH_ID:08x}",
        "source_model": "U.S. Standard Atmosphere 1976 compiled representative profile",
        "source_url": "https://ntrs.nasa.gov/citations/19770009539",
        "runtime_interpolation": "piecewise linear fixed point",
        "terminal_behavior": "zero above 200 km",
        "records": records,
    }
    return bytes(out), evidence


def render() -> dict[Path, bytes]:
    pack, evidence = build()
    evidence_bytes = (json.dumps(evidence, indent=2, sort_keys=True) + "\n").encode()
    hashes = {
        "ksa-g10r.kat10": hashlib.sha256(pack).hexdigest(),
        "atmosphere-fixtures-v1.json": hashlib.sha256(evidence_bytes).hexdigest(),
    }
    return {
        OUT / "ksa-g10r.kat10": pack,
        OUT / "atmosphere-fixtures-v1.json": evidence_bytes,
        OUT / "atmosphere-hashes-v1.json": (
            json.dumps(hashes, indent=2, sort_keys=True) + "\n"
        ).encode(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    failures = []
    for path, content in render().items():
        if args.check:
            if not path.exists() or path.read_bytes() != content:
                failures.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    if failures:
        print("stale: " + ", ".join(failures))
        return 1
    print("phase10 environment fixtures: " + ("PASS" if args.check else "generated"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
