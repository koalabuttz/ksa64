#!/usr/bin/env python3
"""Generate frozen Phase 10 Earth/frame fixtures.

Generation requires the pinned SatKit 0.16.0 plus satkit-data 0.9.0. Validation
of checked-in artifacts uses only the Python standard library.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import struct
import sys
import zlib
from datetime import date
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
PHASE = ROOT / "phase10"
GENERATED = PHASE / "generated"
FIXTURE_JSON = GENERATED / "frame-fixtures-v1.json"
EARTH_BIN = GENERATED / "ksa-g10r.kem10"
TRANSFORM_BIN = GENERATED / "ksa-g10r.kft10"
SOURCE_MANIFEST = PHASE / "source-data" / "source-manifest.json"
LEAP_SOURCE = PHASE / "source-data" / "leap-seconds-v1.csv"
EOP_SOURCE = PHASE / "source-data" / "eop-accepted-epoch-v1.csv"

KEM_LENGTH = 512
KFT_HEADER_LENGTH = 128
KNOT_LENGTH = 48
MAX_KNOTS = 128
KFT_LENGTH = KFT_HEADER_LENGTH + KNOT_LENGTH * MAX_KNOTS + 4
CONTRACT_ID = 0x10E00001
VERSION = 10
EARTH_ID = 0x10EA2024
TRANSFORM_ID = 0x10F72024
Q30 = 1 << 30

WGS84_A_M = 6_378_137.0
WGS84_INV_F = 298.257223563
WGS84_F = 1.0 / WGS84_INV_F
WGS84_E2 = WGS84_F * (2.0 - WGS84_F)


def load_csv(path: Path) -> list[dict[str, str]]:
    lines = [
        line
        for line in path.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    ]
    return list(csv.DictReader(lines))


def unix_day(iso: str) -> int:
    return (date.fromisoformat(iso[:10]) - date(1970, 1, 1)).days


def round_away(value: float) -> int:
    if value >= 0.0:
        return math.floor(value + 0.5)
    return math.ceil(value - 0.5)


def qraw(value: float, bits: int) -> int:
    raw = round_away(value * (1 << bits))
    if not -(1 << 31) <= raw < (1 << 31):
        raise OverflowError((value, bits, raw))
    return raw


def pack_header(buffer: bytearray, magic: bytes, kind: int, identity: int) -> None:
    buffer[:] = b"\0" * len(buffer)
    buffer[0:5] = magic
    struct.pack_into("<HHHIII", buffer, 6, VERSION, 32, kind, len(buffer), CONTRACT_ID, identity)


def seal(buffer: bytearray) -> None:
    struct.pack_into("<I", buffer, len(buffer) - 4, zlib.crc32(buffer[:-4]) & 0xFFFFFFFF)


def normalized_sha_crc(path: Path) -> int:
    return zlib.crc32(path.read_bytes()) & 0xFFFFFFFF


def build_earth() -> bytes:
    source = json.loads(SOURCE_MANIFEST.read_text(encoding="utf-8"))
    leaps = load_csv(LEAP_SOURCE)
    output = bytearray(KEM_LENGTH)
    pack_header(output, b"KEM10", 1, EARTH_ID)
    output[32:35] = bytes((1, 1, 1))
    struct.pack_into(
        "<7ih2xiiIIIBxh",
        output,
        36,
        26_124_849,
        26_037_257,
        312_745_366,
        102_041_713,
        1_162_465,
        78_298,
        unix_day(source["accepted_epoch_utc"]),
        37,
        unix_day(source["earth_orientation"]["normalized_window_start_utc"]),
        unix_day(source["earth_orientation"]["normalized_window_end_utc"]),
        normalized_sha_crc(LEAP_SOURCE),
        normalized_sha_crc(EOP_SOURCE),
        zlib.crc32(b"WGS84|IERS2010|IAU2006-2000A|TAI") & 0xFFFFFFFF,
        len(leaps),
        10,
    )
    for index, leap in enumerate(leaps):
        struct.pack_into(
            "<ih2x",
            output,
            96 + index * 8,
            unix_day(leap["effective_utc"]),
            int(leap["tai_minus_utc_after"]),
        )
    seal(output)
    return bytes(output)


def geodetic_fixture(name: str, latitude_deg: float, longitude_deg: float, altitude_m: float) -> dict[str, Any]:
    lat = math.radians(latitude_deg)
    lon = math.radians(longitude_deg)
    sin_lat = math.sin(lat)
    cos_lat = math.cos(lat)
    sin_lon = math.sin(lon)
    cos_lon = math.cos(lon)
    n = WGS84_A_M / math.sqrt(1.0 - WGS84_E2 * sin_lat * sin_lat)
    ecef = [
        (n + altitude_m) * cos_lat * cos_lon,
        (n + altitude_m) * cos_lat * sin_lon,
        (n * (1.0 - WGS84_E2) + altitude_m) * sin_lat,
    ]
    enu_to_ecef = [
        [-sin_lon, -sin_lat * cos_lon, cos_lat * cos_lon],
        [cos_lon, -sin_lat * sin_lon, cos_lat * sin_lon],
        [0.0, cos_lat, sin_lat],
    ]
    return {
        "name": name,
        "latitude_deg": latitude_deg,
        "longitude_deg": longitude_deg,
        "altitude_m": altitude_m,
        "ecef_m": ecef,
        "enu_to_ecef": enu_to_ecef,
        "reference_meridian_deg": longitude_deg if abs(latitude_deg) == 90.0 else None,
    }


def require_satkit() -> Any:
    try:
        import numpy as np
        import satkit as sk
    except ImportError as error:
        raise SystemExit(
            "generation requires pinned SatKit; validation with --check does not"
        ) from error
    if str(sk.__version__) != "0.16.0":
        raise SystemExit(f"expected SatKit 0.16.0, got {sk.__version__}")
    return sk, np


def angular_kinematics(sk: Any, np: Any, tm: Any) -> tuple[Any, Any]:
    rotation = sk.frametransform.qitrf2gcrf(tm).as_rotation_matrix()
    velocity_columns = []
    for axis in range(3):
        basis = np.zeros(3)
        basis[axis] = 1.0
        _, velocity = sk.frametransform.itrf_to_gcrf_state(basis, np.zeros(3), tm)
        velocity_columns.append(velocity)
    velocity_map = np.column_stack(velocity_columns)
    skew = velocity_map @ rotation.T
    omega = np.array([skew[2, 1], skew[0, 2], skew[1, 0]])
    return rotation, omega


def transform_record(sk: Any, np: Any, epoch: Any, seconds: int) -> dict[str, Any]:
    tm = epoch + float(seconds) / 86_400.0
    q = sk.frametransform.qitrf2gcrf(tm)
    components = [float(q.w), float(q.x), float(q.y), float(q.z)]
    if components[0] < 0.0:
        components = [-value for value in components]
    _, omega = angular_kinematics(sk, np, tm)
    _, omega_before = angular_kinematics(sk, np, tm - 0.5 / 86_400.0)
    _, omega_after = angular_kinematics(sk, np, tm + 0.5 / 86_400.0)
    alpha = omega_after - omega_before
    return {
        "elapsed_s": seconds,
        "ecef_to_gcrf_quaternion_wxyz": components,
        "angular_velocity_gcrf_rad_s": [float(value) for value in omega],
        "angular_acceleration_gcrf_rad_s2": [float(value) for value in alpha],
        "q30": [qraw(value, 30) for value in components],
        "omega_q24": [qraw(float(value), 24) for value in omega],
        "alpha_q28": [qraw(float(value), 28) for value in alpha],
    }


def build_transform(records: list[dict[str, Any]]) -> bytes:
    if len(records) > MAX_KNOTS:
        raise ValueError("too many knots")
    output = bytearray(KFT_LENGTH)
    pack_header(output, b"KFT10", 2, TRANSFORM_ID)
    struct.pack_into(
        "<IIB3xII",
        output,
        32,
        EARTH_ID,
        60 << 16,
        len(records),
        records[0]["elapsed_s"] << 16,
        records[-1]["elapsed_s"] << 16,
    )
    for index in range(MAX_KNOTS):
        at = KFT_HEADER_LENGTH + index * KNOT_LENGTH
        if index < len(records):
            record = records[index]
            struct.pack_into("<I", output, at, record["elapsed_s"] << 16)
            struct.pack_into("<4i", output, at + 4, *record["q30"])
            struct.pack_into("<3i", output, at + 20, *record["omega_q24"])
            struct.pack_into("<3i", output, at + 32, *record["alpha_q28"])
        else:
            struct.pack_into("<I4i", output, at, 0, Q30, 0, 0, 0)
    seal(output)
    return bytes(output)


def generate() -> tuple[dict[str, Any], bytes, bytes]:
    sk, np = require_satkit()
    epoch = sk.time(2024, 1, 1, 0, 0, 0.0)
    records = [transform_record(sk, np, epoch, second) for second in range(0, 7_201, 60)]
    locations = [
        geodetic_fixture("equator-prime", 0.0, 0.0, 0.0),
        geodetic_fixture("dateline-east", 0.0, 179.999, 0.0),
        geodetic_fixture("dateline-west", 0.0, -179.999, 0.0),
        geodetic_fixture("high-altitude", 45.0, 20.0, 2_000_000.0),
        geodetic_fixture("near-north-pole", 89.999, 45.0, 0.0),
        geodetic_fixture("north-pole", 90.0, 0.0, 0.0),
        geodetic_fixture("near-south-pole", -89.999, -120.0, 0.0),
        geodetic_fixture("south-pole", -90.0, 0.0, 0.0),
    ]
    eop_rows = load_csv(EOP_SOURCE)
    fixture = {
        "schema": "ksa64.phase10.frame-fixtures-v1",
        "generator": {
            "tool": "SatKit",
            "version": "0.16.0",
            "wheel_sha256": "25fe4d6bbfddfc2575e67a64ffc3fa8ff63b2e9c07adfa5dbbd01a3b527f9a89",
            "data_package": "satkit-data 0.9.0",
            "data_wheel_sha256": "a77aed96c99c1f3fc7c03311af79c3b922dc169975630960800eaf9a04af5c48",
            "convention": "IERS 2010 full IAU 2006/2000A ITRF to GCRF",
        },
        "epoch_utc": epoch.as_iso8601(),
        "earth_identity": f"{EARTH_ID:08x}",
        "transform_identity": f"{TRANSFORM_ID:08x}",
        "knot_spacing_s": 60,
        "coverage_s": [0, 7_200],
        "locations": locations,
        "transform_knots": records,
        "time_boundaries": {
            "positive_leap": [
                "2016-12-31T23:59:59Z",
                "2016-12-31T23:59:60Z",
                "2017-01-01T00:00:00Z",
            ],
            "eop_coverage_utc": [eop_rows[0]["utc"], eop_rows[-1]["utc"]],
            "outside_coverage_must_fail": [
                "2023-12-30T23:59:59Z",
                "2024-01-02T00:00:01Z",
            ],
        },
        "tolerances": {
            "rotation_arcsec": 0.05,
            "surface_position_m": 2.0,
            "transport_velocity_m_s": 0.005,
        },
    }
    return fixture, build_earth(), build_transform(records)


def canonical_json(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def validate_checked() -> None:
    fixture = json.loads(FIXTURE_JSON.read_text(encoding="utf-8"))
    assert fixture["schema"] == "ksa64.phase10.frame-fixtures-v1"
    assert fixture["generator"]["version"] == "0.16.0"
    assert len(fixture["transform_knots"]) == 121
    assert len(fixture["locations"]) == 8
    assert EARTH_BIN.stat().st_size == KEM_LENGTH
    assert TRANSFORM_BIN.stat().st_size == KFT_LENGTH
    for path, magic in ((EARTH_BIN, b"KEM10"), (TRANSFORM_BIN, b"KFT10")):
        data = path.read_bytes()
        assert data[:5] == magic
        assert struct.unpack_from("<I", data, len(data) - 4)[0] == zlib.crc32(data[:-4]) & 0xFFFFFFFF


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        validate_checked()
        return
    fixture, earth, transforms = generate()
    GENERATED.mkdir(parents=True, exist_ok=True)
    FIXTURE_JSON.write_bytes(canonical_json(fixture))
    EARTH_BIN.write_bytes(earth)
    TRANSFORM_BIN.write_bytes(transforms)
    provenance = {
        "frame_fixture_sha256": hashlib.sha256(FIXTURE_JSON.read_bytes()).hexdigest(),
        "kem10_sha256": hashlib.sha256(earth).hexdigest(),
        "kft10_sha256": hashlib.sha256(transforms).hexdigest(),
    }
    (GENERATED / "fixture-hashes-v1.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
