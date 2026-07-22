use ksa64_core::mission::VERTICAL_CHECKSUM_OFFSET;
use ksa64_core::scenario::{crc32_ieee, parse_scenario_image, NUMERIC_CONTRACT_ID};
use ksa64_core::telemetry::{
    parse_telemetry_frame, parse_telemetry_header, parse_telemetry_header_for_scenario,
    TelemetryEvents, TelemetryReadError, TelemetryStatus, TELEMETRY_FRAME_LENGTH,
    TELEMETRY_HEADER_LENGTH,
};

const SCENARIO_IMAGE: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");
const GOLDEN_STREAM: &[u8; 112] = include_bytes!("../../phase0/numeric/telemetry-v1.bin");

fn repair_crc(record: &mut [u8], content_length: usize, checksum_offset: usize) {
    let checksum = crc32_ieee(&record[..content_length]);
    record[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn canonical_header_decodes_and_binds_to_its_scenario() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let bytes = &GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH];
    let header = parse_telemetry_header(bytes).unwrap();

    assert_eq!(header.numeric_contract_id(), NUMERIC_CONTRACT_ID);
    assert_eq!(header.scenario_id(), scenario.scenario_id());
    assert_eq!(header.timestep(), scenario.timestep());
    assert_eq!(header.telemetry_stride(), scenario.telemetry_stride());
    assert_eq!(
        parse_telemetry_header_for_scenario(bytes, &scenario),
        Ok(header)
    );
}

#[test]
fn canonical_frames_decode_to_strong_values() {
    let initial = parse_telemetry_frame(
        &GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH..TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH],
    )
    .unwrap();
    assert_eq!(initial.step(), 0);
    assert_eq!(initial.time().raw(), 0);
    assert_eq!(initial.status(), TelemetryStatus::from_engine_active(true));
    assert_eq!(initial.events(), TelemetryEvents::NONE);
    assert_eq!(initial.state_checksum(), VERTICAL_CHECKSUM_OFFSET);

    let second =
        parse_telemetry_frame(&GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH..])
            .unwrap();
    assert_eq!(second.step(), 8);
    assert_eq!(second.time().raw(), 65_536);
    assert_eq!(second.altitude().raw(), 16);
    assert_eq!(second.velocity().raw(), 134_218);
    assert_eq!(second.acceleration().raw(), 2_147_484);
    assert_eq!(second.total_mass().raw(), 2_037_760);
    assert_eq!(second.propellant().raw(), 1_546_240);
    assert_eq!(second.state_checksum(), 0x1234_5678);
}

#[test]
fn header_rejects_bad_framing_identity_and_reserved_fields() {
    assert_eq!(
        parse_telemetry_header(&GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH - 1]),
        Err(TelemetryReadError::Length)
    );

    let mut header = [0u8; TELEMETRY_HEADER_LENGTH];
    header.copy_from_slice(&GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH]);
    header[0] = b'X';
    repair_crc(&mut header, 28, 28);
    assert_eq!(
        parse_telemetry_header(&header),
        Err(TelemetryReadError::Magic)
    );

    header.copy_from_slice(&GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH]);
    header[10] = 1;
    repair_crc(&mut header, 28, 28);
    assert_eq!(
        parse_telemetry_header(&header),
        Err(TelemetryReadError::Reserved)
    );

    header.copy_from_slice(&GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH]);
    header[16] ^= 1;
    repair_crc(&mut header, 28, 28);
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    assert!(parse_telemetry_header(&header).is_ok());
    assert_eq!(
        parse_telemetry_header_for_scenario(&header, &scenario),
        Err(TelemetryReadError::ScenarioIdentity)
    );
}

#[test]
fn record_crc_failures_are_detected_before_values_escape() {
    let mut header = [0u8; TELEMETRY_HEADER_LENGTH];
    header.copy_from_slice(&GOLDEN_STREAM[..TELEMETRY_HEADER_LENGTH]);
    header[20] ^= 1;
    assert_eq!(
        parse_telemetry_header(&header),
        Err(TelemetryReadError::Checksum)
    );

    let mut frame = [0u8; TELEMETRY_FRAME_LENGTH];
    frame.copy_from_slice(
        &GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH..TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH],
    );
    frame[8] ^= 1;
    assert_eq!(
        parse_telemetry_frame(&frame),
        Err(TelemetryReadError::Checksum)
    );
}

#[test]
fn frames_reject_unknown_status_and_event_bits() {
    let source =
        &GOLDEN_STREAM[TELEMETRY_HEADER_LENGTH..TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH];
    let mut frame = [0u8; TELEMETRY_FRAME_LENGTH];
    frame.copy_from_slice(source);
    frame[28..30].copy_from_slice(&0x8001u16.to_le_bytes());
    repair_crc(&mut frame, 36, 36);
    assert_eq!(
        parse_telemetry_frame(&frame),
        Err(TelemetryReadError::StatusBits)
    );

    frame.copy_from_slice(source);
    frame[30..32].copy_from_slice(&0x8000u16.to_le_bytes());
    repair_crc(&mut frame, 36, 36);
    assert_eq!(
        parse_telemetry_frame(&frame),
        Err(TelemetryReadError::EventBits)
    );
}
