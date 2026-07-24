#!/usr/bin/env python3
"""Independently validate and summarize a Phase 7 KSC7/KRA7 campaign."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import zlib
from pathlib import Path

KSC_LENGTH = 512
KRA_HEADER_LENGTH = 64
KSR_LENGTH = 192
ARCHIVE_RECORD_LENGTH = 8 + KSR_LENGTH
NUMERIC_ID = 0xEE0448FA
ENVIRONMENT_ID = 0x42B15A63
CATALOG_ID = 0x07000001
PARAMETER_COUNT = 8
FNV_OFFSET = 2_166_136_261
FNV_PRIME = 16_777_619


def u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def i32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def crc(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFFFFFF


def fnv(data: bytes) -> int:
    value = FNV_OFFSET
    for byte in data:
        value = ((value ^ byte) * FNV_PRIME) & 0xFFFFFFFF
    return value


def validate_record(data: bytes, magic: bytes, kind: int, fixed: int | None) -> int:
    if fixed is not None and len(data) != fixed:
        raise ValueError(f"{magic.decode()} length {len(data)} != {fixed}")
    if len(data) < 36 or data[:4] != magic:
        raise ValueError(f"bad {magic.decode()} framing")
    if u16(data, 4) != 7 or u16(data, 6) != 32 or u16(data, 8) != len(data):
        raise ValueError(f"bad {magic.decode()} common header")
    if u16(data, 10) != kind or u32(data, 12) != NUMERIC_ID:
        raise ValueError(f"bad {magic.decode()} identity")
    if any(data[20:32]):
        raise ValueError(f"nonzero {magic.decode()} reserved bytes")
    if u32(data, len(data) - 4) != crc(data[:-4]):
        raise ValueError(f"bad {magic.decode()} CRC")
    return u32(data, 16)


def mix32(value: int) -> int:
    value &= 0xFFFFFFFF
    value ^= value >> 16
    value = (value * 0x7FEB352D) & 0xFFFFFFFF
    value ^= value >> 15
    value = (value * 0x846CA68B) & 0xFFFFFFFF
    return (value ^ (value >> 16)) & 0xFFFFFFFF


def keyed_word(seed: int, run: int, parameter: int) -> int:
    return mix32(
        seed
        ^ ((run * 0x9E3779B9) & 0xFFFFFFFF)
        ^ ((parameter * 0x85EBCA6B) & 0xFFFFFFFF)
    )


def variation(seed: int, run: int, ranges: list[tuple[int, int]]) -> tuple[list[int], int]:
    if run == 0:
        return [0] * len(ranges), 0
    values = []
    for parameter, (minimum, maximum) in enumerate(ranges):
        span = maximum - minimum + 1
        values.append(minimum + ((keyed_word(seed, run, parameter) * span) >> 32))
    payload = struct.pack("<8i", *values)
    return values, crc(payload)


def materialized_identities(vehicle: bytes, motor: bytes, mission: bytes) -> list[int]:
    vehicle_id = u32(vehicle, 16)
    motor_id = u32(motor, 16)
    mission_id = u32(mission, 16)
    if u32(mission, 32) != vehicle_id or u32(mission, 36) != motor_id:
        raise ValueError("base pack identity mismatch")
    if u32(mission, 40) != ENVIRONMENT_ID:
        raise ValueError("base environment identity mismatch")
    design = struct.pack("<IIII", vehicle_id, mission_id, 1_000_000, 1_000_000)
    designed_vehicle = fnv(design)
    designed_mission = fnv(design + struct.pack("<ii", 0, 0))
    return [NUMERIC_ID, ENVIRONMENT_ID, designed_vehicle, motor_id, designed_mission]


def parse_config(data: bytes) -> tuple[int, int, list[tuple[int, int]]]:
    identity = validate_record(data, b"KSC7", 6, KSC_LENGTH)
    if identity != CATALOG_ID or u32(data, 40) != CATALOG_ID:
        raise ValueError("unknown KSC7 catalog")
    seed, runs = u32(data, 32), u32(data, 36)
    if seed == 0 or runs == 0 or runs > 65_535 or data[44] != PARAMETER_COUNT:
        raise ValueError("invalid KSC7 campaign")
    if any(data[45:48]) or any(data[48 + PARAMETER_COUNT * 12 : -4]):
        raise ValueError("nonzero KSC7 reserved bytes")
    ranges = []
    for parameter in range(PARAMETER_COUNT):
        offset = 48 + parameter * 12
        if data[offset] != parameter or data[offset + 1] != 1 or any(data[offset + 2 : offset + 4]):
            raise ValueError(f"invalid KSC7 parameter {parameter}")
        minimum, maximum = i32(data, offset + 4), i32(data, offset + 8)
        if minimum > maximum:
            raise ValueError(f"reversed KSC7 parameter {parameter}")
        ranges.append((minimum, maximum))
    return seed, runs, ranges


def analyze(ksc: bytes, kra: bytes, identities: list[int]) -> dict[str, object]:
    seed, runs, ranges = parse_config(ksc)
    if len(kra) != KRA_HEADER_LENGTH + runs * ARCHIVE_RECORD_LENGTH:
        raise ValueError("KRA7 archive length mismatch")
    header = kra[:KRA_HEADER_LENGTH]
    archive_identity = validate_record(header, b"KRA7", 9, KRA_HEADER_LENGTH)
    if u32(header, 32) != seed or u32(header, 36) != runs:
        raise ValueError("KRA7 campaign mismatch")
    if u32(header, 40) != ARCHIVE_RECORD_LENGTH or u32(header, 44) != archive_identity:
        raise ValueError("KRA7 record contract mismatch")
    summaries = bytearray()
    apogees: list[int] = []
    impacts: list[int] = []
    outcomes: dict[int, int] = {}
    for run in range(runs):
        offset = KRA_HEADER_LENGTH + run * ARCHIVE_RECORD_LENGTH
        if u32(kra, offset) != run or u32(kra, offset + 4) != KSR_LENGTH:
            raise ValueError(f"KRA7 ordering failure at run {run}")
        record = kra[offset + 8 : offset + 8 + KSR_LENGTH]
        input_identity = validate_record(record, b"KSR7", 5, KSR_LENGTH)
        if record[32] != 3 or record[34] != 0 or record[35] != 0:
            raise ValueError(f"invalid KSR7 profile/fault at run {run}")
        _, variation_crc = variation(seed, run, ranges)
        expected_identity = fnv(struct.pack("<6I", *identities, variation_crc))
        if input_identity != expected_identity:
            raise ValueError(f"variation identity mismatch at run {run}")
        outcome = record[33]
        outcomes[outcome] = outcomes.get(outcome, 0) + 1
        validity = u32(record, 40)
        if validity & (1 << 1) == 0 or validity & (1 << 21) == 0:
            raise ValueError(f"missing KSR7 campaign metrics at run {run}")
        apogees.append(i32(record, 68 + 4))
        impacts.append(i32(record, 68 + 21 * 4))
        summaries.extend(record)
    ordered_crc = crc(summaries)
    if ordered_crc != archive_identity:
        raise ValueError("KRA7 ordered summary CRC mismatch")
    return {
        "schema": "ksa64.phase7.campaign-analysis-v1",
        "master_seed": f"{seed:08x}",
        "run_count": runs,
        "parameter_count": len(ranges),
        "variation_identities_exact": True,
        "outcomes": {str(key): value for key, value in sorted(outcomes.items())},
        "aggregate": {
            "minimum_apogee_raw_q13": min(apogees),
            "maximum_apogee_raw_q13": max(apogees),
            "mean_apogee_raw_q13": sum(apogees) // len(apogees),
            "minimum_impact_velocity_raw_q19": min(impacts),
            "maximum_impact_velocity_raw_q19": max(impacts),
            "ordered_records_crc32": f"{ordered_crc:08x}",
        },
        "si_envelope": {
            "minimum_apogee_m": min(apogees) / (1 << 13),
            "maximum_apogee_m": max(apogees) / (1 << 13),
            "minimum_impact_velocity_mps": min(impacts) / (1 << 19),
            "maximum_impact_velocity_mps": max(impacts) / (1 << 19),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ksc", type=Path, required=True)
    parser.add_argument("--kra", type=Path, required=True)
    parser.add_argument("--vehicle", type=Path, required=True)
    parser.add_argument("--motor", type=Path, required=True)
    parser.add_argument("--mission", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    ksc, kra = args.ksc.read_bytes(), args.kra.read_bytes()
    vehicle, motor, mission = (
        args.vehicle.read_bytes(),
        args.motor.read_bytes(),
        args.mission.read_bytes(),
    )
    for data, magic, kind, length in (
        (vehicle, b"KVP7", 1, 512),
        (motor, b"KMP7", 2, 896),
        (mission, b"KMC7", 3, 256),
    ):
        validate_record(data, magic, kind, length)
    data = analyze(ksc, kra, materialized_identities(vehicle, motor, mission))
    data["artifacts"] = {
        "ksc7_sha256": hashlib.sha256(ksc).hexdigest(),
        "kra7_sha256": hashlib.sha256(kra).hexdigest(),
    }
    text = json.dumps(data, indent=2) + "\n"
    print(text, end="")
    if args.check:
        if args.output is None:
            raise ValueError("--check requires --output")
        if json.loads(args.output.read_text()) != data:
            raise ValueError(f"analysis differs from {args.output}")
    elif args.output is not None:
        args.output.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
