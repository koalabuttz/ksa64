use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::phase2_telemetry::{
    parse_phase2_telemetry_frame, PHASE2_TELEMETRY_FRAME_LENGTH, PHASE2_TELEMETRY_HEADER_LENGTH,
};
use ksa64_core::phase5_contract::*;
use ksa64_interface::crc32_ieee;
use ksa64_sim::mission::{run_phase3_mission, MissionCase};

const SCENARIO: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const PHASE2_STREAM: &[u8] = include_bytes!("../../phase2/examples/ksa2a-200km.kst2");
const PHASE3_STREAM: &[u8] = include_bytes!("../../phase3/examples/ksa3-nominal.kst3");

#[test]
fn phase5_wrapper_preserves_nominal_phase3_checksum_chains() {
    let scenario = parse_phase2_scenario(SCENARIO).unwrap();
    let result = run_phase3_mission(&scenario, MissionCase::Nominal).unwrap();
    assert_eq!(result.truth_checksum, PHASE5_PLANAR_TRUTH_CHECKSUM);
    assert_eq!(result.sensor_checksum, PHASE5_PLANAR_SENSOR_CHECKSUM);
    assert_eq!(result.nav_checksum, PHASE5_PLANAR_NAV_CHECKSUM);
    assert_eq!(result.flight_checksum, PHASE5_PLANAR_FLIGHT_CHECKSUM);
}

#[test]
fn inherited_phase2_terminal_state_is_unchanged() {
    let frame_bytes = &PHASE2_STREAM[PHASE2_STREAM.len() - PHASE2_TELEMETRY_FRAME_LENGTH..];
    let frame = parse_phase2_telemetry_frame(frame_bytes).unwrap();
    assert_eq!(frame.state_checksum(), PHASE5_PLANAR_PHASE2_CHECKSUM);
    assert_eq!(crc32_ieee(PHASE2_STREAM), 0x7d13_b2bf);
    assert_eq!(
        (PHASE2_STREAM.len() - PHASE2_TELEMETRY_HEADER_LENGTH) % PHASE2_TELEMETRY_FRAME_LENGTH,
        0
    );
}

#[test]
fn inherited_phase3_canonical_stream_is_unchanged() {
    assert_eq!(crc32_ieee(PHASE3_STREAM), PHASE5_PLANAR_KST3_CRC32);
}
