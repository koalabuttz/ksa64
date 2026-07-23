//! Canonical allocation-free Phase 2 planar telemetry (`KST2`).

use crate::numeric::NumericStatus;
use crate::phase2_mission::{
    execute_phase2_mission_observed, Phase2ExecutionError, Phase2MissionError, Phase2MissionResult,
    Phase2Observation, Phase2Observer, EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_IMPACT,
    EVENT_SEPARATION,
};
use crate::phase2_numeric::{PHASE2_ENVIRONMENT_ID, PHASE2_NUMERIC_CONTRACT_ID};
use crate::phase2_quantities::{
    DownrangeAngle, DynamicPressure, Mach, PitchAngle, PlanarAcceleration, PlanarVelocity, Radius,
    SpecificAngularMomentum,
};
use crate::phase2_scenario::Phase2Scenario;
use crate::planar::StagePhase;
use crate::quantities::{Mass, Time};
use crate::scenario::crc32_ieee;

pub const PHASE2_TELEMETRY_VERSION: u16 = 2;
pub const PHASE2_TELEMETRY_HEADER_LENGTH: usize = 40;
pub const PHASE2_TELEMETRY_FRAME_LENGTH: usize = 64;
const HEADER_MAGIC: [u8; 4] = *b"KST2";
const ACCEPTED_EVENTS: u16 =
    EVENT_IGNITION | EVENT_CUTOFF | EVENT_SEPARATION | EVENT_IMPACT | EVENT_END;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2TelemetryWriteError {
    Length,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2TelemetryReadError {
    Length,
    Magic,
    Version,
    HeaderLength,
    FrameLength,
    Reserved,
    NumericContract,
    Environment,
    ScenarioIdentity,
    Timestep,
    TelemetryStride,
    MissionSteps,
    StatusBits,
    EventBits,
    StagePhase,
    Checksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2TelemetryHeader {
    numeric_contract_id: u32,
    scenario_id: u32,
    environment_id: u32,
    timestep: Time,
    telemetry_stride: u16,
    mission_steps: u32,
}

impl Phase2TelemetryHeader {
    pub const fn numeric_contract_id(self) -> u32 {
        self.numeric_contract_id
    }
    pub const fn scenario_id(self) -> u32 {
        self.scenario_id
    }
    pub const fn environment_id(self) -> u32 {
        self.environment_id
    }
    pub const fn timestep(self) -> Time {
        self.timestep
    }
    pub const fn telemetry_stride(self) -> u16 {
        self.telemetry_stride
    }
    pub const fn mission_steps(self) -> u32 {
        self.mission_steps
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2TelemetryFrame {
    step: u32,
    time: Time,
    radius: Radius,
    downrange: DownrangeAngle,
    radial_velocity: PlanarVelocity,
    specific_angular_momentum: SpecificAngularMomentum,
    radial_acceleration: PlanarAcceleration,
    tangential_acceleration: PlanarAcceleration,
    total_mass: Mass,
    propellant: Mass,
    pitch: PitchAngle,
    active_stage: u8,
    stage_phase: StagePhase,
    mach: Mach,
    dynamic_pressure: DynamicPressure,
    status: u16,
    events: u16,
    state_checksum: u32,
}

impl Phase2TelemetryFrame {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        step: u32,
        time: Time,
        radius: Radius,
        downrange: DownrangeAngle,
        radial_velocity: PlanarVelocity,
        specific_angular_momentum: SpecificAngularMomentum,
        radial_acceleration: PlanarAcceleration,
        tangential_acceleration: PlanarAcceleration,
        total_mass: Mass,
        propellant: Mass,
        pitch: PitchAngle,
        active_stage: u8,
        stage_phase: StagePhase,
        mach: Mach,
        dynamic_pressure: DynamicPressure,
        status: u16,
        events: u16,
        state_checksum: u32,
    ) -> Self {
        Self {
            step,
            time,
            radius,
            downrange,
            radial_velocity,
            specific_angular_momentum,
            radial_acceleration,
            tangential_acceleration,
            total_mass,
            propellant,
            pitch,
            active_stage,
            stage_phase,
            mach,
            dynamic_pressure,
            status,
            events,
            state_checksum,
        }
    }

    pub const fn from_observation(observation: Phase2Observation, events: u16) -> Self {
        let truth = observation.truth();
        Self::new(
            truth.step(),
            truth.time(),
            truth.radius(),
            truth.downrange(),
            truth.radial_velocity(),
            truth.specific_angular_momentum(),
            truth.radial_acceleration(),
            truth.tangential_acceleration(),
            truth.total_mass(),
            truth.active_propellant(),
            observation.pitch(),
            truth.active_stage(),
            truth.stage_phase(),
            observation.mach(),
            observation.dynamic_pressure(),
            if matches!(truth.stage_phase(), StagePhase::Burning) {
                1
            } else {
                0
            },
            events,
            observation.state_checksum(),
        )
    }

    pub const fn step(self) -> u32 {
        self.step
    }
    pub const fn time(self) -> Time {
        self.time
    }
    pub const fn radius(self) -> Radius {
        self.radius
    }
    pub const fn downrange(self) -> DownrangeAngle {
        self.downrange
    }
    pub const fn radial_velocity(self) -> PlanarVelocity {
        self.radial_velocity
    }
    pub const fn specific_angular_momentum(self) -> SpecificAngularMomentum {
        self.specific_angular_momentum
    }
    pub const fn radial_acceleration(self) -> PlanarAcceleration {
        self.radial_acceleration
    }
    pub const fn tangential_acceleration(self) -> PlanarAcceleration {
        self.tangential_acceleration
    }
    pub const fn total_mass(self) -> Mass {
        self.total_mass
    }
    pub const fn propellant(self) -> Mass {
        self.propellant
    }
    pub const fn pitch(self) -> PitchAngle {
        self.pitch
    }
    pub const fn active_stage(self) -> u8 {
        self.active_stage
    }
    pub const fn stage_phase(self) -> StagePhase {
        self.stage_phase
    }
    pub const fn mach(self) -> Mach {
        self.mach
    }
    pub const fn dynamic_pressure(self) -> DynamicPressure {
        self.dynamic_pressure
    }
    pub const fn status(self) -> u16 {
        self.status
    }
    pub const fn events(self) -> u16 {
        self.events
    }
    pub const fn state_checksum(self) -> u32 {
        self.state_checksum
    }
}

#[inline]
fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
#[inline]
fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
#[inline]
fn write_i32(output: &mut [u8], offset: usize, value: i32) {
    write_u32(output, offset, value as u32);
}
#[inline]
fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
#[inline]
fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
#[inline]
fn read_i32(input: &[u8], offset: usize) -> i32 {
    read_u32(input, offset) as i32
}

pub fn write_phase2_telemetry_header(
    scenario: &Phase2Scenario,
    output: &mut [u8],
) -> Result<(), Phase2TelemetryWriteError> {
    if output.len() != PHASE2_TELEMETRY_HEADER_LENGTH {
        return Err(Phase2TelemetryWriteError::Length);
    }
    output[0..4].copy_from_slice(&HEADER_MAGIC);
    write_u16(output, 4, PHASE2_TELEMETRY_VERSION);
    write_u16(output, 6, PHASE2_TELEMETRY_HEADER_LENGTH as u16);
    write_u16(output, 8, PHASE2_TELEMETRY_FRAME_LENGTH as u16);
    write_u16(output, 10, 0);
    write_u32(output, 12, PHASE2_NUMERIC_CONTRACT_ID);
    write_u32(output, 16, scenario.scenario_id());
    write_u32(output, 20, PHASE2_ENVIRONMENT_ID);
    write_i32(output, 24, scenario.timestep().raw());
    write_u16(output, 28, scenario.telemetry_stride());
    write_u16(output, 30, 0);
    write_u32(output, 32, scenario.steps());
    write_u32(output, 36, crc32_ieee(&output[..36]));
    Ok(())
}

pub fn write_phase2_telemetry_frame(
    frame: &Phase2TelemetryFrame,
    output: &mut [u8],
) -> Result<(), Phase2TelemetryWriteError> {
    if output.len() != PHASE2_TELEMETRY_FRAME_LENGTH {
        return Err(Phase2TelemetryWriteError::Length);
    }
    write_u32(output, 0, frame.step());
    write_i32(output, 4, frame.time().raw());
    write_i32(output, 8, frame.radius().raw());
    write_i32(output, 12, frame.downrange().raw());
    write_i32(output, 16, frame.radial_velocity().raw());
    write_i32(output, 20, frame.specific_angular_momentum().raw());
    write_i32(output, 24, frame.radial_acceleration().raw());
    write_i32(output, 28, frame.tangential_acceleration().raw());
    write_i32(output, 32, frame.total_mass().raw());
    write_i32(output, 36, frame.propellant().raw());
    write_u16(output, 40, frame.pitch().raw());
    output[42] = frame.active_stage();
    output[43] = frame.stage_phase() as u8;
    write_i32(output, 44, frame.mach().raw());
    write_i32(output, 48, frame.dynamic_pressure().raw());
    write_u16(output, 52, frame.status());
    write_u16(output, 54, frame.events());
    write_u32(output, 56, frame.state_checksum());
    write_u32(output, 60, crc32_ieee(&output[..60]));
    Ok(())
}

pub fn parse_phase2_telemetry_header(
    input: &[u8],
) -> Result<Phase2TelemetryHeader, Phase2TelemetryReadError> {
    if input.len() != PHASE2_TELEMETRY_HEADER_LENGTH {
        return Err(Phase2TelemetryReadError::Length);
    }
    if input[..4] != HEADER_MAGIC {
        return Err(Phase2TelemetryReadError::Magic);
    }
    if read_u16(input, 4) != PHASE2_TELEMETRY_VERSION {
        return Err(Phase2TelemetryReadError::Version);
    }
    if read_u16(input, 6) as usize != PHASE2_TELEMETRY_HEADER_LENGTH {
        return Err(Phase2TelemetryReadError::HeaderLength);
    }
    if read_u16(input, 8) as usize != PHASE2_TELEMETRY_FRAME_LENGTH {
        return Err(Phase2TelemetryReadError::FrameLength);
    }
    if read_u16(input, 10) != 0 || read_u16(input, 30) != 0 {
        return Err(Phase2TelemetryReadError::Reserved);
    }
    if read_u32(input, 12) != PHASE2_NUMERIC_CONTRACT_ID {
        return Err(Phase2TelemetryReadError::NumericContract);
    }
    if read_u32(input, 20) != PHASE2_ENVIRONMENT_ID {
        return Err(Phase2TelemetryReadError::Environment);
    }
    if crc32_ieee(&input[..36]) != read_u32(input, 36) {
        return Err(Phase2TelemetryReadError::Checksum);
    }
    Ok(Phase2TelemetryHeader {
        numeric_contract_id: read_u32(input, 12),
        scenario_id: read_u32(input, 16),
        environment_id: read_u32(input, 20),
        timestep: Time::from_raw(read_i32(input, 24)),
        telemetry_stride: read_u16(input, 28),
        mission_steps: read_u32(input, 32),
    })
}

pub fn parse_phase2_telemetry_header_for_scenario(
    input: &[u8],
    scenario: &Phase2Scenario,
) -> Result<Phase2TelemetryHeader, Phase2TelemetryReadError> {
    let header = parse_phase2_telemetry_header(input)?;
    if header.scenario_id() != scenario.scenario_id() {
        return Err(Phase2TelemetryReadError::ScenarioIdentity);
    }
    if header.timestep() != scenario.timestep() {
        return Err(Phase2TelemetryReadError::Timestep);
    }
    if header.telemetry_stride() != scenario.telemetry_stride() {
        return Err(Phase2TelemetryReadError::TelemetryStride);
    }
    if header.mission_steps() != scenario.steps() {
        return Err(Phase2TelemetryReadError::MissionSteps);
    }
    Ok(header)
}

pub fn parse_phase2_telemetry_frame(
    input: &[u8],
) -> Result<Phase2TelemetryFrame, Phase2TelemetryReadError> {
    if input.len() != PHASE2_TELEMETRY_FRAME_LENGTH {
        return Err(Phase2TelemetryReadError::Length);
    }
    if crc32_ieee(&input[..60]) != read_u32(input, 60) {
        return Err(Phase2TelemetryReadError::Checksum);
    }
    let status = read_u16(input, 52);
    let events = read_u16(input, 54);
    if status & !1 != 0 {
        return Err(Phase2TelemetryReadError::StatusBits);
    }
    if events & !ACCEPTED_EVENTS != 0 {
        return Err(Phase2TelemetryReadError::EventBits);
    }
    let phase = match input[43] {
        0 => StagePhase::CoastBeforeIgnition,
        1 => StagePhase::Burning,
        2 => StagePhase::CoastBeforeSeparation,
        3 => StagePhase::Complete,
        _ => return Err(Phase2TelemetryReadError::StagePhase),
    };
    if (status == 1) != (phase == StagePhase::Burning) {
        return Err(Phase2TelemetryReadError::StatusBits);
    }
    Ok(Phase2TelemetryFrame::new(
        read_u32(input, 0),
        Time::from_raw(read_i32(input, 4)),
        Radius::from_raw(read_i32(input, 8)),
        DownrangeAngle::from_raw(read_i32(input, 12)),
        PlanarVelocity::from_raw(read_i32(input, 16)),
        SpecificAngularMomentum::from_raw(read_i32(input, 20)),
        PlanarAcceleration::from_raw(read_i32(input, 24)),
        PlanarAcceleration::from_raw(read_i32(input, 28)),
        Mass::from_raw(read_i32(input, 32)),
        Mass::from_raw(read_i32(input, 36)),
        PitchAngle::from_raw(read_u16(input, 40)),
        input[42],
        phase,
        Mach::from_raw(read_i32(input, 44)),
        DynamicPressure::from_raw(read_i32(input, 48)),
        status,
        events,
        read_u32(input, 56),
    ))
}

pub trait Phase2TelemetrySink {
    type Error;
    fn write_header(
        &mut self,
        header: &[u8; PHASE2_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error>;
    fn write_frame(
        &mut self,
        frame: &[u8; PHASE2_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct Phase2TelemetrySummary {
    mission: Phase2MissionResult,
    frames_written: u32,
}

impl Phase2TelemetrySummary {
    pub const fn mission(self) -> Phase2MissionResult {
        self.mission
    }
    pub const fn frames_written(self) -> u32 {
        self.frames_written
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2TelemetryFailure<E> {
    Header(E),
    Frame {
        error: E,
        frames_written: u32,
    },
    Mission {
        error: Phase2MissionError,
        frames_written: u32,
    },
}

struct Emitter<'a, S: Phase2TelemetrySink> {
    sink: &'a mut S,
    stride: u32,
    pending_events: u16,
    frames_written: u32,
}

impl<S: Phase2TelemetrySink> Phase2Observer for Emitter<'_, S> {
    type Error = S::Error;
    fn observe(&mut self, observation: Phase2Observation) -> Result<(), Self::Error> {
        self.pending_events |= observation.events();
        let step = observation.truth().step();
        let terminal = observation.events() & EVENT_END != 0;
        if step == 0 || (step / self.stride) * self.stride == step || terminal {
            let frame = Phase2TelemetryFrame::from_observation(observation, self.pending_events);
            let mut output = [0u8; PHASE2_TELEMETRY_FRAME_LENGTH];
            let written = write_phase2_telemetry_frame(&frame, &mut output);
            debug_assert_eq!(written, Ok(()));
            self.sink.write_frame(&output)?;
            self.frames_written += 1;
            self.pending_events = 0;
        }
        Ok(())
    }
}

pub fn run_phase2_mission_with_telemetry<S: Phase2TelemetrySink>(
    scenario: &Phase2Scenario,
    sink: &mut S,
) -> Result<Phase2TelemetrySummary, Phase2TelemetryFailure<S::Error>> {
    let mut header = [0u8; PHASE2_TELEMETRY_HEADER_LENGTH];
    let written = write_phase2_telemetry_header(scenario, &mut header);
    debug_assert_eq!(written, Ok(()));
    sink.write_header(&header)
        .map_err(Phase2TelemetryFailure::Header)?;
    let mut emitter = Emitter {
        sink,
        stride: scenario.telemetry_stride() as u32,
        pending_events: 0,
        frames_written: 0,
    };
    match execute_phase2_mission_observed(scenario, &mut emitter) {
        Ok(mission) => Ok(Phase2TelemetrySummary {
            mission,
            frames_written: emitter.frames_written,
        }),
        Err(Phase2ExecutionError::Mission(error)) => Err(Phase2TelemetryFailure::Mission {
            error,
            frames_written: emitter.frames_written,
        }),
        Err(Phase2ExecutionError::Observer(error)) => Err(Phase2TelemetryFailure::Frame {
            error,
            frames_written: emitter.frames_written,
        }),
    }
}

pub fn validate_phase2_telemetry_frame_numeric(frame: Phase2TelemetryFrame) -> bool {
    let mut status = NumericStatus::CLEAR;
    let valid = frame.pitch().is_phase2_valid()
        && frame.active_stage() < 4
        && frame.mach().raw() >= 0
        && frame.dynamic_pressure().raw() >= 0
        && frame.total_mass().raw() > 0
        && frame.propellant().raw() >= 0;
    let _ = &mut status;
    valid
}
