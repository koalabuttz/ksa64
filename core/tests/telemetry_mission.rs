use ksa64_core::dynamics::VerticalStepError;
use ksa64_core::mission::VERTICAL_CHECKSUM_OFFSET;
use ksa64_core::numeric::NumericFault;
use ksa64_core::scenario::{crc32_ieee, parse_scenario_image};
use ksa64_core::telemetry::{
    run_vertical_mission_with_telemetry, TelemetryMissionFailure, TelemetrySink,
    TELEMETRY_FRAME_LENGTH, TELEMETRY_HEADER_LENGTH,
};

const SCENARIO_IMAGE: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

#[allow(dead_code)]
mod expected {
    include!("../../phase1/generated/mission_v1.rs");
}

#[derive(Debug, PartialEq, Eq)]
enum SinkError {
    Refused,
}

#[derive(Default)]
struct RecordingSink {
    bytes: Vec<u8>,
    writes: usize,
    fail_at: Option<usize>,
}

impl RecordingSink {
    fn failing_at(write: usize) -> Self {
        Self {
            fail_at: Some(write),
            ..Self::default()
        }
    }

    fn accept(&mut self, bytes: &[u8]) -> Result<(), SinkError> {
        if self.fail_at == Some(self.writes) {
            return Err(SinkError::Refused);
        }
        self.bytes.extend_from_slice(bytes);
        self.writes += 1;
        Ok(())
    }
}

impl TelemetrySink for RecordingSink {
    type Error = SinkError;

    fn write_header(&mut self, header: &[u8; TELEMETRY_HEADER_LENGTH]) -> Result<(), Self::Error> {
        self.accept(header)
    }

    fn write_frame(&mut self, frame: &[u8; TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error> {
        self.accept(frame)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn frame(stream: &[u8], index: usize) -> &[u8] {
    let start = TELEMETRY_HEADER_LENGTH + index * TELEMETRY_FRAME_LENGTH;
    &stream[start..start + TELEMETRY_FRAME_LENGTH]
}

fn repair_crc(image: &mut [u8; 76]) {
    let checksum = crc32_ieee(&image[..72]);
    image[72..76].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn golden_stream_matches_the_independent_schedule_oracle() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut sink = RecordingSink::default();
    let summary = run_vertical_mission_with_telemetry(&scenario, &mut sink).unwrap();

    assert_eq!(summary.frames_written(), expected::TELEMETRY_FRAME_COUNT);
    assert_eq!(summary.mission().checksum(), expected::FINAL_CHECKSUM);
    assert_eq!(summary.mission().cutoff_events(), expected::CUTOFF_EVENTS);
    assert_eq!(sink.bytes.len(), expected::TELEMETRY_STREAM_LENGTH);
    assert_eq!(crc32_ieee(&sink.bytes), expected::TELEMETRY_STREAM_CRC32);

    let initial = frame(&sink.bytes, 0);
    assert_eq!(read_u32(initial, 0), 0);
    assert_eq!(read_u16(initial, 30), 0);
    assert_eq!(read_u32(initial, 32), VERTICAL_CHECKSUM_OFFSET);

    let cutoff_index = (expected::CUTOFF_FRAME_STEP / scenario.telemetry_stride() as u32) as usize;
    let cutoff = frame(&sink.bytes, cutoff_index);
    assert_eq!(read_u32(cutoff, 0), expected::CUTOFF_FRAME_STEP);
    assert_eq!(read_u16(cutoff, 28), 0);
    assert_eq!(read_u16(cutoff, 30), expected::CUTOFF_FRAME_EVENTS);
    assert_eq!(read_u32(cutoff, 32), expected::CUTOFF_FRAME_CHECKSUM);

    let final_frame = frame(&sink.bytes, summary.frames_written() as usize - 1);
    assert_eq!(read_u32(final_frame, 0), expected::FINAL_STEP);
    assert_eq!(read_u16(final_frame, 30), expected::FINAL_FRAME_EVENTS);
    assert_eq!(read_u32(final_frame, 32), expected::FINAL_CHECKSUM);
    assert_eq!(read_u32(final_frame, 36), expected::FINAL_FRAME_CRC32);
}

#[test]
fn events_wait_for_the_next_stride_frame_and_then_clear() {
    let mut image = *SCENARIO_IMAGE;
    image[24..26].copy_from_slice(&10u16.to_le_bytes());
    repair_crc(&mut image);
    let scenario = parse_scenario_image(&image).unwrap();
    let mut sink = RecordingSink::default();
    let summary = run_vertical_mission_with_telemetry(&scenario, &mut sink).unwrap();

    assert_eq!(summary.frames_written(), 206);
    let event = frame(&sink.bytes, 122);
    assert_eq!(read_u32(event, 0), 1220);
    assert_eq!(read_u16(event, 30), 0x0003);
    assert_eq!(read_u16(frame(&sink.bytes, 123), 30), 0);

    let final_frame = frame(&sink.bytes, 205);
    assert_eq!(read_u32(final_frame, 0), 2048);
    assert_eq!(read_u16(final_frame, 30), 0x0008);
}

#[test]
fn off_stride_end_is_emitted_once() {
    let mut image = *SCENARIO_IMAGE;
    image[20..24].copy_from_slice(&2047u32.to_le_bytes());
    image[24..26].copy_from_slice(&10u16.to_le_bytes());
    image[60..64].copy_from_slice(&(151i32 << 16).to_le_bytes());
    repair_crc(&mut image);
    let scenario = parse_scenario_image(&image).unwrap();
    let mut sink = RecordingSink::default();
    let summary = run_vertical_mission_with_telemetry(&scenario, &mut sink).unwrap();

    assert_eq!(summary.frames_written(), 206);
    assert_eq!(read_u32(frame(&sink.bytes, 204), 0), 2040);
    let final_frame = frame(&sink.bytes, 205);
    assert_eq!(read_u32(final_frame, 0), 2047);
    assert_eq!(read_u16(final_frame, 30) & 0x0008, 0x0008);
}

#[test]
fn sink_failures_stop_at_the_last_observed_truth() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut header_sink = RecordingSink::failing_at(0);
    assert_eq!(
        run_vertical_mission_with_telemetry(&scenario, &mut header_sink),
        Err(TelemetryMissionFailure::Header(SinkError::Refused))
    );

    let mut initial_sink = RecordingSink::failing_at(1);
    match run_vertical_mission_with_telemetry(&scenario, &mut initial_sink).unwrap_err() {
        TelemetryMissionFailure::Frame {
            error,
            last_truth,
            checksum,
            frames_written,
            ..
        } => {
            assert_eq!(error, SinkError::Refused);
            assert_eq!(last_truth.step(), 0);
            assert_eq!(checksum, VERTICAL_CHECKSUM_OFFSET);
            assert_eq!(frames_written, 0);
        }
        other => panic!("unexpected failure: {other:?}"),
    }

    let mut middle_sink = RecordingSink::failing_at(3);
    match run_vertical_mission_with_telemetry(&scenario, &mut middle_sink).unwrap_err() {
        TelemetryMissionFailure::Frame {
            last_truth,
            frames_written,
            ..
        } => {
            assert_eq!(last_truth.step(), 16);
            assert_eq!(frames_written, 2);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
}

#[test]
fn numeric_fault_emits_a_terminal_fault_frame_at_last_valid_truth() {
    let mut image = *SCENARIO_IMAGE;
    image[36..40].copy_from_slice(&134_217_728i32.to_le_bytes());
    repair_crc(&mut image);
    let scenario = parse_scenario_image(&image).unwrap();
    let mut sink = RecordingSink::default();

    match run_vertical_mission_with_telemetry(&scenario, &mut sink).unwrap_err() {
        TelemetryMissionFailure::Simulation {
            failure,
            fault_frame_written,
            frames_written,
        } => {
            assert!(fault_frame_written);
            assert_eq!(frames_written, 2);
            assert_eq!(failure.cause(), VerticalStepError::NumericFault);
            assert!(failure
                .numeric_status()
                .contains(NumericFault::InvalidInput));
            assert_eq!(failure.last_truth().step(), 0);
            assert_eq!(failure.checksum(), VERTICAL_CHECKSUM_OFFSET);
        }
        other => panic!("unexpected failure: {other:?}"),
    }

    let fault = frame(&sink.bytes, 1);
    assert_eq!(read_u32(fault, 0), 0);
    assert_eq!(read_u16(fault, 30), 0x000c);
    assert_eq!(read_u32(fault, 32), VERTICAL_CHECKSUM_OFFSET);
}

#[test]
fn a_sink_fault_while_reporting_a_numeric_fault_preserves_both_causes() {
    let mut image = *SCENARIO_IMAGE;
    image[36..40].copy_from_slice(&134_217_728i32.to_le_bytes());
    repair_crc(&mut image);
    let scenario = parse_scenario_image(&image).unwrap();
    let mut sink = RecordingSink::failing_at(2);

    match run_vertical_mission_with_telemetry(&scenario, &mut sink).unwrap_err() {
        TelemetryMissionFailure::SimulationAndFrame {
            failure,
            error,
            frames_written,
        } => {
            assert_eq!(failure.cause(), VerticalStepError::NumericFault);
            assert_eq!(error, SinkError::Refused);
            assert_eq!(frames_written, 1);
        }
        other => panic!("unexpected failure: {other:?}"),
    }
}
