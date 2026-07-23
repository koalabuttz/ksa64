#!/usr/bin/env python3
"""Independent float64 audit of frozen Phase 3 KST3 mission records."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys
import zlib
from pathlib import Path

HEADER = 64
FRAME = 160
DT = 0.125
EARTH_RADIUS = 6378.137
MU = 398600.4418
EVENT_CUTOFF = 1 << 1
EVENT_END = 1 << 4
EVENT_ABORT = 1 << 7
CASES = ("nominal", "altimeter-dropout", "gps-outage", "steering-stuck")


def crc32(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFF_FFFF


def load_stream(path: Path) -> tuple[dict, list[dict]]:
    data = path.read_bytes()
    if len(data) < HEADER or (len(data) - HEADER) % FRAME:
        raise ValueError(f"bad KST3 framing: {path}")
    if data[:4] != b"KST3" or struct.unpack_from("<H", data, 4)[0] != 3:
        raise ValueError(f"bad KST3 identity: {path}")
    if crc32(data[:60]) != struct.unpack_from("<I", data, 60)[0]:
        raise ValueError(f"bad KST3 header CRC: {path}")
    header = {
        "scenario_id": struct.unpack_from("<I", data, 16)[0],
        "scenario_crc32": struct.unpack_from("<I", data, 20)[0],
        "config_crc32": struct.unpack_from("<I", data, 24)[0],
        "seed": struct.unpack_from("<I", data, 28)[0],
        "case": data[32],
        "timestep_s": struct.unpack_from("<i", data, 36)[0] / 65536.0,
        "mission_steps": struct.unpack_from("<I", data, 40)[0],
        "stream_crc32": f"0x{crc32(data):08x}",
        "sha256": hashlib.sha256(data).hexdigest(),
    }
    frames = []
    for index, at in enumerate(range(HEADER, len(data), FRAME)):
        raw = data[at : at + FRAME]
        if crc32(raw[:156]) != struct.unpack_from("<I", raw, 156)[0]:
            raise ValueError(f"bad KST3 frame CRC at {index}: {path}")
        frame = {
            "step": struct.unpack_from("<I", raw, 0)[0],
            "time": struct.unpack_from("<i", raw, 4)[0] / 65536.0,
            "r": struct.unpack_from("<i", raw, 8)[0] / 4096.0,
            "downrange": struct.unpack_from("<i", raw, 12)[0] / 2**32,
            "vr": struct.unpack_from("<i", raw, 16)[0] / 2**24,
            "h": struct.unpack_from("<i", raw, 20)[0] / 2**14,
            "ar": struct.unpack_from("<i", raw, 24)[0] / 2**28,
            "at": struct.unpack_from("<i", raw, 28)[0] / 2**28,
            "pitch": struct.unpack_from("<H", raw, 42)[0],
            "requested": struct.unpack_from("<H", raw, 44)[0],
            "validity": struct.unpack_from("<H", raw, 46)[0],
            "q_pa": struct.unpack_from("<i", raw, 52)[0] / 65536.0,
            "events": struct.unpack_from("<H", raw, 56)[0],
            "alarms": struct.unpack_from("<H", raw, 58)[0],
            "stage": raw[60],
            "mode": raw[63],
            "nav_r": struct.unpack_from("<i", raw, 108)[0] / 4096.0,
            "nav_downrange": struct.unpack_from("<i", raw, 112)[0] / 2**32,
            "nav_vr": struct.unpack_from("<i", raw, 116)[0] / 2**24,
            "nav_vt": struct.unpack_from("<i", raw, 120)[0] / 2**24,
            "truth_checksum": f"0x{struct.unpack_from('<I', raw, 128)[0]:08x}",
            "sensor_checksum": f"0x{struct.unpack_from('<I', raw, 132)[0]:08x}",
            "nav_checksum": f"0x{struct.unpack_from('<I', raw, 136)[0]:08x}",
            "flight_checksum": f"0x{struct.unpack_from('<I', raw, 140)[0]:08x}",
            "terminal": bool(struct.unpack_from("<H", raw, 148)[0] & 1),
        }
        if abs(frame["time"] - frame["step"] * header["timestep_s"]) > 1e-9:
            raise ValueError(f"mission clock divergence at frame {index}: {path}")
        frames.append(frame)
    if not frames[-1]["terminal"] or not frames[-1]["events"] & EVENT_END:
        raise ValueError(f"missing KST3 terminal: {path}")
    return header, frames


def orbit(state: dict) -> dict:
    r, vr, h = state["r"], state["vr"], state["h"]
    vt = h / r
    energy = 0.5 * (vr * vr + vt * vt) - MU / r
    e_cos = vt * h / MU - 1.0
    e_sin = vr * h / MU
    eccentricity = math.hypot(e_cos, e_sin)
    semi_major = -MU / (2.0 * energy)
    return {
        "eccentricity": eccentricity,
        "perigee_km": semi_major * (1.0 - eccentricity) - EARTH_RADIUS,
        "apogee_km": semi_major * (1.0 + eccentricity) - EARTH_RADIUS,
    }


def proper_acceleration(frame: dict) -> float:
    vacuum_ar = frame["h"] ** 2 / frame["r"] ** 3 - MU / frame["r"] ** 2
    return math.hypot(frame["ar"] - vacuum_ar, frame["at"]) * 1000.0


def navigation_error(frame: dict) -> tuple[float, float]:
    dr = frame["nav_r"] - frame["r"]
    turns = frame["nav_downrange"] - frame["downrange"]
    turns -= round(turns)
    arc = turns * 2.0 * math.pi * frame["r"]
    truth_vt = frame["h"] / frame["r"]
    return math.hypot(dr, arc), math.hypot(frame["nav_vr"] - frame["vr"], frame["nav_vt"] - truth_vt)


def coast_reference(cutoff: dict, terminal_step: int) -> dict:
    r, vr, h = cutoff["r"], cutoff["vr"], cutoff["h"]
    for _ in range(cutoff["step"], terminal_step):
        ar = h * h / (r * r * r) - MU / (r * r)
        vr += ar * DT
        r += vr * DT
    return {"radius_km": r, "radial_velocity_km_s": vr}


def analyze(header: dict, frames: list[dict]) -> dict:
    terminal = frames[-1]
    max_q = max(frame["q_pa"] for frame in frames)
    max_proper = max(proper_acceleration(frame) for frame in frames)
    cutoff_frames = [frame for frame in frames if frame["events"] & EVENT_CUTOFF and frame["stage"] == 1]
    cutoff = cutoff_frames[-1] if cutoff_frames else None
    result = {
        "header": header,
        "frame_count": len(frames),
        "terminal_step": terminal["step"],
        "terminal_altitude_km": terminal["r"] - EARTH_RADIUS,
        "max_sampled_dynamic_pressure_kpa": max_q,
        "max_sampled_proper_acceleration_m_s2": max_proper,
        "terminal_checksums": {name: terminal[name] for name in ("truth_checksum", "sensor_checksum", "nav_checksum", "flight_checksum")},
        "abort": bool(terminal["events"] & EVENT_ABORT),
    }
    if cutoff is not None:
        position_error, velocity_error = navigation_error(cutoff)
        coast = coast_reference(cutoff, terminal["step"])
        result["cutoff"] = {
            "step": cutoff["step"],
            "navigation_position_error_km": position_error,
            "navigation_velocity_error_km_s": velocity_error,
        }
        result["independent_float64_coast"] = {
            **coast,
            "terminal_radius_difference_km": terminal["r"] - coast["radius_km"],
            "terminal_radial_velocity_difference_km_s": terminal["vr"] - coast["radial_velocity_km_s"],
        }
    if not result["abort"]:
        result["terminal_orbit"] = orbit(terminal)
    outage = [frame for frame in frames if 2080 <= frame["step"] <= 2560]
    if header["case"] == 2:
        errors = [navigation_error(frame) for frame in outage]
        result["gps_outage_bridge"] = {
            "max_position_error_km": max(x[0] for x in errors),
            "max_velocity_error_km_s": max(x[1] for x in errors),
        }
    return result


def build(root: Path) -> dict:
    cases = {}
    for name in CASES:
        header, frames = load_stream(root / f"phase3/examples/ksa3-{name}.kst3")
        cases[name] = analyze(header, frames)
    acceptance = {
        "orbit_180_to_220_km": all(
            180.0 <= cases[name]["terminal_orbit"][key] <= 220.0
            for name in ("nominal", "altimeter-dropout", "gps-outage")
            for key in ("perigee_km", "apogee_km")
        ),
        "eccentricity_at_most_0_01": all(cases[name]["terminal_orbit"]["eccentricity"] <= 0.01 for name in ("nominal", "altimeter-dropout", "gps-outage")),
        "max_q_at_most_60_kpa": all(cases[name]["max_sampled_dynamic_pressure_kpa"] <= 60.0 for name in ("nominal", "altimeter-dropout", "gps-outage")),
        "max_acceleration_at_most_60_m_s2": all(cases[name]["max_sampled_proper_acceleration_m_s2"] <= 60.0 for name in ("nominal", "altimeter-dropout", "gps-outage")),
        "cutoff_navigation_within_1_km_and_10_m_s": all(
            cases[name]["cutoff"]["navigation_position_error_km"] <= 1.0
            and cases[name]["cutoff"]["navigation_velocity_error_km_s"] <= 0.010
            for name in ("nominal", "altimeter-dropout", "gps-outage")
        ),
        "gps_bridge_within_5_km_and_30_m_s": cases["gps-outage"]["gps_outage_bridge"]["max_position_error_km"] <= 5.0 and cases["gps-outage"]["gps_outage_bridge"]["max_velocity_error_km_s"] <= 0.030,
        "stuck_case_aborts": cases["steering-stuck"]["abort"],
        "float64_coast_agrees_with_fixed_point_terminal": all(
            abs(cases[name]["independent_float64_coast"]["terminal_radius_difference_km"]) <= 0.100
            and abs(cases[name]["independent_float64_coast"]["terminal_radial_velocity_difference_km_s"]) <= 0.001
            for name in ("nominal", "altimeter-dropout", "gps-outage")
        ),
    }
    return {
        "model": "independent Python float64 orbital, coast, load, navigation, and binary audit of KST3",
        "constants": {"earth_radius_km": EARTH_RADIUS, "mu_km3_s2": MU, "timestep_s": DT},
        "cases": cases,
        "acceptance": acceptance,
        "all_acceptance_passed": all(acceptance.values()),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = root / "phase3/mission-reference-v1.json"
    sha = output.with_name(output.name + ".sha256")
    data = (json.dumps(build(root), indent=2) + "\n").encode()
    digest = (hashlib.sha256(data).hexdigest() + "  mission-reference-v1.json\n").encode()
    if args.check:
        if not output.exists() or output.read_bytes() != data or not sha.exists() or sha.read_bytes() != digest:
            print("Phase 3 reference evidence is stale", file=sys.stderr)
            return 1
        print("Phase 3 independent reference evidence is current")
        return 0
    output.write_bytes(data)
    sha.write_bytes(digest)
    print(output.relative_to(root))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())