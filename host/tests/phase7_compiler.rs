use ksa64_host::phase7_compiler::compile_sources;

const VEHICLE_SOURCE: &[u8] = include_bytes!("../../phase7/source-data/firestorm54.json");
const MOTOR_SOURCE: &[u8] = include_bytes!("../../phase7/source-data/aerotech-i211w.json");
const MISSION_SOURCE: &[u8] =
    include_bytes!("../../phase7/source-data/firestorm-i211-mission.json");
const VEHICLE_PACK: &[u8] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const MOTOR_PACK: &[u8] = include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const MISSION_PACK: &[u8] = include_bytes!("../../phase7/examples/firestorm-i211.kmc7");

#[test]
fn canonical_sources_rebuild_checked_in_packs_exactly() {
    let compiled = compile_sources(VEHICLE_SOURCE, MOTOR_SOURCE, MISSION_SOURCE).unwrap();
    assert_eq!(compiled.vehicle.as_slice(), VEHICLE_PACK);
    assert_eq!(compiled.motor.as_slice(), MOTOR_PACK);
    assert_eq!(compiled.mission.as_slice(), MISSION_PACK);
}

#[test]
fn identity_mismatch_fails_closed() {
    let broken = String::from_utf8(MISSION_SOURCE.to_vec())
        .unwrap()
        .replace("giant-leap-firestorm-54-dual-v1", "another-vehicle");
    assert!(compile_sources(VEHICLE_SOURCE, MOTOR_SOURCE, broken.as_bytes()).is_err());
}
