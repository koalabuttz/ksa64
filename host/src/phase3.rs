use std::io::{self, Write};

use ksa64_core::phase2_mission::PLANAR_CHECKSUM_OFFSET;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_scenario::{
    parse_phase2_scenario, Phase2Scenario, PHASE2_SCENARIO_IMAGE_LENGTH,
};
use ksa64_interface::{
    crc32_ieee, write_sensor_frame, SensorFrame, EVENT_ABORT, EVENT_END, EVENT_IMPACT,
    SENSOR_FRAME_LENGTH,
};
use ksa64_sim::config::{parse_phase3_config, ConfigError, PHASE3_CONFIG_LENGTH};
use ksa64_sim::mission::{MissionCase, MissionResult};
use ksa64_sim::telemetry::{
    parse_phase3_telemetry_frame, parse_phase3_telemetry_header, run_phase3_mission_with_telemetry,
    validate_phase3_header, Phase3TelemetryError, Phase3TelemetryFailure, Phase3TelemetryFrame,
    Phase3TelemetryHeader, Phase3TelemetrySink, PHASE3_TELEMETRY_FRAME_LENGTH,
    PHASE3_TELEMETRY_HEADER_LENGTH,
};

pub struct WriterPhase3TelemetrySink<'a, W: Write> {
    writer: &'a mut W,
    frames_written: u32,
    bytes_written: usize,
}
impl<'a, W: Write> WriterPhase3TelemetrySink<'a, W> {
    pub fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            frames_written: 0,
            bytes_written: 0,
        }
    }
    pub const fn frames_written(&self) -> u32 {
        self.frames_written
    }
    pub const fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}
impl<W: Write> Phase3TelemetrySink for WriterPhase3TelemetrySink<'_, W> {
    type Error = io::Error;
    fn write_header(
        &mut self,
        bytes: &[u8; PHASE3_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        self.writer.write_all(bytes)?;
        self.bytes_written += bytes.len();
        Ok(())
    }
    fn write_frame(
        &mut self,
        bytes: &[u8; PHASE3_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error> {
        self.writer.write_all(bytes)?;
        self.frames_written += 1;
        self.bytes_written += bytes.len();
        Ok(())
    }
}

pub fn capture_phase3_mission<W: Write>(
    scenario: &Phase2Scenario,
    scenario_crc32: u32,
    config_crc32: u32,
    case: MissionCase,
    writer: &mut W,
) -> Result<(MissionResult, u32), Phase3TelemetryFailure<io::Error>> {
    let mut sink = WriterPhase3TelemetrySink::new(writer);
    run_phase3_mission_with_telemetry(scenario, scenario_crc32, config_crc32, case, &mut sink)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase3SemanticError {
    InitialFrame,
    StepOrder,
    StepRange,
    MissionTime,
    Cadence,
    TerminalBeforeEnd,
    MissingTerminal,
    TerminalEvent,
    EventMarker,
    SensorChecksum,
    SensorFrameChecksum,
    EnginePhase,
    SuccessfulEndStep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase3StreamInspectionError {
    Framing,
    Empty,
    BaseScenario,
    Config(ConfigError),
    Header(Phase3TelemetryError),
    Frame {
        index: usize,
        cause: Phase3TelemetryError,
    },
    Semantic {
        index: usize,
        cause: Phase3SemanticError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase3StreamInspection {
    pub header: Phase3TelemetryHeader,
    pub frame_count: usize,
    pub stream_bytes: usize,
    pub stream_crc32: u32,
    pub first_frame: Phase3TelemetryFrame,
    pub final_frame: Phase3TelemetryFrame,
    pub event_frames: u32,
    pub terminal_frames: u32,
}

fn sensor_bytes(frame: Phase3TelemetryFrame) -> [u8; SENSOR_FRAME_LENGTH] {
    let sensor = SensorFrame {
        sequence: frame.step,
        onboard_time_q16: frame.onboard_time_q16,
        accel_radial_q28: frame.accel_radial_q28,
        accel_tangential_q28: frame.accel_tangential_q28,
        gyro_rate_q24: frame.gyro_rate_q24,
        steering_pitch: frame.sensor_pitch,
        validity: frame.sensor_validity,
        altitude_q12: frame.altitude_q12,
        gps_radius_q12: frame.gps_radius_q12,
        gps_downrange_q32: frame.gps_downrange_q32,
        gps_radial_velocity_q24: frame.gps_radial_velocity_q24,
        gps_tangential_velocity_q24: frame.gps_tangential_velocity_q24,
        events: frame.events & !(EVENT_ABORT | EVENT_END | EVENT_IMPACT),
        active_stage: frame.active_stage,
        stage_phase: frame.stage_phase,
        engine_on: frame.engine_on,
    };
    let mut bytes = [0u8; SENSOR_FRAME_LENGTH];
    write_sensor_frame(&sensor, &mut bytes).expect("validated KST3 sensor projection");
    bytes
}
fn rolling_hash(mut hash: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}
fn semantic(index: usize, cause: Phase3SemanticError) -> Phase3StreamInspectionError {
    Phase3StreamInspectionError::Semantic { index, cause }
}

pub fn inspect_phase3_stream(
    stream: &[u8],
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
    config_image: &[u8; PHASE3_CONFIG_LENGTH],
) -> Result<Phase3StreamInspection, Phase3StreamInspectionError> {
    if stream.len() < PHASE3_TELEMETRY_HEADER_LENGTH
        || ((stream.len() - PHASE3_TELEMETRY_HEADER_LENGTH) / PHASE3_TELEMETRY_FRAME_LENGTH)
            * PHASE3_TELEMETRY_FRAME_LENGTH
            != stream.len() - PHASE3_TELEMETRY_HEADER_LENGTH
    {
        return Err(Phase3StreamInspectionError::Framing);
    }
    let frame_count =
        (stream.len() - PHASE3_TELEMETRY_HEADER_LENGTH) / PHASE3_TELEMETRY_FRAME_LENGTH;
    if frame_count == 0 {
        return Err(Phase3StreamInspectionError::Empty);
    }
    let scenario =
        parse_phase2_scenario(base).map_err(|_| Phase3StreamInspectionError::BaseScenario)?;
    let config =
        parse_phase3_config(config_image, base).map_err(Phase3StreamInspectionError::Config)?;
    let header = parse_phase3_telemetry_header(&stream[..PHASE3_TELEMETRY_HEADER_LENGTH])
        .map_err(Phase3StreamInspectionError::Header)?;
    validate_phase3_header(
        header,
        &scenario,
        crc32_ieee(&base[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
        crc32_ieee(&config_image[..PHASE3_CONFIG_LENGTH - 4]),
        config.case,
    )
    .map_err(Phase3StreamInspectionError::Header)?;

    let mut first = None;
    let mut final_frame = None;
    let mut prior_step = 0;
    let mut prior_mode = None;
    let mut event_frames = 0;
    let mut terminal_frames = 0;
    for index in 0..frame_count {
        let start = PHASE3_TELEMETRY_HEADER_LENGTH + index * PHASE3_TELEMETRY_FRAME_LENGTH;
        let frame =
            parse_phase3_telemetry_frame(&stream[start..start + PHASE3_TELEMETRY_FRAME_LENGTH])
                .map_err(|cause| Phase3StreamInspectionError::Frame { index, cause })?;
        if index == 0 {
            if frame.step != 0
                || frame.mission_time_q16 != 0
                || frame.truth_checksum != PLANAR_CHECKSUM_OFFSET
                || frame.events != 0
                || frame.terminal()
            {
                return Err(semantic(index, Phase3SemanticError::InitialFrame));
            }
            let expected_sensor_checksum = rolling_hash(2_166_136_261, &sensor_bytes(frame));
            if frame.sensor_checksum != expected_sensor_checksum {
                return Err(semantic(index, Phase3SemanticError::SensorChecksum));
            }
            first = Some(frame);
        } else if frame.step <= prior_step {
            return Err(semantic(index, Phase3SemanticError::StepOrder));
        }
        if frame.step > scenario.steps() {
            return Err(semantic(index, Phase3SemanticError::StepRange));
        }
        if i64::from(frame.mission_time_q16)
            != i64::from(frame.step) * i64::from(scenario.timestep().raw())
        {
            return Err(semantic(index, Phase3SemanticError::MissionTime));
        }
        let mode_changed = prior_mode.map(|mode| mode != frame.mode).unwrap_or(false);
        let substantive_events = frame.events & !EVENT_END;
        if frame.event_record() && substantive_events == 0 && !mode_changed {
            return Err(semantic(index, Phase3SemanticError::EventMarker));
        }
        if !frame.event_record()
            && !frame.terminal()
            && frame.step % header.telemetry_stride as u32 != 0
        {
            return Err(semantic(index, Phase3SemanticError::Cadence));
        }
        if frame.terminal() {
            terminal_frames += 1;
            if index + 1 != frame_count {
                return Err(semantic(index, Phase3SemanticError::TerminalBeforeEnd));
            }
            if frame.events & EVENT_END == 0 {
                return Err(semantic(index, Phase3SemanticError::TerminalEvent));
            }
        } else if frame.events & EVENT_END != 0 {
            return Err(semantic(index, Phase3SemanticError::TerminalEvent));
        }
        let bytes = sensor_bytes(frame);
        let embedded_crc = u32::from_le_bytes(bytes[52..56].try_into().unwrap());
        if embedded_crc != frame.sensor_frame_crc32 {
            return Err(semantic(index, Phase3SemanticError::SensorFrameChecksum));
        }
        if frame.engine_on != matches!(frame.stage_phase, ksa64_interface::StagePhase::Burning) {
            return Err(semantic(index, Phase3SemanticError::EnginePhase));
        }
        event_frames += u32::from(frame.event_record());
        prior_step = frame.step;
        prior_mode = Some(frame.mode);
        final_frame = Some(frame);
    }
    let first_frame = first.ok_or(Phase3StreamInspectionError::Empty)?;
    let final_frame = final_frame.ok_or(Phase3StreamInspectionError::Empty)?;
    if terminal_frames != 1 {
        return Err(semantic(
            frame_count - 1,
            Phase3SemanticError::MissingTerminal,
        ));
    }
    if config.case != MissionCase::SteeringStuck && final_frame.step != scenario.steps() {
        return Err(semantic(
            frame_count - 1,
            Phase3SemanticError::SuccessfulEndStep,
        ));
    }
    Ok(Phase3StreamInspection {
        header,
        frame_count,
        stream_bytes: stream.len(),
        stream_crc32: crc32_ieee(stream),
        first_frame,
        final_frame,
        event_frames,
        terminal_frames,
    })
}

pub const PHASE3_REPLAY_HEADER_LENGTH: usize = 32;
pub const PHASE3_REPLAY_FRAME_LENGTH: usize = 24;
const REPLAY_MAGIC: [u8; 4] = *b"KRP3";
fn put_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes())
}
fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes())
}
fn put_i32(out: &mut [u8], at: usize, value: i32) {
    put_u32(out, at, value as u32)
}

pub fn derive_validated_phase3_replay(
    stream: &[u8],
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
    config_image: &[u8; PHASE3_CONFIG_LENGTH],
) -> Result<Vec<u8>, Phase3StreamInspectionError> {
    let inspection = inspect_phase3_stream(stream, base, config_image)?;
    let mut replay = vec![
        0u8;
        PHASE3_REPLAY_HEADER_LENGTH
            + inspection.frame_count * PHASE3_REPLAY_FRAME_LENGTH
    ];
    replay[..4].copy_from_slice(&REPLAY_MAGIC);
    put_u16(&mut replay, 4, 3);
    put_u16(&mut replay, 6, PHASE3_REPLAY_HEADER_LENGTH as u16);
    put_u16(&mut replay, 8, PHASE3_REPLAY_FRAME_LENGTH as u16);
    put_u32(&mut replay, 12, inspection.stream_crc32);
    put_u32(&mut replay, 16, inspection.header.scenario_id);
    put_u32(&mut replay, 20, inspection.header.config_crc32);
    put_u32(&mut replay, 24, inspection.frame_count as u32);
    let header_crc = crc32_ieee(&replay[..28]);
    put_u32(&mut replay, 28, header_crc);
    for index in 0..inspection.frame_count {
        let source = PHASE3_TELEMETRY_HEADER_LENGTH + index * PHASE3_TELEMETRY_FRAME_LENGTH;
        let frame =
            parse_phase3_telemetry_frame(&stream[source..source + PHASE3_TELEMETRY_FRAME_LENGTH])
                .expect("stream was strictly inspected");
        let at = PHASE3_REPLAY_HEADER_LENGTH + index * PHASE3_REPLAY_FRAME_LENGTH;
        put_u32(&mut replay, at, frame.step);
        put_i32(&mut replay, at + 4, frame.radius_q12 - EARTH_RADIUS_Q12);
        put_i32(&mut replay, at + 8, frame.downrange_q32);
        put_u16(&mut replay, at + 12, frame.applied_pitch);
        replay[at + 14] = frame.mode as u8;
        replay[at + 15] = frame.active_stage;
        put_u16(&mut replay, at + 16, frame.events);
        put_u16(&mut replay, at + 18, frame.alarms);
        let frame_crc = crc32_ieee(&replay[at..at + 20]);
        put_u32(&mut replay, at + 20, frame_crc);
    }
    Ok(replay)
}
