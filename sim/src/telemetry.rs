//! Canonical allocation-free Phase 3 telemetry (`KST3`).

use crate::mission::{
    run_phase3_mission_observed, MissionCase, MissionError, MissionObserver, MissionRecord,
    MissionResult, MissionRunError,
};
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_interface::{
    crc32_ieee, EngineAction, FlightMode, StagePhase, ALARM_MASK, EVENT_ABORT, EVENT_END,
    EVENT_IMPACT, EVENT_MASK, SENSOR_VALID_MASK,
};

pub const PHASE3_TELEMETRY_VERSION: u16 = 3;
pub const PHASE3_TELEMETRY_HEADER_LENGTH: usize = 64;
pub const PHASE3_TELEMETRY_FRAME_LENGTH: usize = 160;
pub const PHASE3_TELEMETRY_STRIDE: u16 = 8;
pub const PHASE3_TELEMETRY_CONTRACT_ID: u32 = 0x0300_0001;
pub const FRAME_TERMINAL: u16 = 1 << 0;
pub const FRAME_EVENT: u16 = 1 << 1;
const FRAME_FLAG_MASK: u16 = FRAME_TERMINAL | FRAME_EVENT;
const HEADER_MAGIC: [u8; 4] = *b"KST3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase3TelemetryError {
    Length,
    Magic,
    Version,
    HeaderLength,
    FrameLength,
    Contract,
    Identity,
    Timestep,
    Stride,
    MissionSteps,
    Reserved,
    Flags,
    Enum,
    Checksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase3TelemetryHeader {
    pub contract_id: u32,
    pub scenario_id: u32,
    pub scenario_crc32: u32,
    pub config_crc32: u32,
    pub seed: u32,
    pub case: MissionCase,
    pub timestep_q16: i32,
    pub telemetry_stride: u16,
    pub mission_steps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase3TelemetryFrame {
    pub step: u32,
    pub mission_time_q16: i32,
    pub radius_q12: i32,
    pub downrange_q32: i32,
    pub radial_velocity_q24: i32,
    pub specific_angular_momentum_q14: i32,
    pub radial_acceleration_q28: i32,
    pub tangential_acceleration_q28: i32,
    pub total_mass_q12: i32,
    pub propellant_q12: i32,
    pub truth_pitch: u16,
    pub applied_pitch: u16,
    pub requested_pitch: u16,
    pub sensor_validity: u16,
    pub mach_q16: i32,
    pub dynamic_pressure_q16: i32,
    pub events: u16,
    pub alarms: u16,
    pub active_stage: u8,
    pub stage_phase: StagePhase,
    pub engine_on: bool,
    pub mode: FlightMode,
    pub accel_radial_q28: i32,
    pub accel_tangential_q28: i32,
    pub gyro_rate_q24: i32,
    pub sensor_pitch: u16,
    pub altitude_q12: i32,
    pub gps_radius_q12: i32,
    pub gps_downrange_q32: i32,
    pub gps_radial_velocity_q24: i32,
    pub gps_tangential_velocity_q24: i32,
    pub onboard_time_q16: i32,
    pub nav_time_q16: i32,
    pub nav_radius_q12: i32,
    pub nav_downrange_q32: i32,
    pub nav_radial_velocity_q24: i32,
    pub nav_tangential_velocity_q24: i32,
    pub nav_pitch: u16,
    pub command_bits: u16,
    pub truth_checksum: u32,
    pub sensor_checksum: u32,
    pub nav_checksum: u32,
    pub flight_checksum: u32,
    pub sensor_frame_crc32: u32,
    pub frame_flags: u16,
}

impl Phase3TelemetryFrame {
    pub const fn terminal(self) -> bool {
        self.frame_flags & FRAME_TERMINAL != 0
    }
    pub const fn event_record(self) -> bool {
        self.frame_flags & FRAME_EVENT != 0
    }
}

fn case_byte(case: MissionCase) -> u8 {
    match case {
        MissionCase::Nominal => 0,
        MissionCase::AltimeterDropout => 1,
        MissionCase::GpsOutage => 2,
        MissionCase::SteeringStuck => 3,
    }
}
fn parse_case(value: u8) -> Result<MissionCase, Phase3TelemetryError> {
    match value {
        0 => Ok(MissionCase::Nominal),
        1 => Ok(MissionCase::AltimeterDropout),
        2 => Ok(MissionCase::GpsOutage),
        3 => Ok(MissionCase::SteeringStuck),
        _ => Err(Phase3TelemetryError::Enum),
    }
}
fn parse_stage(value: u8) -> Result<StagePhase, Phase3TelemetryError> {
    match value {
        0 => Ok(StagePhase::CoastBeforeIgnition),
        1 => Ok(StagePhase::Burning),
        2 => Ok(StagePhase::CoastBeforeSeparation),
        3 => Ok(StagePhase::Complete),
        _ => Err(Phase3TelemetryError::Enum),
    }
}
fn parse_mode(value: u8) -> Result<FlightMode, Phase3TelemetryError> {
    match value {
        0 => Ok(FlightMode::Boot),
        1 => Ok(FlightMode::Prelaunch),
        2 => Ok(FlightMode::ProgrammedAscent),
        3 => Ok(FlightMode::StageTransition),
        4 => Ok(FlightMode::Insertion),
        5 => Ok(FlightMode::Coast),
        6 => Ok(FlightMode::Complete),
        7 => Ok(FlightMode::Abort),
        _ => Err(Phase3TelemetryError::Enum),
    }
}
fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes())
}
fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
}
fn put_i32(out: &mut [u8], offset: usize, value: i32) {
    put_u32(out, offset, value as u32)
}
fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
fn get_i32(input: &[u8], offset: usize) -> i32 {
    get_u32(input, offset) as i32
}

pub fn write_phase3_telemetry_header(
    header: Phase3TelemetryHeader,
    output: &mut [u8],
) -> Result<(), Phase3TelemetryError> {
    if output.len() != PHASE3_TELEMETRY_HEADER_LENGTH {
        return Err(Phase3TelemetryError::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(&HEADER_MAGIC);
    put_u16(output, 4, PHASE3_TELEMETRY_VERSION);
    put_u16(output, 6, PHASE3_TELEMETRY_HEADER_LENGTH as u16);
    put_u16(output, 8, PHASE3_TELEMETRY_FRAME_LENGTH as u16);
    put_u16(output, 10, header.telemetry_stride);
    put_u32(output, 12, header.contract_id);
    put_u32(output, 16, header.scenario_id);
    put_u32(output, 20, header.scenario_crc32);
    put_u32(output, 24, header.config_crc32);
    put_u32(output, 28, header.seed);
    output[32] = case_byte(header.case);
    put_i32(output, 36, header.timestep_q16);
    put_u32(output, 40, header.mission_steps);
    put_u32(output, 60, crc32_ieee(&output[..60]));
    Ok(())
}

pub fn parse_phase3_telemetry_header(
    input: &[u8],
) -> Result<Phase3TelemetryHeader, Phase3TelemetryError> {
    if input.len() != PHASE3_TELEMETRY_HEADER_LENGTH {
        return Err(Phase3TelemetryError::Length);
    }
    if input[..4] != HEADER_MAGIC {
        return Err(Phase3TelemetryError::Magic);
    }
    if get_u16(input, 4) != PHASE3_TELEMETRY_VERSION {
        return Err(Phase3TelemetryError::Version);
    }
    if get_u16(input, 6) as usize != PHASE3_TELEMETRY_HEADER_LENGTH {
        return Err(Phase3TelemetryError::HeaderLength);
    }
    if get_u16(input, 8) as usize != PHASE3_TELEMETRY_FRAME_LENGTH {
        return Err(Phase3TelemetryError::FrameLength);
    }
    if get_u32(input, 12) != PHASE3_TELEMETRY_CONTRACT_ID {
        return Err(Phase3TelemetryError::Contract);
    }
    if input[33] != 0 || input[34] != 0 || input[35] != 0 || input[44..60].iter().any(|&b| b != 0) {
        return Err(Phase3TelemetryError::Reserved);
    }
    if get_u16(input, 10) != PHASE3_TELEMETRY_STRIDE {
        return Err(Phase3TelemetryError::Stride);
    }
    if crc32_ieee(&input[..60]) != get_u32(input, 60) {
        return Err(Phase3TelemetryError::Checksum);
    }
    Ok(Phase3TelemetryHeader {
        contract_id: get_u32(input, 12),
        scenario_id: get_u32(input, 16),
        scenario_crc32: get_u32(input, 20),
        config_crc32: get_u32(input, 24),
        seed: get_u32(input, 28),
        case: parse_case(input[32])?,
        timestep_q16: get_i32(input, 36),
        telemetry_stride: get_u16(input, 10),
        mission_steps: get_u32(input, 40),
    })
}

pub fn validate_phase3_header(
    header: Phase3TelemetryHeader,
    scenario: &Phase2Scenario,
    scenario_crc32: u32,
    config_crc32: u32,
    case: MissionCase,
) -> Result<(), Phase3TelemetryError> {
    if header.scenario_id != scenario.scenario_id()
        || header.scenario_crc32 != scenario_crc32
        || header.config_crc32 != config_crc32
        || header.seed != case.seed()
        || header.case != case
    {
        return Err(Phase3TelemetryError::Identity);
    }
    if header.timestep_q16 != scenario.timestep().raw() {
        return Err(Phase3TelemetryError::Timestep);
    }
    if header.mission_steps != scenario.steps() {
        return Err(Phase3TelemetryError::MissionSteps);
    }
    Ok(())
}

pub fn frame_from_record(
    record: MissionRecord,
    terminal: bool,
    event_record: bool,
) -> Phase3TelemetryFrame {
    let truth = record.world.truth;
    let mut sensor_bytes = [0u8; ksa64_interface::SENSOR_FRAME_LENGTH];
    let _ = ksa64_interface::write_sensor_frame(&record.sensors, &mut sensor_bytes);
    let mut events = record.sensors.events;
    if truth.radius().raw() < EARTH_RADIUS_Q12 {
        events |= EVENT_IMPACT;
    }
    if record.flight.mode == FlightMode::Abort {
        events |= EVENT_ABORT;
    }
    if terminal {
        events |= EVENT_END;
    }
    let command = record.flight.command;
    let command_bits = command.engine_action as u16
        | ((command.separate as u16) << 2)
        | ((command.abort_safeing as u16) << 3)
        | ((command.recovery_requested as u16) << 4);
    Phase3TelemetryFrame {
        step: truth.step(),
        mission_time_q16: truth.time().raw(),
        radius_q12: truth.radius().raw(),
        downrange_q32: truth.downrange().raw(),
        radial_velocity_q24: truth.radial_velocity().raw(),
        specific_angular_momentum_q14: truth.specific_angular_momentum().raw(),
        radial_acceleration_q28: truth.radial_acceleration().raw(),
        tangential_acceleration_q28: truth.tangential_acceleration().raw(),
        total_mass_q12: truth.total_mass().raw(),
        propellant_q12: truth.active_propellant().raw(),
        truth_pitch: record.world.pitch.raw(),
        applied_pitch: record.steering.applied,
        requested_pitch: command.desired_pitch,
        sensor_validity: record.sensors.validity,
        mach_q16: record.world.mach.raw(),
        dynamic_pressure_q16: record.world.dynamic_pressure.raw(),
        events,
        alarms: record.flight.alarms,
        active_stage: record.sensors.active_stage,
        stage_phase: record.sensors.stage_phase,
        engine_on: record.sensors.engine_on,
        mode: record.flight.mode,
        accel_radial_q28: record.sensors.accel_radial_q28,
        accel_tangential_q28: record.sensors.accel_tangential_q28,
        gyro_rate_q24: record.sensors.gyro_rate_q24,
        sensor_pitch: record.sensors.steering_pitch,
        altitude_q12: record.sensors.altitude_q12,
        gps_radius_q12: record.sensors.gps_radius_q12,
        gps_downrange_q32: record.sensors.gps_downrange_q32,
        gps_radial_velocity_q24: record.sensors.gps_radial_velocity_q24,
        gps_tangential_velocity_q24: record.sensors.gps_tangential_velocity_q24,
        onboard_time_q16: record.sensors.onboard_time_q16,
        nav_time_q16: record.flight.nav_time_q16,
        nav_radius_q12: record.flight.nav_radius_q12,
        nav_downrange_q32: record.flight.nav_downrange_q32,
        nav_radial_velocity_q24: record.flight.nav_radial_velocity_q24,
        nav_tangential_velocity_q24: record.flight.nav_tangential_velocity_q24,
        nav_pitch: record.flight.nav_pitch,
        command_bits,
        truth_checksum: record.world.truth_checksum,
        sensor_checksum: record.sensor_checksum,
        nav_checksum: record.flight.nav_checksum,
        flight_checksum: record.flight.flight_checksum,
        sensor_frame_crc32: get_u32(&sensor_bytes, 52),
        frame_flags: (if terminal { FRAME_TERMINAL } else { 0 })
            | (if event_record { FRAME_EVENT } else { 0 }),
    }
}

pub fn write_phase3_telemetry_frame(
    frame: Phase3TelemetryFrame,
    output: &mut [u8],
) -> Result<(), Phase3TelemetryError> {
    if output.len() != PHASE3_TELEMETRY_FRAME_LENGTH {
        return Err(Phase3TelemetryError::Length);
    }
    if frame.sensor_validity & !SENSOR_VALID_MASK != 0
        || frame.events & !EVENT_MASK != 0
        || frame.alarms & !ALARM_MASK != 0
        || frame.frame_flags & !FRAME_FLAG_MASK != 0
        || frame.command_bits & !0x001f != 0
    {
        return Err(Phase3TelemetryError::Flags);
    }
    output.fill(0);
    put_u32(output, 0, frame.step);
    put_i32(output, 4, frame.mission_time_q16);
    put_i32(output, 8, frame.radius_q12);
    put_i32(output, 12, frame.downrange_q32);
    put_i32(output, 16, frame.radial_velocity_q24);
    put_i32(output, 20, frame.specific_angular_momentum_q14);
    put_i32(output, 24, frame.radial_acceleration_q28);
    put_i32(output, 28, frame.tangential_acceleration_q28);
    put_i32(output, 32, frame.total_mass_q12);
    put_i32(output, 36, frame.propellant_q12);
    put_u16(output, 40, frame.truth_pitch);
    put_u16(output, 42, frame.applied_pitch);
    put_u16(output, 44, frame.requested_pitch);
    put_u16(output, 46, frame.sensor_validity);
    put_i32(output, 48, frame.mach_q16);
    put_i32(output, 52, frame.dynamic_pressure_q16);
    put_u16(output, 56, frame.events);
    put_u16(output, 58, frame.alarms);
    output[60] = frame.active_stage;
    output[61] = frame.stage_phase as u8;
    output[62] = frame.engine_on as u8;
    output[63] = frame.mode as u8;
    put_i32(output, 64, frame.accel_radial_q28);
    put_i32(output, 68, frame.accel_tangential_q28);
    put_i32(output, 72, frame.gyro_rate_q24);
    put_u16(output, 76, frame.sensor_pitch);
    put_i32(output, 80, frame.altitude_q12);
    put_i32(output, 84, frame.gps_radius_q12);
    put_i32(output, 88, frame.gps_downrange_q32);
    put_i32(output, 92, frame.gps_radial_velocity_q24);
    put_i32(output, 96, frame.gps_tangential_velocity_q24);
    put_i32(output, 100, frame.onboard_time_q16);
    put_i32(output, 104, frame.nav_time_q16);
    put_i32(output, 108, frame.nav_radius_q12);
    put_i32(output, 112, frame.nav_downrange_q32);
    put_i32(output, 116, frame.nav_radial_velocity_q24);
    put_i32(output, 120, frame.nav_tangential_velocity_q24);
    put_u16(output, 124, frame.nav_pitch);
    put_u16(output, 126, frame.command_bits);
    put_u32(output, 128, frame.truth_checksum);
    put_u32(output, 132, frame.sensor_checksum);
    put_u32(output, 136, frame.nav_checksum);
    put_u32(output, 140, frame.flight_checksum);
    put_u32(output, 144, frame.sensor_frame_crc32);
    put_u16(output, 148, frame.frame_flags);
    put_u32(output, 156, crc32_ieee(&output[..156]));
    Ok(())
}

pub fn parse_phase3_telemetry_frame(
    input: &[u8],
) -> Result<Phase3TelemetryFrame, Phase3TelemetryError> {
    if input.len() != PHASE3_TELEMETRY_FRAME_LENGTH {
        return Err(Phase3TelemetryError::Length);
    }
    if input[150..156].iter().any(|&b| b != 0) {
        return Err(Phase3TelemetryError::Reserved);
    }
    if crc32_ieee(&input[..156]) != get_u32(input, 156) {
        return Err(Phase3TelemetryError::Checksum);
    }
    let validity = get_u16(input, 46);
    let events = get_u16(input, 56);
    let alarms = get_u16(input, 58);
    let command_bits = get_u16(input, 126);
    let frame_flags = get_u16(input, 148);
    if validity & !SENSOR_VALID_MASK != 0
        || events & !EVENT_MASK != 0
        || alarms & !ALARM_MASK != 0
        || command_bits & !0x001f != 0
        || frame_flags & !FRAME_FLAG_MASK != 0
        || input[62] > 1
        || command_bits & 3 > EngineAction::Cutoff as u16
    {
        return Err(Phase3TelemetryError::Flags);
    }
    Ok(Phase3TelemetryFrame {
        step: get_u32(input, 0),
        mission_time_q16: get_i32(input, 4),
        radius_q12: get_i32(input, 8),
        downrange_q32: get_i32(input, 12),
        radial_velocity_q24: get_i32(input, 16),
        specific_angular_momentum_q14: get_i32(input, 20),
        radial_acceleration_q28: get_i32(input, 24),
        tangential_acceleration_q28: get_i32(input, 28),
        total_mass_q12: get_i32(input, 32),
        propellant_q12: get_i32(input, 36),
        truth_pitch: get_u16(input, 40),
        applied_pitch: get_u16(input, 42),
        requested_pitch: get_u16(input, 44),
        sensor_validity: validity,
        mach_q16: get_i32(input, 48),
        dynamic_pressure_q16: get_i32(input, 52),
        events,
        alarms,
        active_stage: input[60],
        stage_phase: parse_stage(input[61])?,
        engine_on: input[62] != 0,
        mode: parse_mode(input[63])?,
        accel_radial_q28: get_i32(input, 64),
        accel_tangential_q28: get_i32(input, 68),
        gyro_rate_q24: get_i32(input, 72),
        sensor_pitch: get_u16(input, 76),
        altitude_q12: get_i32(input, 80),
        gps_radius_q12: get_i32(input, 84),
        gps_downrange_q32: get_i32(input, 88),
        gps_radial_velocity_q24: get_i32(input, 92),
        gps_tangential_velocity_q24: get_i32(input, 96),
        onboard_time_q16: get_i32(input, 100),
        nav_time_q16: get_i32(input, 104),
        nav_radius_q12: get_i32(input, 108),
        nav_downrange_q32: get_i32(input, 112),
        nav_radial_velocity_q24: get_i32(input, 116),
        nav_tangential_velocity_q24: get_i32(input, 120),
        nav_pitch: get_u16(input, 124),
        command_bits,
        truth_checksum: get_u32(input, 128),
        sensor_checksum: get_u32(input, 132),
        nav_checksum: get_u32(input, 136),
        flight_checksum: get_u32(input, 140),
        sensor_frame_crc32: get_u32(input, 144),
        frame_flags,
    })
}

pub trait Phase3TelemetrySink {
    type Error;
    fn write_header(
        &mut self,
        header: &[u8; PHASE3_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error>;
    fn write_frame(
        &mut self,
        frame: &[u8; PHASE3_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase3TelemetryFailure<E> {
    Mission(MissionError),
    Sink(E),
    Encoding(Phase3TelemetryError),
}

struct TelemetryObserver<'a, S: Phase3TelemetrySink> {
    sink: &'a mut S,
    pending: Option<MissionRecord>,
    pending_event: bool,
    previous_mode: Option<FlightMode>,
    frames: u32,
}
impl<'a, S: Phase3TelemetrySink> TelemetryObserver<'a, S> {
    fn emit(
        &mut self,
        record: MissionRecord,
        terminal: bool,
        event_record: bool,
    ) -> Result<(), S::Error> {
        if record.world.truth.step() == 0
            || terminal
            || event_record
            || (record.world.truth.step() / PHASE3_TELEMETRY_STRIDE as u32)
                * PHASE3_TELEMETRY_STRIDE as u32
                == record.world.truth.step()
        {
            let frame = frame_from_record(record, terminal, event_record);
            let mut bytes = [0u8; PHASE3_TELEMETRY_FRAME_LENGTH];
            write_phase3_telemetry_frame(frame, &mut bytes).expect("internally valid KST3 frame");
            self.sink.write_frame(&bytes)?;
            self.frames += 1;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), S::Error> {
        if let Some(record) = self.pending.take() {
            self.emit(record, true, self.pending_event)?;
        }
        Ok(())
    }
}
impl<S: Phase3TelemetrySink> MissionObserver for TelemetryObserver<'_, S> {
    type Error = S::Error;
    fn observe(&mut self, record: MissionRecord) -> Result<(), Self::Error> {
        if let Some(pending) = self.pending.take() {
            self.emit(pending, false, self.pending_event)?;
        }
        let mode_event = self
            .previous_mode
            .map(|m| m != record.flight.mode)
            .unwrap_or(false);
        self.pending_event = record.world.events != 0 || mode_event;
        self.previous_mode = Some(record.flight.mode);
        self.pending = Some(record);
        Ok(())
    }
}

pub fn run_phase3_mission_with_telemetry<S: Phase3TelemetrySink>(
    scenario: &Phase2Scenario,
    scenario_crc32: u32,
    config_crc32: u32,
    case: MissionCase,
    sink: &mut S,
) -> Result<(MissionResult, u32), Phase3TelemetryFailure<S::Error>> {
    let header = Phase3TelemetryHeader {
        contract_id: PHASE3_TELEMETRY_CONTRACT_ID,
        scenario_id: scenario.scenario_id(),
        scenario_crc32,
        config_crc32,
        seed: case.seed(),
        case,
        timestep_q16: scenario.timestep().raw(),
        telemetry_stride: PHASE3_TELEMETRY_STRIDE,
        mission_steps: scenario.steps(),
    };
    let mut header_bytes = [0u8; PHASE3_TELEMETRY_HEADER_LENGTH];
    write_phase3_telemetry_header(header, &mut header_bytes)
        .map_err(Phase3TelemetryFailure::Encoding)?;
    sink.write_header(&header_bytes)
        .map_err(Phase3TelemetryFailure::Sink)?;
    let mut observer = TelemetryObserver {
        sink,
        pending: None,
        pending_event: false,
        previous_mode: None,
        frames: 0,
    };
    let result = match run_phase3_mission_observed(scenario, case, &mut observer) {
        Ok(result) => result,
        Err(MissionRunError::Mission(error)) => return Err(Phase3TelemetryFailure::Mission(error)),
        Err(MissionRunError::Observer(error)) => return Err(Phase3TelemetryFailure::Sink(error)),
    };
    observer.finish().map_err(Phase3TelemetryFailure::Sink)?;
    Ok((result, observer.frames))
}
