use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_pack::{
    packs_are_compatible, parse_mission_pack, parse_motor_pack, parse_vehicle_pack,
};

const VEHICLE: &[u8; KVP7_LENGTH] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const MOTOR: &[u8; KMP7_LENGTH] = include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const MISSION: &[u8; KMC7_LENGTH] = include_bytes!("../../phase7/examples/firestorm-i211.kmc7");

#[test]
fn canonical_packs_parse_and_bind_to_each_other() {
    let vehicle = parse_vehicle_pack(VEHICLE).unwrap();
    let motor = parse_motor_pack(MOTOR).unwrap();
    let mission = parse_mission_pack(MISSION).unwrap();
    assert!(packs_are_compatible(vehicle, &motor, mission));
    assert_eq!(motor.knot_count, 27);
    assert_eq!(motor.knots[0].time.raw(), 0);
    assert_eq!(motor.knots[26].time, motor.burn_time);
    assert_eq!(motor.knots[26].thrust_raw_q13, 0);
    assert_eq!(mission.rail_length.raw(), 2 * (1 << 13));
    assert_eq!(mission.main_deployment_altitude.raw(), 200 * (1 << 13));
}

#[test]
fn every_pack_rejects_corruption() {
    let mut vehicle = *VEHICLE;
    vehicle[100] ^= 1;
    assert!(parse_vehicle_pack(&vehicle).is_err());
    let mut motor = *MOTOR;
    motor[200] ^= 1;
    assert!(parse_motor_pack(&motor).is_err());
    let mut mission = *MISSION;
    mission[75] ^= 1;
    assert!(parse_mission_pack(&mission).is_err());
}

#[test]
fn unknown_payload_bytes_fail_reserved_checks() {
    use ksa64_core::phase7_format::seal_phase7_record;

    let mut vehicle = *VEHICLE;
    vehicle[300] = 1;
    seal_phase7_record(&mut vehicle).unwrap();
    assert!(parse_vehicle_pack(&vehicle).is_err());
    let mut motor = *MOTOR;
    motor[600] = 1;
    seal_phase7_record(&mut motor).unwrap();
    assert!(parse_motor_pack(&motor).is_err());
}
