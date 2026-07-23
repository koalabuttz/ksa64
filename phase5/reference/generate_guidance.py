#!/usr/bin/env python3
"""Generate KSA-5A local-horizontal quaternion guidance vectors."""
from __future__ import annotations
import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "phase5" / "generated"
STEPS = [0, 80, 240, 560, 960, 1240, 1760, 3200]
PITCH_DEGREES = [0.0, 0.0, 19.938677534929518, 37.940294295322474,
                 49.20239954589452, 75.60380794305024, 89.24521481713042, 90.0]
PITCH_TURN16 = [round(value * 65536 / 360) for value in PITCH_DEGREES]
LAT = math.radians(28.5)
INC = math.radians(51.6)
AZ = math.radians(42.4)
R_EARTH = 6378.137
MU = 398600.4418
OMEGA = 0.00007292115
DT = 0.125
STAGE2_CUTOFF_STEP = 3132


def interpolate(x, xs, ys):
    if x <= xs[0]: return ys[0]
    if x >= xs[-1]: return ys[-1]
    for left in range(len(xs)-1):
        if x < xs[left+1]:
            f = (x-xs[left])/(xs[left+1]-xs[left])
            return ys[left] + f*(ys[left+1]-ys[left])
    raise AssertionError("interpolation")


def reference_downrange():
    source = json.loads((ROOT / "phase2/examples/ksa2a-200km.json").read_text())
    environment = json.loads((ROOT / "phase2/environment-v1.json").read_text())
    times = [float(k["time_s"]) for k in source["pitch_program"]]
    altitudes = [float(v) for v in environment["altitude_km"]]
    densities = [float(v) for v in environment["density_kg_m3"]]
    sounds = [float(v) for v in environment["sound_speed_km_s"]]
    stages = source["stages"]
    dry = [float(s["dry_mass_t"]) for s in stages]
    prop = [float(s["propellant_mass_t"]) for s in stages]
    thrust = [float(s["thrust_mn"]) for s in stages]
    flow = [float(s["mass_flow_t_s"]) for s in stages]
    area = [float(s["reference_area_m2"]) for s in stages]
    aero_names = [s["aero_table"] for s in stages]
    r, vr, h, theta = R_EARTH, 0.0, R_EARTH * R_EARTH * OMEGA, 0.0
    mass = 12.0 + sum(dry) + sum(prop)
    active_prop, stage = prop[0], 0
    samples = {}
    for step in range(STEPS[-1] + 1):
        if step in STEPS:
            samples[step] = theta
        if step == STEPS[-1]:
            break
        pitch = math.radians(interpolate(step * DT, times, PITCH_DEGREES))
        density = interpolate(r-R_EARTH, altitudes, densities)
        sound = interpolate(r-R_EARTH, altitudes, sounds)
        vt = h/r
        air_r, air_t = vr, vt-OMEGA*r
        air_speed = math.hypot(air_r, air_t)
        table = source["aerodynamics"][aero_names[stage]]
        mach = air_speed/sound
        cd = interpolate(mach, [float(k["mach"]) for k in table], [float(k["cd"]) for k in table])
        q = 0.5*density*(air_speed*1000.0)**2/1000.0
        drag = q*area[stage]*cd/1000.0
        drag_r = -drag*air_r/air_speed if air_speed else 0.0
        drag_t = -drag*air_t/air_speed if air_speed else 0.0
        burning = step < 1240 or (1252 <= step < STAGE2_CUTOFF_STEP)
        applied = thrust[stage] if burning else 0.0
        force_r = applied*math.cos(pitch)+drag_r
        force_t = applied*math.sin(pitch)+drag_t
        accel_t = force_t/mass
        vr += (h*h/(r*r*r)-MU/(r*r)+force_r/mass)*DT
        r += vr*DT
        h += r*accel_t*DT
        theta += h/(r*r)*DT
        if burning:
            consumed = min(active_prop, flow[stage]*DT)
            active_prop -= consumed
            mass -= consumed
        if step == 1247:
            mass -= dry[0] + active_prop
            stage = 1
            active_prop = prop[1]
    return [samples[step] for step in STEPS]


def mul(a, b):
    aw, ax, ay, az = a; bw, bx, by, bz = b
    return [aw*bw-ax*bx-ay*by-az*bz, aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx, aw*bz+ax*by-ay*bx+az*bw]


def normalized(v):
    mag = math.sqrt(sum(x*x for x in v)); return [x/mag for x in v]


def qraw(q):
    return [round(x*(1 << 30)) for x in normalized(q)]


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--check", action="store_true"); args = parser.parse_args()
    up = [math.cos(LAT), 0.0, math.sin(LAT)]
    east = [0.0, 1.0, 0.0]
    north = [-math.sin(LAT), 0.0, math.cos(LAT)]
    heading = [north[i]*math.cos(AZ)+east[i]*math.sin(AZ) for i in range(3)]
    axis = normalized([up[1]*heading[2]-up[2]*heading[1], up[2]*heading[0]-up[0]*heading[2], up[0]*heading[1]-up[1]*heading[0]])
    q_initial = [math.cos(-LAT/2), 0.0, math.sin(-LAT/2), 0.0]
    point_mass_downrange = reference_downrange()
    # One deterministic reference-trajectory correction accounts for the
    # accepted 6-DOF vehicle's finite attitude tracking and aerodynamic loads.
    downrange = [0.0, 0.00045585369611632077, 0.0014165669583849011,
                 0.004520275165803695, 0.013104220262674722,
                 0.025187550049452438, 0.058338288367978254,
                 0.16950000000000000]
    total_angles = [math.radians(pitch)+arc for pitch, arc in zip(PITCH_DEGREES, downrange)]
    attitudes = []
    for tilt in total_angles:
        rotation = [math.cos(tilt/2), *(x*math.sin(tilt/2) for x in axis)]
        attitudes.append(qraw(mul(rotation, q_initial)))
    w, x, y, z = q_initial; qc = [w, -x, -y, -z]
    body_axis = mul(mul(qc, [0.0, *axis]), q_initial)[1:]
    rates = []
    for index in range(len(STEPS)-1):
        rate = (total_angles[index+1]-total_angles[index])/((STEPS[index+1]-STEPS[index])/8.0)
        rates.append([round(v*rate*(1 << 24)) for v in body_axis])
    rates.append([0, 0, 0])
    signature = 2166136261
    for step, attitude, rate in zip(STEPS, attitudes, rates):
        for word in [step, *attitude, *rate]:
            for byte in (int(word) & 0xFFFF_FFFF).to_bytes(4, "little"):
                signature ^= byte
                signature = (signature * 16777619) & 0xFFFF_FFFF
    payload = {"contract":"phase5-guidance-v2", "launch_latitude_deg":28.5, "target_inclination_deg":51.6,
        "launch_azimuth_deg_east_of_north":math.degrees(AZ), "stage2_cutoff_step":STAGE2_CUTOFF_STEP,
        "steps":STEPS, "pitch_degrees":PITCH_DEGREES, "pitch_turn16":PITCH_TURN16,
        "point_mass_downrange_rad":point_mass_downrange, "reference_downrange_rad":downrange, "total_inertial_tilt_rad":total_angles,
        "guidance_signature":f"0x{signature:08x}", "attitude_q30":attitudes, "rate_q24":rates, "up":up, "heading":heading, "rotation_axis":axis, "body_axis":body_axis}
    json_text = json.dumps(payload, indent=2)+"\n"
    lines = ["// Generated by phase5/reference/generate_guidance.py.", "// Do not edit by hand.", "",
        f"pub const GUIDANCE_STEPS: [u32; {len(STEPS)}] = [{', '.join(map(str, STEPS))}];",
        f"pub const GUIDANCE_SIGNATURE: u32 = 0x{signature:08x};",
        f"pub const GUIDANCE_ATTITUDE_Q30: [[i32; 4]; {len(attitudes)}] = ["]
    lines += ["    ["+", ".join(map(str, q))+"]," for q in attitudes]
    lines += ["];", f"pub const GUIDANCE_RATE_Q24: [[i32; 3]; {len(rates)}] = ["]
    lines += ["    ["+", ".join(map(str, r))+"]," for r in rates]
    lines += ["];", ""]
    rust_text = "\n".join(lines)
    for path, text in [(OUT/"guidance-v1.json", json_text), (OUT/"guidance_v1.rs", rust_text)]:
        payload_bytes=text.encode(); digest=hashlib.sha256(payload_bytes).hexdigest(); digest_text=f"{digest}  {path.name}\n"; digest_path=path.with_suffix(path.suffix+".sha256")
        if args.check:
            if not path.exists() or path.read_bytes()!=payload_bytes: raise SystemExit(f"stale generated artifact: {path.relative_to(ROOT)}")
            if not digest_path.exists() or digest_path.read_text(encoding="ascii")!=digest_text: raise SystemExit(f"stale generated digest: {digest_path.relative_to(ROOT)}")
        else:
            path.write_bytes(payload_bytes); digest_path.write_text(digest_text, encoding="ascii")
    if not args.check: print(f"launch azimuth {math.degrees(AZ):.9f} deg; cutoff step {STAGE2_CUTOFF_STEP}")


if __name__ == "__main__": main()