#!/usr/bin/env python3
"""Independent float64 audit of frozen Phase 5 mission summaries."""
from __future__ import annotations
import argparse, hashlib, json, math, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EARTH_RADIUS = 6378.137
MU = 398600.4418
TARGET_INCLINATION = 51.6
RAW = {
    "nominal": {"outcome":"stable-orbit","steps":3133,"position_q12":[21468577,3871182,15698368],"velocity_q24":[-66327286,89767125,68337641],"max_q_q16":2861000,"max_aoa_sine_q16":13229,"max_flexible_q24":52314,"max_nav_position_error_q12":2781,"events":7,"sensor_checksum":1741708362,"navigation_checksum":2996014246,"flight_checksum":4068248986,"summary_checksum":557491580},
    "gust-and-slosh": {"outcome":"stable-orbit","steps":3133,"position_q12":[21468593,3872784,15697224],"velocity_q24":[-66330099,89790871,68316698],"max_q_q16":2861000,"max_aoa_sine_q16":13237,"max_flexible_q24":82713,"max_nav_position_error_q12":2784,"events":7,"sensor_checksum":3542390402,"navigation_checksum":1337884635,"flight_checksum":2646335657,"summary_checksum":977608682},
    "star-outage-gyro-bias": {"outcome":"stable-orbit","steps":3133,"position_q12":[21439624,3874063,15696788],"velocity_q24":[-66856332,89792380,68257395],"max_q_q16":2863250,"max_aoa_sine_q16":12892,"max_flexible_q24":89293,"max_nav_position_error_q12":2797,"events":7,"sensor_checksum":3286636991,"navigation_checksum":3622018424,"flight_checksum":2619625099,"summary_checksum":1801330793},
    "gimbal-jam": {"outcome":"aborted","steps":1103,"position_q12":[23048344,447218,12786253],"velocity_q24":[3694929,27797600,28827133],"max_q_q16":2861000,"max_aoa_sine_q16":18126,"max_flexible_q24":52627,"max_nav_position_error_q12":1055,"events":515,"sensor_checksum":176115100,"navigation_checksum":4084006702,"flight_checksum":478568845,"summary_checksum":3230120338},
    "damping-loss": {"outcome":"aborted","steps":958,"position_q12":[23031841,337557,12677055],"velocity_q24":[3409638,21993631,20807948],"max_q_q16":2861750,"max_aoa_sine_q16":12795,"max_flexible_q24":5278534,"max_nav_position_error_q12":785,"events":3,"sensor_checksum":1291516690,"navigation_checksum":182194722,"flight_checksum":1987933410,"summary_checksum":3522370491},
    "rcs-leak-depletion": {"outcome":"stable-orbit","steps":3133,"position_q12":[21500608,3882452,15708085],"velocity_q24":[-65365693,90145430,68829367],"max_q_q16":2861000,"max_aoa_sine_q16":13995,"max_flexible_q24":52494,"max_nav_position_error_q12":2799,"events":263,"sensor_checksum":3836379467,"navigation_checksum":181928900,"flight_checksum":791787182,"summary_checksum":2173775322},
}


def orbit(raw):
    rvec=[v/4096.0 for v in raw["position_q12"]]; vvec=[v/2**24 for v in raw["velocity_q24"]]
    radius=math.sqrt(sum(v*v for v in rvec)); speed2=sum(v*v for v in vvec)
    h=[rvec[1]*vvec[2]-rvec[2]*vvec[1], rvec[2]*vvec[0]-rvec[0]*vvec[2], rvec[0]*vvec[1]-rvec[1]*vvec[0]]
    hmag=math.sqrt(sum(v*v for v in h)); energy=0.5*speed2-MU/radius
    eccentricity=math.sqrt(max(0.0,1.0+2.0*energy*hmag*hmag/(MU*MU)))
    semi_major=-MU/(2.0*energy); inclination=math.degrees(math.acos(h[2]/hmag))
    radial_velocity=sum(a*b for a,b in zip(rvec,vvec))/radius
    return {"terminal_altitude_km":radius-EARTH_RADIUS,"radial_velocity_km_s":radial_velocity,
        "eccentricity":eccentricity,"perigee_km":semi_major*(1.0-eccentricity)-EARTH_RADIUS,
        "apogee_km":semi_major*(1.0+eccentricity)-EARTH_RADIUS,"inclination_deg":inclination}


def build():
    cases={}
    for name, raw in RAW.items():
        item={"raw":raw,"max_dynamic_pressure_kpa":raw["max_q_q16"]/65536.0,
              "max_angle_of_attack_deg":math.degrees(math.asin(min(1.0,raw["max_aoa_sine_q16"]/65536.0))),
              "max_flexible_state":raw["max_flexible_q24"]/2**24,
              "max_navigation_position_error_km":raw["max_nav_position_error_q12"]/4096.0}
        if raw["outcome"]=="stable-orbit": item["independent_float64_orbit"]=orbit(raw)
        cases[name]=item
    nominal_names=("nominal","gust-and-slosh")
    acceptance={
        "nominal_and_gust_apses_180_to_220_km":all(180.0 <= cases[n]["independent_float64_orbit"][k] <= 220.0 for n in nominal_names for k in ("perigee_km","apogee_km")),
        "nominal_and_gust_inclination_within_0_2_deg":all(abs(cases[n]["independent_float64_orbit"]["inclination_deg"]-TARGET_INCLINATION)<=0.2 for n in nominal_names),
        "sensor_outage_remains_stable":cases["star-outage-gyro-bias"]["independent_float64_orbit"]["perigee_km"]>=120.0,
        "reviewed_faults_abort":all(cases[n]["raw"]["outcome"]=="aborted" for n in ("gimbal-jam","damping-loss")),
        "rcs_leak_depletes_and_remains_stable":bool(cases["rcs-leak-depletion"]["raw"]["events"] & 256) and cases["rcs-leak-depletion"]["independent_float64_orbit"]["perigee_km"]>=120.0,
        "loads_bounded":all(cases[n]["max_dynamic_pressure_kpa"]<=60.0 and cases[n]["max_angle_of_attack_deg"]<=15.0 for n in ("nominal","gust-and-slosh","star-outage-gyro-bias")),
        "navigation_position_error_below_1_km":all(cases[n]["max_navigation_position_error_km"]<=1.0 for n in nominal_names+("star-outage-gyro-bias",)),
    }
    return {"model":"independent Python float64 orbit audit of frozen Phase 5 raw mission summaries",
            "constants":{"earth_radius_km":EARTH_RADIUS,"mu_km3_s2":MU,"target_inclination_deg":TARGET_INCLINATION},
            "cases":cases,"acceptance":acceptance,"all_acceptance_passed":all(acceptance.values())}


def main():
    parser=argparse.ArgumentParser(); parser.add_argument("--check",action="store_true"); args=parser.parse_args()
    output=ROOT/"phase5/mission-reference-v1.json"; sha=output.with_name(output.name+".sha256")
    data=(json.dumps(build(),indent=2)+"\n").encode(); digest=(hashlib.sha256(data).hexdigest()+"  mission-reference-v1.json\n").encode()
    if args.check:
        if not output.exists() or output.read_bytes()!=data or not sha.exists() or sha.read_bytes()!=digest:
            print("Phase 5 mission evidence is stale",file=sys.stderr); return 1
        if not build()["all_acceptance_passed"]: print("Phase 5 acceptance failed",file=sys.stderr); return 1
        print("Phase 5 independent mission evidence is current"); return 0
    output.write_bytes(data); sha.write_bytes(digest); print(output.relative_to(ROOT)); return 0


if __name__=="__main__": raise SystemExit(main())