use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_interface::crc32_ieee;
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::mission::MissionCase;
use ksa64_sim::telemetry::*;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const CONFIG: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");

#[derive(Default)]
struct VecSink(Vec<u8>);
impl Phase3TelemetrySink for VecSink {
    type Error = ();
    fn write_header(
        &mut self,
        bytes: &[u8; PHASE3_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn write_frame(
        &mut self,
        bytes: &[u8; PHASE3_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn canonical_stream_round_trips_with_terminal_and_split_checksums() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let mut sink = VecSink::default();
    let (result, frames) = run_phase3_mission_with_telemetry(
        &scenario,
        crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
        crc32_ieee(&CONFIG[..PHASE3_CONFIG_LENGTH - 4]),
        MissionCase::Nominal,
        &mut sink,
    )
    .unwrap();
    assert_eq!(
        sink.0.len(),
        PHASE3_TELEMETRY_HEADER_LENGTH + frames as usize * PHASE3_TELEMETRY_FRAME_LENGTH
    );
    let header = parse_phase3_telemetry_header(&sink.0[..PHASE3_TELEMETRY_HEADER_LENGTH]).unwrap();
    validate_phase3_header(
        header,
        &scenario,
        crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
        crc32_ieee(&CONFIG[..PHASE3_CONFIG_LENGTH - 4]),
        MissionCase::Nominal,
    )
    .unwrap();
    let mut prior = None;
    let mut final_frame = None;
    for index in 0..frames as usize {
        let start = PHASE3_TELEMETRY_HEADER_LENGTH + index * PHASE3_TELEMETRY_FRAME_LENGTH;
        let frame =
            parse_phase3_telemetry_frame(&sink.0[start..start + PHASE3_TELEMETRY_FRAME_LENGTH])
                .unwrap();
        if let Some(step) = prior {
            assert!(frame.step > step);
        } else {
            assert_eq!(frame.step, 0);
        }
        if !frame.event_record() && !frame.terminal() {
            assert_eq!(
                (frame.step / PHASE3_TELEMETRY_STRIDE as u32) * PHASE3_TELEMETRY_STRIDE as u32,
                frame.step
            );
        }
        if frame.terminal() {
            assert_eq!(index + 1, frames as usize);
        }
        prior = Some(frame.step);
        final_frame = Some(frame);
    }
    let final_frame = final_frame.unwrap();
    assert!(final_frame.terminal());
    assert_eq!(final_frame.truth_checksum, result.truth_checksum);
    assert_eq!(final_frame.sensor_checksum, result.sensor_checksum);
    assert_eq!(final_frame.nav_checksum, result.nav_checksum);
    assert_eq!(final_frame.flight_checksum, result.flight_checksum);
}

#[test]
fn header_frame_corruption_reserved_and_identity_fail_closed() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let header = Phase3TelemetryHeader {
        contract_id: PHASE3_TELEMETRY_CONTRACT_ID,
        scenario_id: scenario.scenario_id(),
        scenario_crc32: crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
        config_crc32: crc32_ieee(&CONFIG[..PHASE3_CONFIG_LENGTH - 4]),
        seed: MissionCase::Nominal.seed(),
        case: MissionCase::Nominal,
        timestep_q16: scenario.timestep().raw(),
        telemetry_stride: PHASE3_TELEMETRY_STRIDE,
        mission_steps: scenario.steps(),
    };
    let mut bytes = [0u8; PHASE3_TELEMETRY_HEADER_LENGTH];
    write_phase3_telemetry_header(header, &mut bytes).unwrap();
    let mut corrupt = bytes;
    corrupt[20] ^= 1;
    assert_eq!(
        parse_phase3_telemetry_header(&corrupt),
        Err(Phase3TelemetryError::Checksum)
    );
    let mut reserved = bytes;
    reserved[50] = 1;
    let crc = crc32_ieee(&reserved[..60]).to_le_bytes();
    reserved[60..64].copy_from_slice(&crc);
    assert_eq!(
        parse_phase3_telemetry_header(&reserved),
        Err(Phase3TelemetryError::Reserved)
    );
    assert_eq!(
        validate_phase3_header(
            header,
            &scenario,
            crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
            crc32_ieee(&CONFIG[..PHASE3_CONFIG_LENGTH - 4]) ^ 1,
            MissionCase::Nominal
        ),
        Err(Phase3TelemetryError::Identity)
    );
}
