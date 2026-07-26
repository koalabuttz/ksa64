#!/usr/bin/env python3
"""Independent float64 KSA-G10R trajectory reference."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "phase10" / "source-data" / "ksa-g10r-source.json"
ATMOSPHERE = ROOT / "phase10" / "generated" / "atmosphere-fixtures-v1.json"
OUT = ROOT / "phase10" / "generated" / "nominal-float64-v1.json"

A = 6378.137
B = 6356.752314245
E2 = 1.0 - (B / A) ** 2
MU = 398600.4418
J2 = 1.08262668e-3
OMEGA = 7.2921150e-5


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def scale(a, s):
    return tuple(x * s for x in a)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def norm(a):
    return math.sqrt(dot(a, a))


def cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def geodetic_to_ecef(lat, lon, h_km):
    n = A / math.sqrt(1.0 - E2 * math.sin(lat) ** 2)
    return (
        (n + h_km) * math.cos(lat) * math.cos(lon),
        (n + h_km) * math.cos(lat) * math.sin(lon),
        (n * (1.0 - E2) + h_km) * math.sin(lat),
    )


def ecef_to_geodetic(p):
    x, y, z = p
    lon = math.atan2(y, x)
    r = math.hypot(x, y)
    lat = math.atan2(z, r * (1.0 - E2))
    h = 0.0
    for _ in range(8):
        n = A / math.sqrt(1.0 - E2 * math.sin(lat) ** 2)
        h = r / max(math.cos(lat), 1e-15) - n
        lat = math.atan2(z, r * (1.0 - E2 * n / (n + h)))
    return lat, lon, h


def enu_basis(lat, lon):
    return (
        (-math.sin(lon), math.cos(lon), 0.0),
        (-math.sin(lat) * math.cos(lon), -math.sin(lat) * math.sin(lon), math.cos(lat)),
        (math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat)),
    )


def interpolate(x, table, key):
    if x <= table[0]["altitude_km"]:
        return table[0][key]
    if x >= table[-1]["altitude_km"]:
        return 0.0 if key in ("density_kg_m3", "pressure_pa") else table[-1][key]
    for lo, hi in zip(table, table[1:]):
        if x <= hi["altitude_km"]:
            f = (x - lo["altitude_km"]) / (hi["altitude_km"] - lo["altitude_km"])
            return lo[key] + f * (hi[key] - lo[key])
    raise AssertionError


def aero_cd(mach, knots):
    if mach <= knots[0][0]:
        return knots[0][1]
    if mach >= knots[-1][0]:
        return knots[-1][1]
    for lo, hi in zip(knots, knots[1:]):
        if mach <= hi[0]:
            f = (mach - lo[0]) / (hi[0] - lo[0])
            return lo[1] + f * (hi[1] - lo[1])
    raise AssertionError


def gravity(p):
    x, y, z = p
    r = norm(p)
    k = 1.5 * J2 * (A / r) ** 2
    zz = (z / r) ** 2
    common = -MU / r**3
    return (
        common * x * (1.0 - k * (5.0 * zz - 1.0)),
        common * y * (1.0 - k * (5.0 * zz - 1.0)),
        common * z * (1.0 - k * (5.0 * zz - 3.0)),
    )


def pitch_at(t, knots):
    if t <= knots[0][0]:
        return math.radians(knots[0][1])
    if t >= knots[-1][0]:
        return math.radians(knots[-1][1])
    for lo, hi in zip(knots, knots[1:]):
        if t <= hi[0]:
            f = (t - lo[0]) / (hi[0] - lo[0])
            return math.radians(lo[1] + f * (hi[1] - lo[1]))
    raise AssertionError


def run() -> dict:
    source = json.loads(SOURCE.read_text())
    atmosphere = json.loads(ATMOSPHERE.read_text())["records"]
    v = source["vehicle"]
    m = source["mission"]
    lat0 = math.radians(m["launch_latitude_deg"])
    lon0 = math.radians(m["launch_longitude_deg"])
    east, north, up = enu_basis(lat0, lon0)
    azimuth = math.radians(m["launch_azimuth_deg"])
    p = geodetic_to_ecef(lat0, lon0, m["launch_height_km"])
    vel = (0.0, 0.0, 0.0)
    mass = v["dry_mass_kg"] + v["main_propellant_kg"] + v["rcs_propellant_kg"]
    t = 0.0
    apogee = m["launch_height_km"]
    apogee_time = 0.0
    max_q = 0.0
    max_mach = 0.0
    drogue = False
    main = False
    descending = False
    last_height = m["launch_height_km"]
    transitions = []
    segment = "LocalLaunch"
    recovery_anchor = geodetic_to_ecef(
        math.radians(m["recovery_latitude_deg"]),
        math.radians(m["recovery_longitude_deg"]),
        m["recovery_height_km"],
    )

    def acceleration(at_t, at_p, at_v, at_mass):
        nonlocal max_q, max_mach
        lat, lon, height = ecef_to_geodetic(at_p)
        rho = interpolate(height, atmosphere, "density_kg_m3")
        sound = interpolate(height, atmosphere, "speed_of_sound_m_s")
        air_speed_m_s = norm(at_v) * 1000.0
        mach = air_speed_m_s / max(sound, 1.0)
        q = 0.5 * rho * air_speed_m_s**2
        max_q = max(max_q, q)
        max_mach = max(max_mach, mach)
        a = gravity(at_p)
        omega = (0.0, 0.0, OMEGA)
        a = add(a, scale(cross(omega, at_v), -2.0))
        a = add(a, scale(cross(omega, cross(omega, at_p)), -1.0))
        cda = math.pi * (v["diameter_m"] * 0.5) ** 2
        if drogue:
            cda = v["drogue_cda_m2"]
        if main:
            cda = v["main_cda_m2"]
        if air_speed_m_s > 1e-9 and q > 0:
            cd = 1.0 if drogue else aero_cd(mach, source["aerodynamics"])
            drag_accel = q * cda * cd / at_mass / 1000.0
            a = add(a, scale(at_v, -drag_accel / max(norm(at_v), 1e-15)))
        if at_t < v["burn_time_s"]:
            elevation = pitch_at(at_t, m["pitch_schedule"])
            horizontal = add(scale(east, math.sin(azimuth)), scale(north, math.cos(azimuth)))
            direction = add(scale(horizontal, math.cos(elevation)), scale(up, math.sin(elevation)))
            a = add(a, scale(direction, v["thrust_n"] / at_mass / 1000.0))
        return a

    landed = False
    while t < m["max_mission_time_s"]:
        _, _, height = ecef_to_geodetic(p)
        if height > apogee:
            apogee, apogee_time = height, t
        if height < last_height and t > v["burn_time_s"]:
            descending = True
        if descending and not drogue:
            drogue = True
        if descending and height <= m["main_deployment_altitude_km"]:
            main = True
        if segment == "LocalLaunch" and t >= 0.03125 and height > 0.01:
            segment = "EcefAscent"
            transitions.append([segment, t, height])
        rho = interpolate(height, atmosphere, "density_kg_m3")
        q = 0.5 * rho * (norm(vel) * 1000.0) ** 2
        if segment == "EcefAscent" and height > 120.0 and q < 1.0:
            segment = "EciCoast"
            transitions.append([segment, t, height])
        if segment == "EciCoast" and descending and height <= 120.0:
            segment = "EcefEntry"
            transitions.append([segment, t, height])
        if (
            segment == "EcefEntry"
            and height <= 20.0
            and norm(vel) * 1000.0 / max(interpolate(height, atmosphere, "speed_of_sound_m_s"), 1) < 0.8
            and norm(add(p, scale(recovery_anchor, -1.0))) <= 200.0
        ):
            segment = "LocalRecovery"
            transitions.append([segment, t, height])
        dt = 1 / 128 if t < v["burn_time_s"] or height < 120 else 1 / 32
        a0 = acceleration(t, p, vel, mass)
        vm = add(vel, scale(a0, dt * 0.5))
        pm = add(p, scale(vel, dt * 0.5))
        mm = max(v["dry_mass_kg"] + v["rcs_propellant_kg"], mass - v["main_mass_flow_kg_s"] * dt * 0.5)
        am = acceleration(t + dt * 0.5, pm, vm, mm)
        p = add(p, scale(vm, dt))
        vel = add(vel, scale(am, dt))
        if t < v["burn_time_s"]:
            mass = max(v["dry_mass_kg"] + v["rcs_propellant_kg"], mass - v["main_mass_flow_kg_s"] * dt)
        t += dt
        last_height = height
        _, _, next_height = ecef_to_geodetic(p)
        if descending and next_height <= 0.0:
            landed = True
            break
    lat, lon, height = ecef_to_geodetic(p)
    launch_to_landing = math.acos(
        max(
            -1.0,
            min(1.0, math.sin(lat0) * math.sin(lat) + math.cos(lat0) * math.cos(lat) * math.cos(lon - lon0)),
        )
    ) * A
    return {
        "model": "independent-float64-global-v1",
        "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "landed": landed,
        "terminal_time_s": t,
        "terminal_segment": segment,
        "apogee_km": apogee,
        "apogee_time_s": apogee_time,
        "downrange_km": launch_to_landing,
        "landing_latitude_deg": math.degrees(lat),
        "landing_longitude_deg": math.degrees(lon),
        "landing_height_km": height,
        "maximum_dynamic_pressure_pa": max_q,
        "maximum_mach": max_mach,
        "terminal_speed_m_s": norm(vel) * 1000.0,
        "transitions": transitions,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = (json.dumps(run(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if not OUT.exists() or OUT.read_bytes() != content:
            print("phase10 float64 nominal: stale")
            return 1
        print("phase10 float64 nominal: PASS")
        return 0
    OUT.write_bytes(content)
    print(content.decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
