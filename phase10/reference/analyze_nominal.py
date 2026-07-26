#!/usr/bin/env python3
"""Independent float64 KSA-G10R world-model reference and tolerance audit."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "phase10" / "source-data" / "ksa-g10r-source.json"
ATMOSPHERE = ROOT / "phase10" / "generated" / "atmosphere-fixtures-v1.json"
FRAMES = ROOT / "phase10" / "generated" / "frame-fixtures-v1.json"
EXACT = ROOT / "phase10" / "generated" / "uninstrumented-exact-v1.json"
OUT = ROOT / "phase10" / "generated" / "nominal-float64-v1.json"

A = 6378.137
B = 6356.752314245
E2 = 1.0 - (B / A) ** 2
MU = 398600.4418
J2 = 1.08262668e-3
OMEGA = 7.2921150e-5
Q30 = float(1 << 30)


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


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


def normalize(a):
    length = norm(a)
    return scale(a, 1.0 / length)


def qnorm(q):
    length = math.sqrt(sum(value * value for value in q))
    return tuple(value / length for value in q)


def qconj(q):
    return (q[0], -q[1], -q[2], -q[3])


def qmul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    )


def qrot(q, vector):
    pure = (0.0, *vector)
    return qmul(qmul(q, pure), qconj(q))[1:]


def qlerp(a, b, fraction):
    if sum(x * y for x, y in zip(a, b)) < 0:
        b = tuple(-value for value in b)
    return qnorm(tuple(x + (y - x) * fraction for x, y in zip(a, b)))


def quaternion_from_matrix(matrix):
    trace = matrix[0][0] + matrix[1][1] + matrix[2][2]
    if trace > 0:
        s = math.sqrt(trace + 1.0) * 2
        return qnorm(
            (
                0.25 * s,
                (matrix[2][1] - matrix[1][2]) / s,
                (matrix[0][2] - matrix[2][0]) / s,
                (matrix[1][0] - matrix[0][1]) / s,
            )
        )
    axis = max(range(3), key=lambda index: matrix[index][index])
    if axis == 0:
        s = math.sqrt(1.0 + matrix[0][0] - matrix[1][1] - matrix[2][2]) * 2
        return qnorm(
            (
                (matrix[2][1] - matrix[1][2]) / s,
                0.25 * s,
                (matrix[0][1] + matrix[1][0]) / s,
                (matrix[0][2] + matrix[2][0]) / s,
            )
        )
    if axis == 1:
        s = math.sqrt(1.0 + matrix[1][1] - matrix[0][0] - matrix[2][2]) * 2
        return qnorm(
            (
                (matrix[0][2] - matrix[2][0]) / s,
                (matrix[0][1] + matrix[1][0]) / s,
                0.25 * s,
                (matrix[1][2] + matrix[2][1]) / s,
            )
        )
    s = math.sqrt(1.0 + matrix[2][2] - matrix[0][0] - matrix[1][1]) * 2
    return qnorm(
        (
            (matrix[1][0] - matrix[0][1]) / s,
            (matrix[0][2] + matrix[2][0]) / s,
            (matrix[1][2] + matrix[2][1]) / s,
            0.25 * s,
        )
    )


def body_x_attitude(direction):
    direction = normalize(direction)
    half_sum = (1.0 + direction[0]) * 0.5
    if half_sum <= 1e-15:
        return (0.0, 0.0, 0.0, 1.0)
    w = math.sqrt(half_sum)
    return qnorm((w, 0.0, -direction[2] / (2 * w), direction[1] / (2 * w)))


def attitude_delta_deg(a, b):
    cosine = min(1.0, max(-1.0, abs(sum(x * y for x, y in zip(qnorm(a), qnorm(b))))))
    return math.degrees(2.0 * math.acos(cosine))


def geodetic_to_ecef(lat, lon, height_km):
    n = A / math.sqrt(1.0 - E2 * math.sin(lat) ** 2)
    return (
        (n + height_km) * math.cos(lat) * math.cos(lon),
        (n + height_km) * math.cos(lat) * math.sin(lon),
        (n * (1.0 - E2) + height_km) * math.sin(lat),
    )


def ecef_to_geodetic(position):
    x, y, z = position
    lon = math.atan2(y, x)
    horizontal = math.hypot(x, y)
    lat = math.atan2(z, horizontal * (1.0 - E2))
    height = 0.0
    for _ in range(8):
        n = A / math.sqrt(1.0 - E2 * math.sin(lat) ** 2)
        height = horizontal / max(math.cos(lat), 1e-15) - n
        lat = math.atan2(z, horizontal * (1.0 - E2 * n / (n + height)))
    return lat, lon, height


def enu_basis(lat, lon):
    return (
        (-math.sin(lon), math.cos(lon), 0.0),
        (-math.sin(lat) * math.cos(lon), -math.sin(lat) * math.sin(lon), math.cos(lat)),
        (math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat)),
    )


def basis_quaternion(basis):
    east, north, up = basis
    matrix = (
        (east[0], north[0], up[0]),
        (east[1], north[1], up[1]),
        (east[2], north[2], up[2]),
    )
    return quaternion_from_matrix(matrix)


def local_to_ecef(origin, basis, position_m, velocity_m_s):
    position = origin
    velocity = (0.0, 0.0, 0.0)
    for axis in range(3):
        position = add(position, scale(basis[axis], position_m[axis] / 1000.0))
        velocity = add(velocity, scale(basis[axis], velocity_m_s[axis] / 1000.0))
    return position, velocity


def ecef_to_local(origin, basis, position, velocity):
    offset = sub(position, origin)
    return (
        tuple(dot(offset, axis) * 1000.0 for axis in basis),
        tuple(dot(velocity, axis) * 1000.0 for axis in basis),
    )


def interpolate_scalar(x, table, key):
    if x <= table[0]["altitude_km"]:
        return table[0][key]
    if x >= table[-1]["altitude_km"]:
        return 0.0 if key in ("density_kg_m3", "pressure_pa") else table[-1][key]
    for lo, hi in zip(table, table[1:]):
        if x <= hi["altitude_km"]:
            fraction = (x - lo["altitude_km"]) / (
                hi["altitude_km"] - lo["altitude_km"]
            )
            return lo[key] + fraction * (hi[key] - lo[key])
    raise AssertionError


def atmosphere_at(height, table):
    height = round(height * (1 << 12)) / (1 << 12)
    if height >= table[-1]["altitude_km"]:
        density = 0.0
        sound = table[-1]["raw"][4] / (1 << 16)
        return density, sound, (0.0, 0.0, 0.0)
    if height <= table[0]["altitude_km"]:
        records = (table[0], table[0], 0.0)
    else:
        records = None
        for lo, hi in zip(table, table[1:]):
            if height <= hi["altitude_km"]:
                fraction = (height - lo["altitude_km"]) / (
                    hi["altitude_km"] - lo["altitude_km"]
                )
                records = (lo, hi, fraction)
                break
    lo, hi, fraction = records
    density_lo, density_hi = lo["raw"][1] / (1 << 28), hi["raw"][1] / (1 << 28)
    sound_lo, sound_hi = lo["raw"][4] / (1 << 16), hi["raw"][4] / (1 << 16)
    density = density_lo + (density_hi - density_lo) * fraction
    sound = sound_lo + (sound_hi - sound_lo) * fraction
    wind_lo = tuple(value / (1 << 19) for value in lo["raw"][5:8])
    wind_hi = tuple(value / (1 << 19) for value in hi["raw"][5:8])
    wind = tuple(a + (b - a) * fraction for a, b in zip(wind_lo, wind_hi))
    return density, sound, wind


def aero_cd(mach, knots):
    if mach <= knots[0][0]:
        return knots[0][1]
    if mach >= knots[-1][0]:
        return knots[-1][1]
    for lo, hi in zip(knots, knots[1:]):
        if mach <= hi[0]:
            fraction = (mach - lo[0]) / (hi[0] - lo[0])
            return lo[1] + fraction * (hi[1] - lo[1])
    raise AssertionError


def gravity(position):
    x, y, z = position
    radius = norm(position)
    correction = 1.5 * J2 * (A / radius) ** 2
    z_ratio_2 = (z / radius) ** 2
    common = -MU / radius**3
    return (
        common * x * (1.0 - correction * (5.0 * z_ratio_2 - 1.0)),
        common * y * (1.0 - correction * (5.0 * z_ratio_2 - 1.0)),
        common * z * (1.0 - correction * (5.0 * z_ratio_2 - 3.0)),
    )


def pitch_at(time, knots):
    if time <= knots[0][0]:
        return math.radians(knots[0][1])
    if time >= knots[-1][0]:
        return math.radians(knots[-1][1])
    for lo, hi in zip(knots, knots[1:]):
        if time <= hi[0]:
            fraction = (time - lo[0]) / (hi[0] - lo[0])
            return math.radians(lo[1] + fraction * (hi[1] - lo[1]))
    raise AssertionError


def transform_at(time, frame_data):
    knots = frame_data["transform_knots"]
    index = min(int(time // 60), len(knots) - 2)
    lo, hi = knots[index], knots[index + 1]
    fraction = (time - lo["elapsed_s"]) / (hi["elapsed_s"] - lo["elapsed_s"])
    q = qlerp(
        tuple(lo["ecef_to_gcrf_quaternion_wxyz"]),
        tuple(hi["ecef_to_gcrf_quaternion_wxyz"]),
        fraction,
    )
    omega = tuple(
        a + (b - a) * fraction
        for a, b in zip(
            lo["angular_velocity_gcrf_rad_s"],
            hi["angular_velocity_gcrf_rad_s"],
        )
    )
    return q, omega


def ecef_to_gcrf(position, velocity, time, frame_data):
    rotation, omega = transform_at(time, frame_data)
    inertial_position = qrot(rotation, position)
    inertial_velocity = add(qrot(rotation, velocity), cross(omega, inertial_position))
    return inertial_position, inertial_velocity


def gcrf_to_ecef(position, velocity, time, frame_data):
    rotation, omega = transform_at(time, frame_data)
    fixed_position = qrot(qconj(rotation), position)
    fixed_velocity = qrot(qconj(rotation), sub(velocity, cross(omega, position)))
    return fixed_position, fixed_velocity


def is_release(time):
    return abs(time * 32.0 - round(time * 32.0)) < 1e-9


def compare(reference, result):
    transition_delta = max(
        abs(a["time_s"] - b[1])
        for a, b in zip(reference["transitions"], result["transitions"])
    )
    flight_event_delta = max(
        abs(reference["event_times_s"][name] - result["event_times_s"][name])
        for name in ("rail_clear", "burnout", "apogee", "drogue", "main")
    )
    landing_time_delta = abs(
        reference["event_times_s"]["landing"] - result["event_times_s"]["landing"]
    )
    landing_time_tolerance = 4.0 / 32.0
    attitude_delta = max(
        attitude_delta_deg(
            tuple(value / Q30 for value in exact["attitude_q30"]),
            tuple(model["attitude_wxyz"]),
        )
        for exact, model in zip(
            reference["transition_samples"], result["transition_samples"]
        )
    )
    landing_delta = norm(
        sub(
            tuple(reference["terminal_ecef_position_km"]),
            tuple(result["terminal_ecef_position_km"]),
        )
    )
    apogee_percent = (
        abs(reference["apogee_km"] - result["apogee_km"])
        / reference["apogee_km"]
        * 100.0
    )
    downrange_percent = (
        abs(reference["downrange_km"] - result["downrange_km"])
        / abs(reference["downrange_km"])
        * 100.0
    )
    landing_limit = max(2.0, 0.02 * abs(reference["downrange_km"]))
    checks = {
        "apogee_within_0_5_percent": apogee_percent <= 0.5,
        "downrange_within_0_5_percent": downrange_percent <= 0.5,
        "landing_within_2km_or_2_percent": landing_delta <= landing_limit,
        "transition_times_within_one_release": transition_delta <= 1.0 / 32.0,
        "flight_events_within_one_step": flight_event_delta <= 1.0 / 32.0,
        "landing_time_within_fixedpoint_bound": landing_time_delta <= landing_time_tolerance,
        "attitude_within_0_5_degree": attitude_delta <= 0.5,
    }
    return {
        "exact_reference_schema": reference["schema"],
        "apogee_delta_percent": apogee_percent,
        "downrange_delta_percent": downrange_percent,
        "landing_position_delta_km": landing_delta,
        "landing_position_limit_km": landing_limit,
        "maximum_transition_time_delta_s": transition_delta,
        "maximum_flight_event_time_delta_s": flight_event_delta,
        "landing_time_delta_s": landing_time_delta,
        "landing_time_tolerance_s": landing_time_tolerance,
        "landing_time_tolerance_basis": "four recovery steps for accumulated fixed-point descent error",
        "maximum_transition_attitude_delta_deg": attitude_delta,
        "checks": checks,
        "pass": all(checks.values()),
    }


def run():
    source = json.loads(SOURCE.read_text())
    atmosphere = json.loads(ATMOSPHERE.read_text())["records"]
    frames = json.loads(FRAMES.read_text())
    vehicle = source["vehicle"]
    mission = source["mission"]
    launch_lat = math.radians(mission["launch_latitude_deg"])
    launch_lon = math.radians(mission["launch_longitude_deg"])
    launch_basis = enu_basis(launch_lat, launch_lon)
    launch_rotation = basis_quaternion(launch_basis)
    launch_origin = geodetic_to_ecef(
        launch_lat, launch_lon, mission["launch_height_km"]
    )
    recovery_lat = math.radians(mission["recovery_latitude_deg"])
    recovery_lon = math.radians(mission["recovery_longitude_deg"])
    recovery_basis = enu_basis(recovery_lat, recovery_lon)
    recovery_rotation = basis_quaternion(recovery_basis)
    recovery_origin = geodetic_to_ecef(
        recovery_lat, recovery_lon, mission["recovery_height_km"]
    )
    azimuth = math.radians(mission["launch_azimuth_deg"])
    horizontal = add(
        scale(launch_basis[0], math.sin(azimuth)),
        scale(launch_basis[1], math.cos(azimuth)),
    )
    reference_area = math.pi * (vehicle["diameter_m"] * 0.5) ** 2
    dry_with_rcs = vehicle["dry_mass_kg"] + vehicle["rcs_propellant_kg"]
    mass = dry_with_rcs + vehicle["main_propellant_kg"]
    time = 0.0
    segment = "LocalLaunch"
    local_position = (0.0, 0.0, 0.0)
    local_velocity = (0.0, 0.0, 0.0)
    position = launch_origin
    velocity = (0.0, 0.0, 0.0)
    attitude = body_x_attitude((0.0, 0.0, 1.0))
    descending = False
    drogue = False
    main = False
    last_height = mission["launch_height_km"]
    apogee = last_height
    apogee_time = 0.0
    maximum_q = 0.0
    maximum_mach = 0.0
    transitions = []
    transition_samples = []
    event_times = {
        "rail_clear": None,
        "burnout": None,
        "apogee": None,
        "drogue": None,
        "main": None,
        "landing": None,
    }

    def direction_ecef(at_time):
        elevation = pitch_at(at_time, mission["pitch_schedule"])
        return add(
            scale(horizontal, math.cos(elevation)),
            scale(launch_basis[2], math.sin(elevation)),
        )

    def commanded_attitude(at_time, owner):
        direction = direction_ecef(at_time)
        if owner == "EciCoast":
            rotation, _ = transform_at(at_time, frames)
            direction = qrot(rotation, direction)
        return body_x_attitude(direction)

    def atmosphere_state(at_position):
        latitude, longitude, height = ecef_to_geodetic(at_position)
        density, sound, wind_enu = atmosphere_at(height, atmosphere)
        basis = enu_basis(latitude, longitude)
        wind = (0.0, 0.0, 0.0)
        for axis in range(3):
            wind = add(wind, scale(basis[axis], wind_enu[axis] / 1000.0))
        return height, density, sound, wind

    def global_acceleration(at_time, at_position, at_velocity, at_mass, owner):
        if owner == "EciCoast":
            ecef_position, ecef_velocity = gcrf_to_ecef(
                at_position, at_velocity, at_time, frames
            )
        else:
            ecef_position, ecef_velocity = at_position, at_velocity
        height, density, sound, wind = atmosphere_state(ecef_position)
        air_velocity = sub(ecef_velocity, wind)
        speed = norm(air_velocity)
        speed_m_s = speed * 1000.0
        mach = 0.0 if density == 0 else speed_m_s / max(sound, 1.0)
        dynamic_pressure = 0.5 * density * speed_m_s**2
        area = reference_area
        if drogue:
            area = (
                vehicle["main_cda_m2"] if main else vehicle["drogue_cda_m2"]
            )
            cd = 1.0
        else:
            if owner == "EcefEntry" and descending:
                area *= mission["entry_drag_area_scale"]
            cd = aero_cd(mach, source["aerodynamics"])
        acceleration = gravity(ecef_position)
        if owner != "EciCoast":
            omega = (0.0, 0.0, OMEGA)
            acceleration = add(acceleration, scale(cross(omega, ecef_velocity), -2.0))
            acceleration = add(
                acceleration,
                scale(cross(omega, cross(omega, ecef_position)), -1.0),
            )
        if speed > 1e-15 and dynamic_pressure > 0:
            drag_acceleration = dynamic_pressure * area * cd / at_mass / 1000.0
            acceleration = add(
                acceleration, scale(air_velocity, -drag_acceleration / speed)
            )
        if at_time < vehicle["burn_time_s"]:
            acceleration = add(
                acceleration,
                scale(
                    direction_ecef(at_time),
                    vehicle["thrust_n"] / at_mass / 1000.0,
                ),
            )
        if owner == "EciCoast":
            rotation, _ = transform_at(at_time, frames)
            acceleration = qrot(rotation, acceleration)
        return acceleration, dynamic_pressure, mach

    steps = 0
    while time < mission["max_mission_time_s"]:
        if segment == "LocalLaunch":
            dt = 1.0 / 128.0
            height = mission["launch_height_km"] + local_position[2] / 1000.0
            density, sound, _ = atmosphere_at(height, atmosphere)
            speed_m_s = norm(local_velocity)
            mach = speed_m_s / max(sound, 1.0)
            dynamic_pressure = 0.5 * density * speed_m_s**2
            cd = aero_cd(mach, source["aerodynamics"])
            drag = dynamic_pressure * reference_area * cd / mass
            acceleration = max(0.0, vehicle["thrust_n"] / mass - drag - 9.806640625)
            successor_velocity = local_velocity[2] + acceleration * dt
            successor_position = local_position[2] + (
                local_velocity[2] + successor_velocity
            ) * 0.5 * dt
            local_velocity = (0.0, 0.0, successor_velocity)
            local_position = (0.0, 0.0, successor_position)
            time += dt
            mass = max(
                dry_with_rcs,
                mass - vehicle["main_mass_flow_kg_s"] * dt,
            )
            maximum_q = max(maximum_q, dynamic_pressure)
            maximum_mach = max(maximum_mach, mach)
            height = mission["launch_height_km"] + local_position[2] / 1000.0
            if is_release(time) and local_position[2] >= mission["rail_length_m"]:
                position, velocity = local_to_ecef(
                    launch_origin, launch_basis, local_position, local_velocity
                )
                attitude = qnorm(
                    qmul(launch_rotation, body_x_attitude((0.0, 0.0, 1.0)))
                )
                segment = "EcefAscent"
                event_times["rail_clear"] = time
                transitions.append([segment, time, height])
                transition_samples.append(
                    {"time_s": time, "attitude_wxyz": attitude}
                )
            steps += 1
            continue

        if segment == "LocalRecovery":
            dt = 1.0 / 32.0
            height = mission["recovery_height_km"] + local_position[2] / 1000.0
            density, sound, _ = atmosphere_at(height, atmosphere)
            speed = norm(local_velocity)
            mach = speed / max(sound, 1.0)
            dynamic_pressure = 0.5 * density * speed**2
            area = vehicle["main_cda_m2"] if main else vehicle["drogue_cda_m2"]
            drag_acceleration = dynamic_pressure * area / mass
            drag = (
                (0.0, 0.0, 0.0)
                if speed <= 1e-15
                else scale(local_velocity, -drag_acceleration / speed)
            )
            acceleration = add(drag, (0.0, 0.0, -9.806640625))
            successor_velocity = add(local_velocity, scale(acceleration, dt))
            local_position = add(
                local_position,
                scale(add(local_velocity, successor_velocity), 0.5 * dt),
            )
            local_velocity = successor_velocity
            time += dt
            maximum_q = max(maximum_q, dynamic_pressure)
            maximum_mach = max(maximum_mach, mach)
            if not main and height <= mission["main_deployment_altitude_km"]:
                main = True
                event_times["main"] = time
            if local_position[2] <= 0 and local_velocity[2] < 0:
                event_times["landing"] = time
                position, velocity = local_to_ecef(
                    recovery_origin, recovery_basis, local_position, local_velocity
                )
                break
            steps += 1
            continue

        dt = 1.0 / 32.0 if segment == "EciCoast" else 1.0 / 128.0
        acceleration_0, _, _ = global_acceleration(
            time, position, velocity, mass, segment
        )
        midpoint_velocity = add(velocity, scale(acceleration_0, dt * 0.5))
        midpoint_position = add(position, scale(velocity, dt * 0.5))
        midpoint_mass = mass
        if time < vehicle["burn_time_s"]:
            midpoint_mass = max(
                dry_with_rcs,
                mass - vehicle["main_mass_flow_kg_s"] * dt * 0.5,
            )
        acceleration_mid, dynamic_pressure, mach = global_acceleration(
            time + dt * 0.5,
            midpoint_position,
            midpoint_velocity,
            midpoint_mass,
            segment,
        )
        position = add(position, scale(midpoint_velocity, dt))
        velocity = add(velocity, scale(acceleration_mid, dt))
        time += dt
        if time <= vehicle["burn_time_s"]:
            mass = max(
                dry_with_rcs,
                mass - vehicle["main_mass_flow_kg_s"] * dt,
            )
        if event_times["burnout"] is None and time >= vehicle["burn_time_s"]:
            event_times["burnout"] = time
        maximum_q = max(maximum_q, dynamic_pressure)
        maximum_mach = max(maximum_mach, mach)
        attitude = commanded_attitude(time, segment)
        ecef_position, ecef_velocity = (
            gcrf_to_ecef(position, velocity, time, frames)
            if segment == "EciCoast"
            else (position, velocity)
        )
        _, _, height = ecef_to_geodetic(ecef_position)
        if height > apogee:
            apogee = height
            apogee_time = time
        elif (
            not descending
            and time > vehicle["burn_time_s"]
            and height < last_height
        ):
            descending = True
            drogue = True
            event_times["apogee"] = time
            event_times["drogue"] = time
        if descending and not main and height <= mission["main_deployment_altitude_km"]:
            main = True
            event_times["main"] = time
        last_height = height
        if is_release(time):
            if (
                segment == "EcefAscent"
                and height > mission["eci_transition_altitude_km"]
                and dynamic_pressure < mission["transition_dynamic_pressure_pa"]
            ):
                position, velocity = ecef_to_gcrf(
                    position, velocity, time, frames
                )
                rotation, _ = transform_at(time, frames)
                attitude = qnorm(qmul(rotation, attitude))
                segment = "EciCoast"
                transitions.append([segment, time, height])
                transition_samples.append(
                    {"time_s": time, "attitude_wxyz": attitude}
                )
            elif (
                segment == "EciCoast"
                and descending
                and height <= mission["entry_transition_altitude_km"]
            ):
                position, velocity = gcrf_to_ecef(
                    position, velocity, time, frames
                )
                rotation, _ = transform_at(time, frames)
                attitude = qnorm(qmul(qconj(rotation), attitude))
                segment = "EcefEntry"
                transitions.append([segment, time, height])
                transition_samples.append(
                    {"time_s": time, "attitude_wxyz": attitude}
                )
            elif (
                segment == "EcefEntry"
                and height <= mission["recovery_transition_altitude_km"]
                and mach <= mission["recovery_transition_mach"]
                and norm(sub(position, recovery_origin))
                <= mission["recovery_radius_km"]
            ):
                local_position, local_velocity = ecef_to_local(
                    recovery_origin, recovery_basis, position, velocity
                )
                transition_attitude = attitude
                attitude = qnorm(qmul(qconj(recovery_rotation), attitude))
                segment = "LocalRecovery"
                transitions.append([segment, time, height])
                transition_samples.append(
                    {"time_s": time, "attitude_wxyz": transition_attitude}
                )
        steps += 1

    latitude, longitude, height = ecef_to_geodetic(position)
    launch_offset, _ = ecef_to_local(
        launch_origin, launch_basis, position, velocity
    )
    result = {
        "model": "independent-float64-global-world-v2",
        "scope": "complete uninstrumented global physical path; avionics verified separately",
        "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "landed": event_times["landing"] is not None,
        "steps": steps,
        "terminal_time_s": time,
        "terminal_segment": segment,
        "apogee_km": apogee,
        "apogee_time_s": apogee_time,
        "downrange_km": launch_offset[0] / 1000.0,
        "crossrange_km": launch_offset[1] / 1000.0,
        "landing_latitude_deg": math.degrees(latitude),
        "landing_longitude_deg": math.degrees(longitude),
        "landing_height_km": height,
        "terminal_ecef_position_km": position,
        "terminal_ecef_velocity_km_s": velocity,
        "maximum_dynamic_pressure_pa": maximum_q,
        "maximum_mach": maximum_mach,
        "terminal_speed_m_s": norm(velocity) * 1000.0,
        "transitions": transitions,
        "transition_samples": transition_samples,
        "event_times_s": event_times,
    }
    if EXACT.exists():
        result["comparison"] = compare(json.loads(EXACT.read_text()), result)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = (json.dumps(run(), indent=2, sort_keys=True) + "\n").encode()
    if args.check:
        if not OUT.exists() or OUT.read_bytes() != content:
            print("phase10 float64 nominal: stale")
            return 1
        result = json.loads(content)
        if not result.get("comparison", {}).get("pass", False):
            print("phase10 float64 nominal: tolerance failure")
            return 2
        print("phase10 float64 nominal: PASS")
        return 0
    OUT.write_bytes(content)
    print(content.decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
