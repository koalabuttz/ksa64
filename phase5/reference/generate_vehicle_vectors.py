#!/usr/bin/env python3
"""Generate Phase 5 multirate vehicle constants and independent component vectors."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from decimal import Decimal, ROUND_HALF_UP, getcontext
from pathlib import Path

getcontext().prec = 50
ROOT = Path(__file__).resolve().parents[2]
JSON_OUT = ROOT / "phase5" / "generated" / "vehicle-v1.json"
RUST_OUT = ROOT / "phase5" / "generated" / "vehicle_v1.rs"


def raw(value: Decimal | str | float, bits: int) -> int:
    return int((Decimal(str(value)) * (1 << bits)).to_integral_value(rounding=ROUND_HALF_UP))


def cylinder_inertia(mass_t: Decimal, length_m: Decimal, radius_m: Decimal) -> list[int]:
    roll = mass_t * radius_m * radius_m / Decimal(2)
    transverse = mass_t * (Decimal(3) * radius_m * radius_m + length_m * length_m) / Decimal(12)
    return [raw(roll, 12), raw(transverse, 12), raw(transverse, 12)]

def mul_scaled(a: int, b: int, shift: int) -> int:
    magnitude = abs(a * b)
    result = (magnitude + (1 << (shift - 1))) >> shift if shift else magnitude
    return -result if (a < 0) ^ (b < 0) else result


def trunc_div(value: int, denominator: int) -> int:
    return -(abs(value) // denominator) if value < 0 else value // denominator


def build() -> dict:
    stages = [
        {
            "id": "s1", "dry_mass_t": Decimal("30"), "propellant_mass_t": Decimal("400"),
            "supported_dry_mass_t": Decimal("134"), "supported_wet_mass_t": Decimal("534"),
            "thrust_mn": Decimal("7.6"), "mass_flow_t_s": Decimal("2.58"),
            "burn_mission_steps": 1240, "separation_delay_steps": 8, "ignition_delay_steps": 0,
            "length_m": Decimal("40"), "radius_m": Decimal("2.985"), "gimbal_arm_m": Decimal("20"),
            "area_m2": Decimal("28"), "normal_slope": Decimal("2.0"), "cp_aft_m": Decimal("6"),
            "rate_damping": Decimal("0.18"), "bend_hz": Decimal("1.2"), "bend_zeta": Decimal("0.015"),
            "aero": [("0", "0.30"), ("0.8", "0.36"), ("1", "0.55"), ("1.2", "0.62"), ("2", "0.40"), ("5", "0.25"), ("25", "0.20")],
            "slosh_hz": Decimal("0.45"), "slosh_zeta": Decimal("0.03"),
        },
        {
            "id": "s2", "dry_mass_t": Decimal("8"), "propellant_mass_t": Decimal("84"),
            "supported_dry_mass_t": Decimal("20"), "supported_wet_mass_t": Decimal("104"),
            "thrust_mn": Decimal("1.1"), "mass_flow_t_s": Decimal("0.35"),
            "burn_mission_steps": 1920, "separation_delay_steps": 0, "ignition_delay_steps": 4,
            "length_m": Decimal("20"), "radius_m": Decimal("1.784"), "gimbal_arm_m": Decimal("9"),
            "area_m2": Decimal("10"), "normal_slope": Decimal("1.6"), "cp_aft_m": Decimal("3"),
            "rate_damping": Decimal("0.12"), "bend_hz": Decimal("1.8"), "bend_zeta": Decimal("0.02"),
            "aero": [("0", "0.25"), ("1", "0.45"), ("2", "0.30"), ("5", "0.20"), ("25", "0.18")],
            "slosh_hz": Decimal("0.70"), "slosh_zeta": Decimal("0.04"),
        },
    ]
    latitude = math.radians(28.5)
    half = -latitude / 2
    result_stages = []
    for stage in stages:
        result_stages.append({
            "id": stage["id"],
            "dry_mass_q12": raw(stage["dry_mass_t"], 12),
            "propellant_mass_q12": raw(stage["propellant_mass_t"], 12),
            "thrust_q12": raw(stage["thrust_mn"], 12),
            "mass_flow_q16": raw(stage["mass_flow_t_s"], 16),
            "burn_mission_steps": stage["burn_mission_steps"],
            "separation_delay_steps": stage["separation_delay_steps"],
            "ignition_delay_steps": stage["ignition_delay_steps"],
            "gimbal_arm_q16": raw(stage["gimbal_arm_m"], 16),
            "area_q16": raw(stage["area_m2"], 16),
            "normal_slope_q14": raw(stage["normal_slope"], 14),
            "cp_aft_q16": raw(stage["cp_aft_m"], 16),
            "rate_damping_q16": raw(stage["rate_damping"], 16),
            "inertia_wet_q12": cylinder_inertia(stage["supported_wet_mass_t"], stage["length_m"], stage["radius_m"]),
            "inertia_dry_q12": cylinder_inertia(stage["supported_dry_mass_t"], stage["length_m"], stage["radius_m"]),
            "bend_omega_q16": raw(Decimal(2) * Decimal(str(math.pi)) * stage["bend_hz"], 16),
            "bend_zeta_q16": raw(stage["bend_zeta"], 16),
            "slosh_omega_q16": raw(Decimal(2) * Decimal(str(math.pi)) * stage["slosh_hz"], 16),
            "slosh_zeta_q16": raw(stage["slosh_zeta"], 16),
            "aero_mach_q16": [raw(mach, 16) for mach, _ in stage["aero"]],
            "aero_cd_q14": [raw(cd, 14) for _, cd in stage["aero"]],
        })
    # With force in kN, F/(Isp*g0) is numerically tonnes per second.
    max_axis_mass_flow = Decimal("20") / (Decimal("220") * Decimal("9.80665"))
    gimbal_limit = raw(math.radians(6), 16)
    gimbal_slew = raw(math.radians(8) * 0.03125, 16)
    lagged = 0
    applied = 0
    gimbal_points = []
    for step in range(1, 9):
        lagged += trunc_div(gimbal_limit - lagged, 4)
        applied += max(-gimbal_slew, min(gimbal_slew, lagged - applied))
        if step in (1, 4, 8):
            gimbal_points.append([lagged, applied])
    max_flow_q24 = raw(max_axis_mass_flow, 24)
    rcs_flow_q24 = mul_scaled(max_flow_q24, 32767, 15)
    rcs_step_q24 = mul_scaled(rcs_flow_q24, raw("0.03125", 16), 16)
    return {
        "contract": "ksa64.vehicle.phase5-v1",
        "portable_signature": "0x21e55663",
        "payload_mass_q12": raw("12", 12),
        "initial_total_mass_q12": raw("534", 12),
        "fast_step_q16": raw("0.03125", 16),
        "substeps": 4,
        "gimbal_limit_q16": gimbal_limit,
        "gimbal_slew_per_fast_step_q16": gimbal_slew,
        "gimbal_lag_steps": 4,
        "rcs_propellant_q12": raw("0.10", 12),
        "rcs_max_torque_q16": raw("0.08", 16),
        "rcs_max_axis_mass_flow_q24": raw(max_axis_mass_flow, 24),
        "component_vectors": {
            "gimbal_positive_lagged_applied_q16": gimbal_points,
            "stage_inertia_half_q12": [[(a + b) // 2 for a, b in zip(x["inertia_dry_q12"], x["inertia_wet_q12"])] for x in result_stages],
            "rcs_full_axis_one_mission_consumed_q12": (rcs_step_q24 * 4) >> 12,
        },
        "initial_attitude_q30": [raw(math.cos(half), 30), 0, raw(math.sin(half), 30), 0],
        "flex_bend_drive_gain_q16": raw("0.02", 16),
        "flex_slosh_drive_gain_q16": raw("0.05", 16),
        "stages": result_stages,
    }


def rust(data: dict) -> str:
    def arr(values: list[int]) -> str:
        return "[" + ", ".join(str(v) for v in values) + "]"
    stage = data["stages"]
    fields = [
        ("STAGE_DRY_MASS_Q12", "i32", [x["dry_mass_q12"] for x in stage]),
        ("STAGE_PROPELLANT_MASS_Q12", "i32", [x["propellant_mass_q12"] for x in stage]),
        ("STAGE_THRUST_Q12", "i32", [x["thrust_q12"] for x in stage]),
        ("STAGE_MASS_FLOW_Q16", "i32", [x["mass_flow_q16"] for x in stage]),
        ("STAGE_BURN_MISSION_STEPS", "u32", [x["burn_mission_steps"] for x in stage]),
        ("STAGE_SEPARATION_DELAY_STEPS", "u16", [x["separation_delay_steps"] for x in stage]),
        ("STAGE_IGNITION_DELAY_STEPS", "u16", [x["ignition_delay_steps"] for x in stage]),
        ("STAGE_GIMBAL_ARM_Q16", "i32", [x["gimbal_arm_q16"] for x in stage]),
        ("STAGE_AREA_Q16", "i32", [x["area_q16"] for x in stage]),
        ("STAGE_NORMAL_SLOPE_Q14", "i32", [x["normal_slope_q14"] for x in stage]),
        ("STAGE_CP_AFT_Q16", "i32", [x["cp_aft_q16"] for x in stage]),
        ("STAGE_RATE_DAMPING_Q16", "i32", [x["rate_damping_q16"] for x in stage]),
        ("STAGE_BEND_OMEGA_Q16", "i32", [x["bend_omega_q16"] for x in stage]),
        ("STAGE_BEND_ZETA_Q16", "i32", [x["bend_zeta_q16"] for x in stage]),
        ("STAGE_SLOSH_OMEGA_Q16", "i32", [x["slosh_omega_q16"] for x in stage]),
        ("STAGE_SLOSH_ZETA_Q16", "i32", [x["slosh_zeta_q16"] for x in stage]),
    ]
    lines = ["// Generated by phase5/reference/generate_vehicle_vectors.py.", "// Do not edit by hand.", "",
             "pub const VEHICLE_SIGNATURE: u32 = 0x21e55663;",
             f"pub const PAYLOAD_MASS_Q12: i32 = {data['payload_mass_q12']};",
             f"pub const INITIAL_TOTAL_MASS_Q12: i32 = {data['initial_total_mass_q12']};",
             f"pub const FAST_STEP_Q16: i32 = {data['fast_step_q16']};",
             f"pub const SUBSTEPS: u8 = {data['substeps']};",
             f"pub const GIMBAL_LIMIT_Q16: i32 = {data['gimbal_limit_q16']};",
             f"pub const GIMBAL_SLEW_PER_FAST_STEP_Q16: i32 = {data['gimbal_slew_per_fast_step_q16']};",
             f"pub const GIMBAL_LAG_STEPS: u8 = {data['gimbal_lag_steps']};",
             f"pub const RCS_PROPELLANT_Q12: i32 = {data['rcs_propellant_q12']};",
             f"pub const RCS_MAX_TORQUE_Q16: i32 = {data['rcs_max_torque_q16']};",
             f"pub const RCS_MAX_AXIS_MASS_FLOW_Q24: i32 = {data['rcs_max_axis_mass_flow_q24']};",
             f"pub const INITIAL_ATTITUDE_Q30: [i32; 4] = {arr(data['initial_attitude_q30'])};",
             f"pub const FLEX_BEND_DRIVE_GAIN_Q16: i32 = {data['flex_bend_drive_gain_q16']};",
             f"pub const FLEX_SLOSH_DRIVE_GAIN_Q16: i32 = {data['flex_slosh_drive_gain_q16']};",
             f"pub const GIMBAL_POSITIVE_LAGGED_APPLIED_Q16: [[i32; 2]; 3] = [{arr(data['component_vectors']['gimbal_positive_lagged_applied_q16'][0])}, {arr(data['component_vectors']['gimbal_positive_lagged_applied_q16'][1])}, {arr(data['component_vectors']['gimbal_positive_lagged_applied_q16'][2])}];",
             f"pub const RCS_FULL_AXIS_ONE_MISSION_CONSUMED_Q12: i32 = {data['component_vectors']['rcs_full_axis_one_mission_consumed_q12']};", ""]
    for name, typ, values in fields:
        lines.append(f"pub const {name}: [{typ}; 2] = {arr(values)};")
    lines.append(f"pub const STAGE0_AERO_MACH_Q16: [i32; 7] = {arr(stage[0]['aero_mach_q16'])};")
    lines.append(f"pub const STAGE0_AERO_CD_Q14: [i32; 7] = {arr(stage[0]['aero_cd_q14'])};")
    lines.append(f"pub const STAGE1_AERO_MACH_Q16: [i32; 5] = {arr(stage[1]['aero_mach_q16'])};")
    lines.append(f"pub const STAGE1_AERO_CD_Q14: [i32; 5] = {arr(stage[1]['aero_cd_q14'])};")
    lines.append(f"pub const STAGE_INERTIA_WET_Q12: [[i32; 3]; 2] = [{arr(stage[0]['inertia_wet_q12'])}, {arr(stage[1]['inertia_wet_q12'])}];")
    lines.append(f"pub const STAGE_INERTIA_DRY_Q12: [[i32; 3]; 2] = [{arr(stage[0]['inertia_dry_q12'])}, {arr(stage[1]['inertia_dry_q12'])}];")
    lines.append(f"pub const STAGE_INERTIA_HALF_Q12: [[i32; 3]; 2] = [{arr(data['component_vectors']['stage_inertia_half_q12'][0])}, {arr(data['component_vectors']['stage_inertia_half_q12'][1])}];")
    lines.append("")
    return "\n".join(lines)


def write_or_check(path: Path, payload: bytes, check: bool) -> None:
    if check:
        if not path.exists() or path.read_bytes() != payload:
            raise SystemExit(f"stale generated artifact: {path.relative_to(ROOT)}")
        return
    path.write_bytes(payload)
    digest = hashlib.sha256(payload).hexdigest()
    path.with_suffix(path.suffix + ".sha256").write_text(f"{digest}  {path.name}\n", encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    data = build()
    write_or_check(JSON_OUT, (json.dumps(data, indent=2) + "\n").encode(), args.check)
    write_or_check(RUST_OUT, rust(data).encode(), args.check)


if __name__ == "__main__":
    main()