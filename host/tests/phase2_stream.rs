use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::phase2_telemetry::{PHASE2_TELEMETRY_FRAME_LENGTH, PHASE2_TELEMETRY_HEADER_LENGTH};
use ksa64_core::scenario::crc32_ieee;
use ksa64_host::phase2::{
    capture_phase2_mission, format_phase2_inspection, inspect_phase2_stream,
    Phase2StreamInspectionError,
};

const NOMINAL: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

fn capture() -> Vec<u8> {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let mut stream = Vec::new();
    let summary = capture_phase2_mission(&scenario, &mut stream).unwrap();
    assert_eq!(summary.frames_written(), 901);
    stream
}

fn repair_frame(stream: &mut [u8], index: usize) {
    let start = PHASE2_TELEMETRY_HEADER_LENGTH + index * PHASE2_TELEMETRY_FRAME_LENGTH;
    let crc = crc32_ieee(&stream[start..start + 60]);
    stream[start + 60..start + 64].copy_from_slice(&crc.to_le_bytes());
}

#[test]
fn captured_nominal_stream_passes_strict_host_inspection() {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let stream = capture();
    let inspection = inspect_phase2_stream(&stream, &scenario).unwrap();
    assert_eq!(inspection.frame_count(), 901);
    assert_eq!(inspection.stream_bytes(), 57_704);
    assert_eq!(inspection.stream_crc32(), 0x7d13_b2bf);
    assert_eq!(inspection.final_frame().step(), 7_200);
    assert_eq!(inspection.final_frame().state_checksum(), 0xcc57_612b);
    assert_eq!(inspection.ignition_event_frames(), 1);
    assert_eq!(inspection.cutoff_event_frames(), 2);
    assert_eq!(inspection.separation_event_frames(), 1);
    assert_eq!(inspection.impact_event_frames(), 0);
    let text = format_phase2_inspection(inspection);
    assert!(text.contains("KSA64 TELEMETRY V2"));
    assert!(text.contains("altitude       197.665527 km"));
    assert!(text.contains("state checksum 0xcc57612b"));
}

#[test]
fn host_inspector_rejects_bad_framing_cadence_time_and_terminal_state() {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let mut stream = capture();
    stream.push(0);
    assert_eq!(
        inspect_phase2_stream(&stream, &scenario),
        Err(Phase2StreamInspectionError::Framing)
    );

    let mut stream = capture();
    let second = PHASE2_TELEMETRY_HEADER_LENGTH + PHASE2_TELEMETRY_FRAME_LENGTH;
    stream[second..second + 4].copy_from_slice(&9u32.to_le_bytes());
    stream[second + 4..second + 8].copy_from_slice(&(9 * scenario.timestep().raw()).to_le_bytes());
    repair_frame(&mut stream, 1);
    assert_eq!(
        inspect_phase2_stream(&stream, &scenario),
        Err(Phase2StreamInspectionError::Stride { index: 1 })
    );

    let mut stream = capture();
    stream[second + 4] ^= 1;
    repair_frame(&mut stream, 1);
    assert_eq!(
        inspect_phase2_stream(&stream, &scenario),
        Err(Phase2StreamInspectionError::MissionTime { index: 1 })
    );

    let mut stream = capture();
    let final_index = 900;
    let final_start = PHASE2_TELEMETRY_HEADER_LENGTH + final_index * PHASE2_TELEMETRY_FRAME_LENGTH;
    stream[final_start + 54..final_start + 56].copy_from_slice(&0u16.to_le_bytes());
    repair_frame(&mut stream, final_index);
    assert_eq!(
        inspect_phase2_stream(&stream, &scenario),
        Err(Phase2StreamInspectionError::MissingTerminal)
    );
}
