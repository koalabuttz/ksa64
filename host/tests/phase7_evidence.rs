use ksa64_core::evaluation::MetricSlot;
use ksa64_core::phase7_format::{
    KMC7_LENGTH, KMP7_LENGTH, KSR7_LENGTH, KST7_FRAME_LENGTH, KST7_HEADER_LENGTH, KVP7_LENGTH,
};
use ksa64_core::phase7_numeric::{
    HOBBY_ALTITUDE_FRACTIONAL_BITS, HOBBY_DYNAMIC_PRESSURE_FRACTIONAL_BITS,
    HOBBY_VELOCITY_FRACTIONAL_BITS,
};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_core::phase7_result::parse_ksr7;
use ksa64_core::phase7_telemetry::{parse_kst7_frame, parse_kst7_header};
use ksa64_host::phase7::capture_hobby_mission;
use ksa64_host::phase7_reference::analyze_hobby_mission_f64;

const VEHICLE_BYTES: &[u8; KVP7_LENGTH] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const MOTOR_BYTES: &[u8; KMP7_LENGTH] = include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const MISSION_BYTES: &[u8; KMC7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm-i211.kmc7");
const SUMMARY_BYTES: &[u8; KSR7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm-i211.ksr7");
const TELEMETRY_BYTES: &[u8] = include_bytes!("../../phase7/examples/firestorm-i211.kst7");

fn packs() -> (
    ksa64_core::phase7_pack::VerticalVehiclePack,
    ksa64_core::phase7_pack::MotorPack,
    ksa64_core::phase7_pack::HobbyMissionPack,
) {
    (
        parse_vehicle_pack(VEHICLE_BYTES).unwrap(),
        parse_motor_pack(MOTOR_BYTES).unwrap(),
        parse_mission_pack(MISSION_BYTES).unwrap(),
    )
}

fn relative_error(left: f64, right: f64) -> f64 {
    (left - right).abs() / right.abs().max(1.0)
}

#[test]
fn checked_in_artifacts_rebuild_and_every_frame_validates() {
    let (vehicle, motor, mission) = packs();
    let capture = capture_hobby_mission(vehicle, &motor, mission).unwrap();
    assert_eq!(capture.summary_record.as_slice(), SUMMARY_BYTES);
    assert_eq!(capture.telemetry.as_slice(), TELEMETRY_BYTES);
    let header = parse_kst7_header(&TELEMETRY_BYTES[..KST7_HEADER_LENGTH]).unwrap();
    assert_eq!(header.vehicle_identity, vehicle.identity);
    let frames = &TELEMETRY_BYTES[KST7_HEADER_LENGTH..];
    assert_eq!(frames.len() % KST7_FRAME_LENGTH, 0);
    let mut previous_step = None;
    for bytes in frames.chunks_exact(KST7_FRAME_LENGTH) {
        let frame = parse_kst7_frame(bytes).unwrap();
        if let Some(previous) = previous_step {
            assert!(frame.observation.state.step > previous);
        }
        previous_step = Some(frame.observation.state.step);
    }
    let summary = parse_ksr7(SUMMARY_BYTES).unwrap();
    assert_eq!(summary.summary.source_checksums[0], 0xa61c_5720);
}

#[test]
fn independent_float64_analysis_agrees_with_declared_tolerances() {
    let (vehicle, motor, mission) = packs();
    let exact = capture_hobby_mission(vehicle, &motor, mission)
        .unwrap()
        .evaluation;
    let reference = analyze_hobby_mission_f64(vehicle, &motor, mission);
    let exact_apogee = exact.metric(MetricSlot::ApogeeAltitude).unwrap() as f64
        / (1u64 << HOBBY_ALTITUDE_FRACTIONAL_BITS) as f64;
    let exact_impact = exact.metric(MetricSlot::ImpactVelocity).unwrap() as f64
        / (1u64 << HOBBY_VELOCITY_FRACTIONAL_BITS) as f64;
    let exact_q = exact.metric(MetricSlot::MaxDynamicPressure).unwrap() as f64
        / (1u64 << HOBBY_DYNAMIC_PRESSURE_FRACTIONAL_BITS) as f64;
    println!(
        "exact apogee={exact_apogee:.3} impact={exact_impact:.3} q={exact_q:.1}; f64={reference:?}"
    );
    assert!(relative_error(exact_apogee, reference.apogee_m) < 0.02);
    assert!((exact_impact - reference.impact_velocity_mps).abs() < 0.5);
    assert!(relative_error(exact_q, reference.max_dynamic_pressure_pa) < 0.05);
}

#[test]
fn artifact_corruption_is_rejected() {
    let mut summary = *SUMMARY_BYTES;
    summary[80] ^= 1;
    assert!(parse_ksr7(&summary).is_err());
    let mut frame: [u8; KST7_FRAME_LENGTH] = TELEMETRY_BYTES
        [KST7_HEADER_LENGTH..KST7_HEADER_LENGTH + KST7_FRAME_LENGTH]
        .try_into()
        .unwrap();
    frame[12] ^= 1;
    assert!(parse_kst7_frame(&frame).is_err());
}
