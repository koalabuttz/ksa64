use ksa64_core::phase2_mission::{EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_SEPARATION};
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::phase2_telemetry::{
    parse_phase2_telemetry_frame, parse_phase2_telemetry_header_for_scenario,
    run_phase2_mission_with_telemetry, Phase2TelemetryReadError, Phase2TelemetrySink,
    PHASE2_TELEMETRY_FRAME_LENGTH, PHASE2_TELEMETRY_HEADER_LENGTH,
};
use ksa64_core::scenario::crc32_ieee;

const NOMINAL: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

#[derive(Default)]
struct VecSink {
    bytes: Vec<u8>,
    frames: u32,
}

impl Phase2TelemetrySink for VecSink {
    type Error = ();
    fn write_header(
        &mut self,
        header: &[u8; PHASE2_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        self.bytes.extend_from_slice(header);
        Ok(())
    }
    fn write_frame(
        &mut self,
        frame: &[u8; PHASE2_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error> {
        self.bytes.extend_from_slice(frame);
        self.frames += 1;
        Ok(())
    }
}

fn capture() -> (ksa64_core::phase2_scenario::Phase2Scenario, VecSink) {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let mut sink = VecSink::default();
    let summary = run_phase2_mission_with_telemetry(&scenario, &mut sink).unwrap();
    assert_eq!(summary.frames_written(), sink.frames);
    assert_eq!(summary.mission().truth().step(), 7_200);
    (scenario, sink)
}

#[test]
fn nominal_kst2_stream_has_canonical_cadence_events_and_identity() {
    let (scenario, sink) = capture();
    assert_eq!(sink.frames, 901);
    assert_eq!(sink.bytes.len(), 57_704);
    let header = parse_phase2_telemetry_header_for_scenario(
        &sink.bytes[..PHASE2_TELEMETRY_HEADER_LENGTH],
        &scenario,
    )
    .unwrap();
    assert_eq!(header.mission_steps(), 7_200);

    let mut prior = None;
    let mut ignition = 0;
    let mut cutoff = 0;
    let mut separation = 0;
    for index in 0..sink.frames as usize {
        let start = PHASE2_TELEMETRY_HEADER_LENGTH + index * PHASE2_TELEMETRY_FRAME_LENGTH;
        let frame =
            parse_phase2_telemetry_frame(&sink.bytes[start..start + PHASE2_TELEMETRY_FRAME_LENGTH])
                .unwrap();
        if let Some(step) = prior {
            assert!(frame.step() > step);
        }
        if index + 1 != sink.frames as usize {
            assert_eq!(frame.step() % 8, 0);
        }
        ignition += u32::from(frame.events() & EVENT_IGNITION != 0);
        cutoff += u32::from(frame.events() & EVENT_CUTOFF != 0);
        separation += u32::from(frame.events() & EVENT_SEPARATION != 0);
        prior = Some(frame.step());
    }
    let final_start = sink.bytes.len() - PHASE2_TELEMETRY_FRAME_LENGTH;
    let final_frame = parse_phase2_telemetry_frame(&sink.bytes[final_start..]).unwrap();
    eprintln!(
        "KST2 bytes={} crc=0x{:08x} final_checksum=0x{:08x}",
        sink.bytes.len(),
        crc32_ieee(&sink.bytes),
        final_frame.state_checksum(),
    );
    assert_eq!(final_frame.step(), 7_200);
    assert_eq!(final_frame.events() & EVENT_END, EVENT_END);
    assert_eq!(ignition, 1);
    assert_eq!(cutoff, 2);
    assert_eq!(separation, 1);
}

#[test]
fn kst2_header_and_frame_corruption_fail_closed() {
    let (scenario, mut sink) = capture();
    sink.bytes[16] ^= 1;
    assert_eq!(
        parse_phase2_telemetry_header_for_scenario(
            &sink.bytes[..PHASE2_TELEMETRY_HEADER_LENGTH],
            &scenario,
        ),
        Err(Phase2TelemetryReadError::Checksum)
    );

    let (_, mut sink) = capture();
    sink.bytes[PHASE2_TELEMETRY_HEADER_LENGTH + 8] ^= 1;
    assert_eq!(
        parse_phase2_telemetry_frame(
            &sink.bytes[PHASE2_TELEMETRY_HEADER_LENGTH
                ..PHASE2_TELEMETRY_HEADER_LENGTH + PHASE2_TELEMETRY_FRAME_LENGTH],
        ),
        Err(Phase2TelemetryReadError::Checksum)
    );
    let (_, sink) = capture();
    let mut bad_status = [0u8; PHASE2_TELEMETRY_FRAME_LENGTH];
    bad_status.copy_from_slice(
        &sink.bytes[PHASE2_TELEMETRY_HEADER_LENGTH
            ..PHASE2_TELEMETRY_HEADER_LENGTH + PHASE2_TELEMETRY_FRAME_LENGTH],
    );
    bad_status[52..54].copy_from_slice(&0u16.to_le_bytes());
    let checksum = crc32_ieee(&bad_status[..60]);
    bad_status[60..64].copy_from_slice(&checksum.to_le_bytes());
    assert_eq!(
        parse_phase2_telemetry_frame(&bad_status),
        Err(Phase2TelemetryReadError::StatusBits)
    );
}
