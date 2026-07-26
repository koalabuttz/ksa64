#!/usr/bin/env python3
"""Compile the provenance-bearing KSA-G10R source into KGV10/KGM10."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "phase10" / "source-data" / "ksa-g10r-source.json"
OUT = ROOT / "phase10" / "generated"
CONTRACT = 0x10E0_0001
VEHICLE_ID = 0x4756_1001
MISSION_ID = 0x474D_1001
EARTH_ID = 0x454D_1001
TRANSFORM_ID = 0x4654_1001
ATMOSPHERE_ID = 0x4154_1001


def q(value: float, bits: int) -> int:
    scaled = abs(value) * (1 << bits)
    raw = int(math.floor(scaled + 0.5))
    return -raw if value < 0 else raw


def header(length: int, magic: bytes, kind: int, identity: int) -> bytearray:
    out = bytearray(length)
    out[:5] = magic
    struct.pack_into("<HHHIII", out, 6, 10, 64, kind, length, CONTRACT, identity)
    return out


def seal(out: bytearray) -> bytes:
    struct.pack_into("<I", out, len(out) - 4, zlib.crc32(out[:-4]) & 0xFFFFFFFF)
    return bytes(out)


def compile_vehicle(data: dict, source_id: int) -> tuple[bytes, dict]:
    v = data["vehicle"]
    aero = data["aerodynamics"]
    if len(aero) > 16:
        raise ValueError("too many aero knots")
    if abs(v["dry_mass_kg"] + v["main_propellant_kg"] + v["rcs_propellant_kg"] - 500.0) > 1e-9:
        raise ValueError("wet mass must reconstruct to 500 kg")
    out = header(2048, b"KGV10", 4, VEHICLE_ID)
    struct.pack_into("<I", out, 32, source_id)
    out[36] = 5
    area = math.pi * (v["diameter_m"] * 0.5) ** 2
    values = (
        q(v["dry_mass_kg"], 21),
        q(v["main_propellant_kg"], 21),
        q(v["rcs_propellant_kg"], 21),
        q(v["length_m"], 13),
        q(v["diameter_m"], 13),
    )
    struct.pack_into("<5i", out, 40, *values)
    struct.pack_into(
        "<10i",
        out,
        64,
        q(area, 29),
        q(v["wet_cg_from_nose_m"], 28),
        q(v["dry_cg_from_nose_m"], 28),
        *(q(x, 19) for x in v["wet_inertia_kg_m2"]),
        *(q(x, 19) for x in v["dry_inertia_kg_m2"]),
        q(v["thrust_n"], 13),
    )
    struct.pack_into("<iI", out, 104, q(v["main_mass_flow_kg_s"], 21), q(v["burn_time_s"], 16))
    struct.pack_into(
        "<hhBBHii",
        out,
        112,
        q(v["gimbal_limit_deg"] / 360.0, 16),
        q(v["gimbal_slew_deg_s"] / 360.0 / 32.0, 16),
        v["rcs_jet_count"],
        len(aero),
        q(v["rcs_reserve_fraction"], 16),
        q(v["rcs_jet_thrust_n"], 13),
        q(v["rcs_isp_s"], 16),
    )
    struct.pack_into("<ii", out, 128, q(v["drogue_cda_m2"], 24), q(v["main_cda_m2"], 24))
    for i, knot in enumerate(aero):
        mach, cd, cp, cn, pitch_damp, yaw_damp = knot
        struct.pack_into(
            "<6i",
            out,
            256 + i * 24,
            q(mach, 24),
            q(cd, 24),
            q(cp, 28),
            q(cn, 24),
            q(pitch_damp, 24),
            q(yaw_damp, 24),
        )
    evidence = {
        "identity": f"0x{VEHICLE_ID:08x}",
        "source_identity": f"0x{source_id:08x}",
        "wet_mass_kg": 500.0,
        "reference_area_m2": area,
        "aero_knots": len(aero),
        "provenance": data["provenance_policy"],
    }
    return seal(out), evidence


def compile_mission(data: dict, source_id: int) -> tuple[bytes, dict]:
    m = data["mission"]
    pitch = m["pitch_schedule"]
    out = header(1024, b"KGM10", 5, MISSION_ID)
    struct.pack_into("<I", out, 32, source_id)
    out[36:40] = bytes((5, 1, 2, 3))
    struct.pack_into("<4I", out, 40, EARTH_ID, TRANSFORM_ID, ATMOSPHERE_ID, VEHICLE_ID)
    values = (
        q(math.radians(m["launch_latitude_deg"]), 28),
        q(math.radians(m["launch_longitude_deg"]), 28),
        q(m["launch_height_km"], 12),
        q(math.radians(m["launch_azimuth_deg"]), 28),
        q(math.radians(m["launch_elevation_deg"]), 28),
        q(m["rail_length_m"], 13),
        q(math.radians(m["recovery_latitude_deg"]), 28),
        q(math.radians(m["recovery_longitude_deg"]), 28),
        q(m["recovery_height_km"], 12),
        q(m["eci_transition_altitude_km"], 12),
        q(m["entry_transition_altitude_km"], 12),
        q(m["recovery_transition_altitude_km"], 12),
        q(m["recovery_radius_km"], 12),
        q(m["transition_dynamic_pressure_pa"], 14),
        q(m["recovery_transition_mach"], 24),
        q(m["main_deployment_altitude_km"], 12),
    )
    struct.pack_into("<16i", out, 64, *values)
    struct.pack_into("<I", out, 128, q(m["max_mission_time_s"], 16))
    out[132] = len(pitch)
    for i, (time_s, elevation_deg) in enumerate(pitch):
        struct.pack_into("<Ii", out, 256 + i * 8, q(time_s, 16), q(math.radians(elevation_deg), 28))
    evidence = {
        "identity": f"0x{MISSION_ID:08x}",
        "launch": [m["launch_latitude_deg"], m["launch_longitude_deg"], m["launch_height_km"]],
        "recovery_anchor": [
            m["recovery_latitude_deg"],
            m["recovery_longitude_deg"],
            m["recovery_height_km"],
        ],
        "pitch_knots": len(pitch),
        "transition_altitudes_km": [120.0, 120.0, 20.0],
    }
    return seal(out), evidence


def render() -> dict[Path, bytes]:
    raw = SOURCE.read_bytes()
    data = json.loads(raw)
    if data["schema"] != "ksa64-global-vehicle-source-v1" or data["profile"] != "GlobalEcef6DofV1":
        raise ValueError("schema/profile mismatch")
    if not data["provenance_policy"].strip() or not data["source_note"].strip():
        raise ValueError("missing provenance")
    source_id = zlib.crc32(raw) & 0xFFFFFFFF
    vehicle, vehicle_evidence = compile_vehicle(data, source_id)
    mission, mission_evidence = compile_mission(data, source_id)
    evidence = {
        "format": "KSA64 Phase 10 compiled vehicle evidence v1",
        "source_sha256": hashlib.sha256(raw).hexdigest(),
        "source_identity": f"0x{source_id:08x}",
        "vehicle": vehicle_evidence,
        "mission": mission_evidence,
    }
    evidence_bytes = (json.dumps(evidence, indent=2, sort_keys=True) + "\n").encode()
    hashes = {
        "ksa-g10r.kgv10": hashlib.sha256(vehicle).hexdigest(),
        "ksa-g10r.kgm10": hashlib.sha256(mission).hexdigest(),
        "vehicle-evidence-v1.json": hashlib.sha256(evidence_bytes).hexdigest(),
    }
    return {
        OUT / "ksa-g10r.kgv10": vehicle,
        OUT / "ksa-g10r.kgm10": mission,
        OUT / "vehicle-evidence-v1.json": evidence_bytes,
        OUT / "vehicle-hashes-v1.json": (json.dumps(hashes, indent=2, sort_keys=True) + "\n").encode(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stale = []
    for path, content in render().items():
        if args.check:
            if not path.exists() or path.read_bytes() != content:
                stale.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    if stale:
        print("stale: " + ", ".join(stale))
        return 1
    print("phase10 vehicle compiler: " + ("PASS" if args.check else "generated"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
