use std::io::{self, Write};

use ksa64_core::scenario::{crc32_ieee, parse_scenario_image};
use ksa64_core::telemetry::{
    TelemetryEvents, TelemetryMissionFailure, TelemetrySink, TELEMETRY_FRAME_LENGTH,
    TELEMETRY_HEADER_LENGTH,
};
use ksa64_host::{
    capture_mission, format_inspection, inspect_stream, StreamInspectionError, WriterTelemetrySink,
};

const SCENARIO_IMAGE: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

#[allow(dead_code)]
mod expected {
    include!("../../phase1/generated/mission_v1.rs");
}

fn capture() -> Vec<u8> {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut stream = Vec::new();
    let summary = capture_mission(&scenario, &mut stream).unwrap();
    assert_eq!(summary.frames_written(), expected::TELEMETRY_FRAME_COUNT);
    stream
}

fn repair_frame_crc(stream: &mut [u8], index: usize) {
    let start = TELEMETRY_HEADER_LENGTH + index * TELEMETRY_FRAME_LENGTH;
    let checksum = crc32_ieee(&stream[start..start + 36]);
    stream[start + 36..start + 40].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn captured_golden_stream_passes_strict_inspection() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let stream = capture();
    let inspection = inspect_stream(&stream, &scenario).unwrap();

    assert_eq!(
        inspection.frame_count(),
        expected::TELEMETRY_FRAME_COUNT as usize
    );
    assert_eq!(inspection.stream_bytes(), expected::TELEMETRY_STREAM_LENGTH);
    assert_eq!(inspection.stream_crc32(), expected::TELEMETRY_STREAM_CRC32);
    assert_eq!(inspection.first_frame().step(), 0);
    assert_eq!(inspection.final_frame().step(), expected::FINAL_STEP);
    assert_eq!(
        inspection.final_frame().state_checksum(),
        expected::FINAL_CHECKSUM
    );
    assert_eq!(inspection.cutoff_event_frames(), 1);
    assert_eq!(inspection.depletion_event_frames(), 1);
    assert_eq!(inspection.numeric_fault_event_frames(), 0);

    let text = format_inspection(inspection);
    assert!(text.contains("frames         257"));
    assert!(text.contains("altitude       379.750244 km"));
    assert!(text.contains("state checksum 0x72bf6e0e"));
}

#[test]
fn stream_framing_and_record_crc_fail_closed() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut stream = capture();
    stream.push(0);
    assert_eq!(
        inspect_stream(&stream, &scenario),
        Err(StreamInspectionError::Framing)
    );

    let mut stream = capture();
    stream[TELEMETRY_HEADER_LENGTH + 8] ^= 1;
    assert_eq!(
        inspect_stream(&stream, &scenario),
        Err(StreamInspectionError::Frame {
            index: 0,
            cause: ksa64_core::telemetry::TelemetryReadError::Checksum,
        })
    );
}

#[test]
fn stream_semantics_reject_bad_stride_time_and_terminal_placement() {
    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();

    let mut bad_initial = capture();
    bad_initial[TELEMETRY_HEADER_LENGTH + 8] ^= 1;
    repair_frame_crc(&mut bad_initial, 0);
    assert_eq!(
        inspect_stream(&bad_initial, &scenario),
        Err(StreamInspectionError::InitialFrame)
    );

    let mut bad_stride = capture();
    let second = TELEMETRY_HEADER_LENGTH + TELEMETRY_FRAME_LENGTH;
    bad_stride[second..second + 4].copy_from_slice(&9u32.to_le_bytes());
    bad_stride[second + 4..second + 8]
        .copy_from_slice(&(9i32 * scenario.timestep().raw()).to_le_bytes());
    repair_frame_crc(&mut bad_stride, 1);
    assert_eq!(
        inspect_stream(&bad_stride, &scenario),
        Err(StreamInspectionError::Stride { index: 1 })
    );

    let mut bad_time = capture();
    bad_time[second + 4] ^= 1;
    repair_frame_crc(&mut bad_time, 1);
    assert_eq!(
        inspect_stream(&bad_time, &scenario),
        Err(StreamInspectionError::MissionTime { index: 1 })
    );

    let mut missing_end = capture();
    let final_index = expected::TELEMETRY_FRAME_COUNT as usize - 1;
    let final_start = TELEMETRY_HEADER_LENGTH + final_index * TELEMETRY_FRAME_LENGTH;
    missing_end[final_start + 30..final_start + 32]
        .copy_from_slice(&TelemetryEvents::NONE.bits().to_le_bytes());
    repair_frame_crc(&mut missing_end, final_index);
    assert_eq!(
        inspect_stream(&missing_end, &scenario),
        Err(StreamInspectionError::MissingTerminal)
    );

    let mut after_end = capture();
    let final_frame = after_end[final_start..final_start + TELEMETRY_FRAME_LENGTH].to_vec();
    after_end.extend_from_slice(&final_frame);
    assert_eq!(
        inspect_stream(&after_end, &scenario),
        Err(StreamInspectionError::TerminalBeforeEnd { index: final_index })
    );
}

#[test]
fn terminal_numeric_fault_stream_preserves_and_repeats_last_valid_truth() {
    let mut image = *SCENARIO_IMAGE;
    image[36..40].copy_from_slice(&134_217_728i32.to_le_bytes());
    let checksum = crc32_ieee(&image[..72]);
    image[72..76].copy_from_slice(&checksum.to_le_bytes());
    let scenario = parse_scenario_image(&image).unwrap();
    let mut stream = Vec::new();
    match capture_mission(&scenario, &mut stream).unwrap_err() {
        TelemetryMissionFailure::Simulation {
            fault_frame_written,
            frames_written,
            ..
        } => {
            assert!(fault_frame_written);
            assert_eq!(frames_written, 2);
        }
        other => panic!("unexpected fault capture error: {other:?}"),
    }

    let inspection = inspect_stream(&stream, &scenario).unwrap();
    assert_eq!(inspection.frame_count(), 2);
    assert_eq!(inspection.first_frame().step(), 0);
    assert_eq!(inspection.final_frame().step(), 0);
    assert_eq!(inspection.numeric_fault_event_frames(), 1);
    assert_eq!(
        inspection.final_frame().events().bits(),
        TelemetryEvents::NUMERIC_FAULT | TelemetryEvents::END_OF_RUN
    );
}
struct RefusingWriter {
    accepted: usize,
    limit: usize,
}

impl Write for RefusingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.accepted >= self.limit {
            return Err(io::Error::other("refused"));
        }
        let count = buffer.len().min(self.limit - self.accepted);
        self.accepted += count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn writer_sink_counts_accepted_records_and_propagates_io_failure() {
    let mut output = Vec::new();
    let mut sink = WriterTelemetrySink::new(&mut output);
    sink.write_header(&[0; TELEMETRY_HEADER_LENGTH]).unwrap();
    sink.write_frame(&[0; TELEMETRY_FRAME_LENGTH]).unwrap();
    assert_eq!(sink.frames_written(), 1);
    assert_eq!(sink.bytes_written(), 72);

    let scenario = parse_scenario_image(SCENARIO_IMAGE).unwrap();
    let mut writer = RefusingWriter {
        accepted: 0,
        limit: TELEMETRY_HEADER_LENGTH,
    };
    match capture_mission(&scenario, &mut writer).unwrap_err() {
        TelemetryMissionFailure::Frame { frames_written, .. } => {
            assert_eq!(frames_written, 0);
        }
        other => panic!("unexpected capture error: {other:?}"),
    }
}
