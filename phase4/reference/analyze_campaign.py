#!/usr/bin/env python3
"""Independent KSC4/KSR4 verification and float64 campaign analysis."""

import argparse
import hashlib
import json
import math
import struct
import sys
import zlib
from pathlib import Path

KSC4_LENGTH = 512
KSR4_LENGTH = 128
PARAMETER_COUNT = 15
EARTH_RADIUS_KM = 26_124_849 / 4096.0
EARTH_MU_KM3_S2 = 1_632_667_410 / 4096.0
ATMOSPHERE_TOP_KM = 100.0
MASK = 0xFFFF_FFFF


def u32(value):
    return value & MASK


def mix32(value):
    value = u32(value ^ (value >> 16))
    value = u32(value * 0x7FEB_352D)
    value = u32(value ^ (value >> 15))
    value = u32(value * 0x846C_A68B)
    return u32(value ^ (value >> 16))


def keyed_word(master, run, parameter, group, draw):
    source = parameter if group == 0 else 0x100 + group
    return mix32(master ^ u32(run * 0x9E37_79B9) ^ u32(source * 0x85EB_CA6B) ^ u32(draw * 0xC2B2_AE35))


def trunc_div(numerator, denominator):
    if denominator <= 0:
        raise ValueError("nonpositive divisor")
    return numerator // denominator if numerator >= 0 else -((-numerator) // denominator)


def uniform(word, minimum, maximum):
    if minimum == maximum:
        return minimum
    span = maximum - minimum + 1
    return minimum + ((word * span) >> 32)


def sample(spec, master, run):
    parameter, kind, group, minimum, baseline, maximum, shape = spec
    if run == 0 or kind == 0:
        return baseline
    word = lambda draw: keyed_word(master, run, parameter, group, draw)
    if kind == 1:
        return uniform(word(0), minimum, maximum)
    if kind == 2:
        return trunc_div(uniform(word(0), minimum, maximum) + uniform(word(1), minimum, maximum), 2)
    if kind == 3:
        return maximum if ((word(0) * 1_000_000) >> 32) < shape else minimum
    if kind == 4:
        centered = sum(word(draw) & 0xFF for draw in range(12)) - 1530
        span = maximum - baseline if centered >= 0 else baseline - minimum
        delta = trunc_div(centered * span, 768)
        return max(minimum, min(maximum, baseline + delta))
    raise ValueError(f"unknown distribution {kind}")


def parse_ksc4(data):
    if len(data) != KSC4_LENGTH or data[:4] != b"KSC4":
        raise ValueError("invalid KSC4 framing")
    version, length = struct.unpack_from("<HH", data, 4)
    if version != 4 or length != KSC4_LENGTH or struct.unpack_from("<I", data, 8)[0] != 0x0400_0001:
        raise ValueError("invalid KSC4 contract")
    if any(data[33:120]):
        raise ValueError("nonzero KSC4 reserved bytes")
    if zlib.crc32(data[128:]) != struct.unpack_from("<I", data, 120)[0]:
        raise ValueError("KSC4 record-region CRC")
    if zlib.crc32(data[:124]) != struct.unpack_from("<I", data, 124)[0]:
        raise ValueError("KSC4 header CRC")
    scenario_id, base_crc, phase3_crc, master, run_count = struct.unpack_from("<IIIII", data, 12)
    count = data[32]
    if count > 16:
        raise ValueError("KSC4 distribution count")
    specs = []
    for index in range(16):
        at = 128 + index * 24
        record = data[at:at + 24]
        if index >= count:
            if any(record):
                raise ValueError("nonzero unused KSC4 record")
            continue
        if record[3] != 0 or zlib.crc32(record[:20]) != struct.unpack_from("<I", record, 20)[0]:
            raise ValueError(f"KSC4 distribution record {index}")
        parameter, kind, group = record[:3]
        minimum, baseline, maximum, shape = struct.unpack_from("<iiii", record, 4)
        specs.append((parameter, kind, group, minimum, baseline, maximum, shape))
    canonical = bytearray(data)
    canonical[120:128] = b"\0" * 8
    return {
        "scenario_id": scenario_id,
        "base_crc32": base_crc,
        "phase3_crc32": phase3_crc,
        "master_seed": master,
        "run_count": run_count,
        "identity": zlib.crc32(canonical),
        "specs": specs,
    }


def derive_run(config, index):
    values = [0] * PARAMETER_COUNT
    for spec in config["specs"]:
        values[spec[0]] = sample(spec, config["master_seed"], index) - spec[4]
    if index == 0:
        seed = 0x4B53_4133
    else:
        seed = mix32(config["master_seed"] ^ u32(index * 0xD1B5_4A35) ^ 0x5345_4544)
        if seed == 0:
            seed = 0x6D2B_79F5
    payload = struct.pack("<II15i", index, seed, *values)
    return seed, values, zlib.crc32(payload)


def parse_ksr4(record, expected_index, config):
    if len(record) != KSR4_LENGTH or record[:4] != b"KSR4":
        raise ValueError(f"KSR4 framing at run {expected_index}")
    if struct.unpack_from("<HHI", record, 4) != (4, KSR4_LENGTH, 0x0400_0001):
        raise ValueError(f"KSR4 contract at run {expected_index}")
    if record[35] or any(record[114:124]):
        raise ValueError(f"KSR4 reserved bytes at run {expected_index}")
    if zlib.crc32(record[:124]) != struct.unpack_from("<I", record, 124)[0]:
        raise ValueError(f"KSR4 CRC at run {expected_index}")
    campaign, scenario, index, seed, variation = struct.unpack_from("<IIIII", record, 12)
    if campaign != config["identity"] or scenario != config["scenario_id"] or index != expected_index:
        raise ValueError(f"KSR4 identity at run {expected_index}")
    derived_seed, _, derived_variation = derive_run(config, index)
    if seed != derived_seed or variation != derived_variation:
        raise ValueError(f"KSR4 variation at run {expected_index}")
    return {
        "outcome": record[32],
        "cutoff_radius_km": struct.unpack_from("<i", record, 64)[0] / 4096.0,
        "cutoff_radial_velocity_km_s": struct.unpack_from("<i", record, 72)[0] / 16_777_216.0,
        "cutoff_angular_momentum_km2_s": struct.unpack_from("<i", record, 76)[0] / 16_384.0,
        "max_dynamic_pressure_kpa": struct.unpack_from("<i", record, 80)[0] / 65_536.0,
        "max_proper_acceleration_m_s2": struct.unpack_from("<i", record, 84)[0] * 1000.0 / 268_435_456.0,
        "navigation_position_error_m": struct.unpack_from("<i", record, 88)[0] * 1000.0 / 4096.0,
        "navigation_velocity_error_m_s": struct.unpack_from("<i", record, 92)[0] * 1000.0 / 16_777_216.0,
        "checksums": list(struct.unpack_from("<IIII", record, 96)),
    }


def orbital_result(run):
    radius = run["cutoff_radius_km"]
    radial = run["cutoff_radial_velocity_km_s"]
    angular_momentum = run["cutoff_angular_momentum_km2_s"]
    tangential = angular_momentum / radius
    energy = 0.5 * (radial * radial + tangential * tangential) - EARTH_MU_KM3_S2 / radius
    if energy >= 0:
        return {"class": "escape", "perigee_altitude_km": radius - EARTH_RADIUS_KM, "apogee_altitude_km": None}
    eccentricity = math.sqrt(max(0.0, 1.0 + 2.0 * energy * angular_momentum * angular_momentum / (EARTH_MU_KM3_S2 ** 2)))
    semi_major = -EARTH_MU_KM3_S2 / (2.0 * energy)
    perigee = semi_major * (1.0 - eccentricity) - EARTH_RADIUS_KM
    apogee = semi_major * (1.0 + eccentricity) - EARTH_RADIUS_KM
    classification = "impact" if perigee <= 0.0 else "suborbital" if perigee < ATMOSPHERE_TOP_KM else "stable"
    return {"class": classification, "perigee_altitude_km": perigee, "apogee_altitude_km": apogee}


def stats(values):
    return {
        "minimum": min(values),
        "maximum": max(values),
        "mean": math.fsum(values) / len(values),
    }


def analyze(ksc_path, ksr_path):
    ksc = ksc_path.read_bytes()
    ksr = ksr_path.read_bytes()
    config = parse_ksc4(ksc)
    if len(ksr) != config["run_count"] * KSR4_LENGTH:
        raise ValueError("KSR4 stream length")
    runs = [parse_ksr4(ksr[index * KSR4_LENGTH:(index + 1) * KSR4_LENGTH], index, config) for index in range(config["run_count"])]
    baseline = runs[0]["checksums"]
    if baseline != [0xC86045A0, 0x47D11FB0, 0xC6F9DA7B, 0x02CE28EF]:
        raise ValueError("run-zero Phase 3 checksum identity")
    orbits = [orbital_result(run) for run in runs]
    orbit_counts = {name: sum(item["class"] == name for item in orbits) for name in ("stable", "suborbital", "impact", "escape")}
    finite_apogees = [item["apogee_altitude_km"] for item in orbits if item["apogee_altitude_km"] is not None]
    return {
        "schema": "KSA64 phase4 independent campaign analysis v1",
        "inputs": {
            "ksc4_sha256": hashlib.sha256(ksc).hexdigest(),
            "ksr4_sha256": hashlib.sha256(ksr).hexdigest(),
            "campaign_identity": f"0x{config['identity']:08x}",
            "master_seed": f"0x{config['master_seed']:08x}",
            "run_count": config["run_count"],
        },
        "variation_reconstruction": {"verified_runs": len(runs), "status": "exact"},
        "run_zero_checksums": [f"0x{value:08x}" for value in baseline],
        "reported_outcomes_non_authoritative": [sum(run["outcome"] == value for run in runs) for value in range(6)],
        "float64_orbits_authoritative": {
            "counts": orbit_counts,
            "perigee_altitude_km": stats([item["perigee_altitude_km"] for item in orbits]),
            "apogee_altitude_km": stats(finite_apogees),
        },
        "loads": {
            "maximum_dynamic_pressure_kpa": stats([run["max_dynamic_pressure_kpa"] for run in runs]),
            "maximum_proper_acceleration_m_s2": stats([run["max_proper_acceleration_m_s2"] for run in runs]),
        },
        "navigation": {
            "position_error_m": stats([run["navigation_position_error_m"] for run in runs]),
            "velocity_error_m_s": stats([run["navigation_velocity_error_m_s"] for run in runs]),
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ksc", type=Path, required=True)
    parser.add_argument("--ksr", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        payload = (json.dumps(analyze(args.ksc, args.ksr), indent=2, sort_keys=True) + "\n").encode()
    except (OSError, ValueError) as error:
        print(f"analysis failed: {error}", file=sys.stderr)
        return 1
    digest = (hashlib.sha256(payload).hexdigest() + "  " + args.output.name + "\n").encode()
    sidecar = args.output.with_name(args.output.name + ".sha256")
    if args.check:
        if not args.output.exists() or args.output.read_bytes() != payload or not sidecar.exists() or sidecar.read_bytes() != digest:
            print("Phase 4 campaign analysis is stale", file=sys.stderr)
            return 1
        print("Phase 4 campaign analysis is current")
        return 0
    args.output.write_bytes(payload)
    sidecar.write_bytes(digest)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())