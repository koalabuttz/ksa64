use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_mission::{
    execute_hobby_mission, sample_motor_thrust, HobbyMissionOutcome, HOBBY_EVENT_APOGEE,
    HOBBY_EVENT_BURNOUT, HOBBY_EVENT_DROGUE, HOBBY_EVENT_END, HOBBY_EVENT_GROUND,
    HOBBY_EVENT_LIFTOFF, HOBBY_EVENT_MAIN, HOBBY_EVENT_RAIL_EXIT,
};
use ksa64_core::phase7_numeric::HobbyTime;
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};

const VEHICLE: &[u8; KVP7_LENGTH] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const MOTOR: &[u8; KMP7_LENGTH] = include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const MISSION: &[u8; KMC7_LENGTH] = include_bytes!("../../phase7/examples/firestorm-i211.kmc7");

fn packs() -> (
    ksa64_core::phase7_pack::VerticalVehiclePack,
    ksa64_core::phase7_pack::MotorPack,
    ksa64_core::phase7_pack::HobbyMissionPack,
) {
    (
        parse_vehicle_pack(VEHICLE).unwrap(),
        parse_motor_pack(MOTOR).unwrap(),
        parse_mission_pack(MISSION).unwrap(),
    )
}

#[test]
fn sampled_motor_curve_has_exact_endpoints_and_positive_burn() {
    let (_, motor, _) = packs();
    assert_eq!(sample_motor_thrust(&motor, HobbyTime::ZERO), 0);
    assert!(sample_motor_thrust(&motor, HobbyTime::from_raw(1 << 18)) > 0);
    assert_eq!(sample_motor_thrust(&motor, motor.burn_time), 0);
    assert_eq!(
        sample_motor_thrust(&motor, HobbyTime::from_raw(motor.burn_time.raw() + 1)),
        0
    );
}

#[test]
fn canonical_reference_reaches_apogee_and_recovers() {
    let (vehicle, motor, mission) = packs();
    let result = execute_hobby_mission(vehicle, &motor, mission).unwrap();
    assert_eq!(result.outcome, HobbyMissionOutcome::Landed);
    assert_eq!(result.numeric_faults, 0);
    let required = HOBBY_EVENT_LIFTOFF
        | HOBBY_EVENT_RAIL_EXIT
        | HOBBY_EVENT_BURNOUT
        | HOBBY_EVENT_APOGEE
        | HOBBY_EVENT_DROGUE
        | HOBBY_EVENT_MAIN
        | HOBBY_EVENT_GROUND
        | HOBBY_EVENT_END;
    assert_eq!(result.event_history & required, required);
    assert!(result.rail_exit.valid);
    assert!(result.burnout.valid);
    assert!(result.apogee.valid);
    assert!(result.drogue.valid);
    assert!(result.main.valid);
    assert!(result.ground.valid);
    assert!(result.max_altitude.raw() > 100 * (1 << 13));
    assert_eq!(result.terminal.altitude, mission.launch_altitude);
    assert!(result.ground.velocity_raw < 0);
    println!(
        "steps={} apogee_raw={} max_v={} max_a={} max_q={} max_mach={} impact_v={} checksum={:08x}",
        result.terminal.step,
        result.max_altitude.raw(),
        result.max_speed.raw(),
        result.max_acceleration.raw(),
        result.max_dynamic_pressure.raw(),
        result.max_mach.raw(),
        result.ground.velocity_raw,
        result.state_checksum
    );
}

#[test]
fn repeated_execution_is_exact() {
    let (vehicle, motor, mission) = packs();
    let first = execute_hobby_mission(vehicle, &motor, mission).unwrap();
    let second = execute_hobby_mission(vehicle, &motor, mission).unwrap();
    assert_eq!(first, second);
}
