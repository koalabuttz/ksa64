use ksa64_core::mission::VERTICAL_CHECKSUM_OFFSET;
use ksa64_core::quantities::{Acceleration, Altitude, Mass, Time, Velocity};
use ksa64_core::scenario::parse_scenario_image;
use ksa64_core::telemetry::{
    write_telemetry_frame, write_telemetry_header, TelemetryEvents, TelemetryFrame,
    TelemetryStatus, TelemetryWriteError, TELEMETRY_FRAME_LENGTH, TELEMETRY_HEADER_LENGTH,
};
use ksa64_core::vehicle::VerticalTruthState;

const SCENARIO_IMAGE: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");
const GOLDEN_STREAM: &[u8; 112] = include_bytes!("../../phase0/numeric/telemetry-v1.bin");

#[test]
fn header_matches_independent_golden_bytes() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut output = [0u8; TELEMETRY_HEADER_LENGTH];
    write_telemetry_header(&scenario, &mut output).unwrap();
    assert_eq!(output, GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH]);
}

#[test]
fn initial_truth_frame_matches_independent_golden_bytes() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let truth = VerticalTruthState::initial(&scenario);
    let frame = TelemetryFrame::from_truth(
        truth,
        TelemetryStatus::from_engine_active(true),
        TelemetryEvents::NONE,
        VERTICAL_CHECKSUM_OFFSET,
    );
    let mut output = [0u8; TELEMETRY_FRAME_LENGTH];
    write_telemetry_frame(&frame, &mut output).unwrap();
    assert_eq!(
        output,
        GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH..TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH]
    );
}

#[test]
fn explicit_frame_matches_second_independent_golden_frame() {
    let frame = TelemetryFrame::new(
        8,
        Time::from_raw(65_536),
        Altitude::from_raw(16),
        Velocity::from_raw(134_218),
        Acceleration::from_raw(2_147_484),
        Mass::from_raw(2_037_760),
        Mass::from_raw(1_546_240),
        TelemetryStatus::from_engine_active(true),
        TelemetryEvents::NONE,
        0x1234_5678,
    );
    let mut output = [0u8; TELEMETRY_FRAME_LENGTH];
    write_telemetry_frame(&frame, &mut output).unwrap();
    assert_eq!(
        output,
        GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH..]
    );
}

#[test]
fn writers_reject_noncanonical_buffer_lengths() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut short_header = [0u8; TELEMETRY_HEADER_LENGTH - 1];
    let mut long_header = [0u8; TELEMETRY_HEADER_LENGTH + 1];
    assert_eq!(
        write_telemetry_header(&scenario, &mut short_header),
        Err(TelemetryWriteError::Length)
    );
    assert_eq!(
        write_telemetry_header(&scenario, &mut long_header),
        Err(TelemetryWriteError::Length)
    );
    assert!(short_header.iter().all(|byte| *byte == 0));
    assert!(long_header.iter().all(|byte| *byte == 0));

    let frame = TelemetryFrame::new(
        0,
        Time::ZERO,
        Altitude::ZERO,
        Velocity::ZERO,
        Acceleration::ZERO,
        Mass::ZERO,
        Mass::ZERO,
        TelemetryStatus::CLEAR,
        TelemetryEvents::NONE,
        0,
    );
    let mut short_frame = [0u8; TELEMETRY_FRAME_LENGTH - 1];
    let mut long_frame = [0u8; TELEMETRY_FRAME_LENGTH + 1];
    assert_eq!(
        write_telemetry_frame(&frame, &mut short_frame),
        Err(TelemetryWriteError::Length)
    );
    assert_eq!(
        write_telemetry_frame(&frame, &mut long_frame),
        Err(TelemetryWriteError::Length)
    );
    assert!(short_frame.iter().all(|byte| *byte == 0));
    assert!(long_frame.iter().all(|byte| *byte == 0));
}

#[test]
fn status_and_event_bits_follow_the_v1_contract() {
    assert_eq!(TelemetryStatus::from_engine_active(true).bits(), 0x0001);
    assert_eq!(TelemetryStatus::from_engine_active(false).bits(), 0);
    assert_eq!(TelemetryEvents::NONE.bits(), 0);
    assert_eq!(TelemetryEvents::new(true, true, true, true).bits(), 0x000f);
}
