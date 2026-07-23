use ksa64_host::phase5::{capture_phase5_mission, inspect_phase5_stream, Phase5StreamError};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase5_mission::Phase5MissionCase;
use ksa64_sim::phase5_telemetry::{
    Phase5TelemetryError, PHASE5_TELEMETRY_FRAME_LENGTH, PHASE5_TELEMETRY_HEADER_LENGTH,
};

#[test]
fn nominal_kst5_stream_matches_frozen_identity() {
    let (summary, bytes) = capture_phase5_mission(Phase5MissionCase::Nominal);
    let got = inspect_phase5_stream(&bytes).unwrap();
    assert_eq!(got.frame_count, 3134);
    assert_eq!(got.stream_bytes, 1_328_912);
    assert_eq!(got.stream_crc32, 0xa9b3_b94c);
    assert_eq!(got.event_frames, 1_964);
    assert_eq!(got.final_frame.step, summary.steps);
    assert_eq!(got.final_frame.position_q12, summary.terminal_position_q12);
    assert_eq!(got.final_frame.velocity_q24, summary.terminal_velocity_q24);
    assert_eq!(got.final_frame.sensor_checksum, summary.sensor_checksum);
    assert_eq!(
        got.final_frame.navigation_checksum,
        summary.navigation_checksum
    );
    assert_eq!(got.final_frame.flight_checksum, summary.flight_checksum);
    assert_eq!(got.final_frame.observation_checksum, 0x5b7b_2419);
}
#[test]
fn strict_inspector_rejects_framing_record_crc_and_chain_damage() {
    let (_, bytes) = capture_phase5_mission(Phase5MissionCase::Nominal);
    let mut truncated = bytes.clone();
    truncated.pop();
    assert_eq!(
        inspect_phase5_stream(&truncated),
        Err(Phase5StreamError::Framing)
    );
    let mut damaged = bytes.clone();
    damaged[PHASE5_TELEMETRY_HEADER_LENGTH + 44] ^= 1;
    assert!(matches!(
        inspect_phase5_stream(&damaged),
        Err(Phase5StreamError::Frame {
            index: 0,
            cause: Phase5TelemetryError::Checksum
        })
    ));
    let mut chain = bytes;
    let second = PHASE5_TELEMETRY_HEADER_LENGTH + PHASE5_TELEMETRY_FRAME_LENGTH;
    chain[second + 412] ^= 1;
    let crc = crc32_ieee(&chain[second..second + 420]);
    chain[second + 420..second + 424].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        inspect_phase5_stream(&chain),
        Err(Phase5StreamError::Checksum { index: 1 })
    );
}
