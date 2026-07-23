//! Canonical allocation-free Phase 5 spatial telemetry (`KST5`).

use crate::phase5_closed_loop::Phase5ClosedLoopStep;
use crate::phase5_mission::{
    run_phase5_mission_observed, Phase5MissionCase, Phase5MissionObserver, Phase5MissionSummary,
    Phase5ObservedMissionError,
};
use crate::phase5_vehicle::{Phase5StagePhase, Phase5VehicleSnapshot};
use ksa64_core::phase5_contract::{
    PHASE5_BASE_KSC2_CRC32, PHASE5_ENVIRONMENT_ID, PHASE5_MISSION_STEPS, PHASE5_MISSION_STEP_Q16,
    PHASE5_NUMERIC_CONTRACT_ID, PHASE5_SCENARIO_ID,
};
use ksa64_flight::phase5_navigation::{SpatialNavigation, SpatialNavigationState};
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, parse_spatial_sensor_frame, write_spatial_actuator_command,
    write_spatial_sensor_frame, SpatialActuatorCommand, SpatialSensorFrame, EVENT_MASK,
    SENSOR_VALID_MASK, SPATIAL_ACTUATOR_COMMAND_LENGTH, SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_interface::{crc32_ieee, FlightMode, ALARM_MASK};

pub const PHASE5_TELEMETRY_VERSION: u16 = 5;
pub const PHASE5_TELEMETRY_HEADER_LENGTH: usize = 96;
pub const PHASE5_TELEMETRY_FRAME_LENGTH: usize = 424;
pub const PHASE5_TELEMETRY_CADENCE: u16 = 1;
pub const PHASE5_TELEMETRY_CONTRACT_ID: u32 = 0x0500_0001;
pub const PHASE5_AVIONICS_SIGNATURE: u32 = 0xaa0a_0b0e;
pub const PHASE5_COORDINATE_FRAME_ECI: u8 = 1;
pub const PHASE5_QUATERNION_BODY_TO_ECI_HAMILTON: u8 = 1;
pub const PHASE5_FRAME_TERMINAL: u16 = 1;
pub const PHASE5_FRAME_EVENT: u16 = 2;
const FRAME_FLAG_MASK: u16 = 3;
const HEADER_CRC_OFFSET: usize = 92;
const FRAME_CRC_OFFSET: usize = 420;
const SENSOR_OFFSET: usize = 176;
const NAVIGATION_OFFSET: usize = 304;
const COMMAND_OFFSET: usize = 368;
const CHECKSUM_OFFSET: u32 = 2_166_136_261;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5TelemetryError {
    Length,
    Magic,
    Version,
    HeaderLength,
    FrameLength,
    Contract,
    Identity,
    Timestep,
    Cadence,
    MissionSteps,
    Reserved,
    Flags,
    Enum,
    Sequence,
    Checksum,
    EmbeddedSensor,
    EmbeddedCommand,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5TelemetryHeader {
    pub seed: u32,
    pub case: Phase5MissionCase,
    pub vehicle_signature: u32,
    pub avionics_signature: u32,
    pub guidance_signature: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5TelemetryFrame {
    pub step: u32,
    pub mission_time_q16: i32,
    pub frame_flags: u16,
    pub events: u16,
    pub active_stage: u8,
    pub stage_phase: Phase5StagePhase,
    pub engine_on: bool,
    pub mode: FlightMode,
    pub alarms: u16,
    pub sensor_validity: u16,
    pub total_mass_q12: i32,
    pub propellant_q12: i32,
    pub rcs_propellant_q12: i32,
    pub mach_q16: i32,
    pub dynamic_pressure_q16: i32,
    pub angle_of_attack_sine_q16: i32,
    pub position_q12: [i32; 3],
    pub velocity_q24: [i32; 3],
    pub acceleration_q28: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub angular_rate_q24: [i32; 3],
    pub flexible_q24: [i32; 8],
    pub inertia_q12: [i32; 3],
    pub gimbal_requested_q16: [i32; 2],
    pub gimbal_lagged_q16: [i32; 2],
    pub gimbal_applied_q16: [i32; 2],
    pub sensor: SpatialSensorFrame,
    pub navigation: SpatialNavigationState,
    pub command: SpatialActuatorCommand,
    pub sensor_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub observation_checksum: u32,
}
impl Phase5TelemetryFrame {
    pub const fn terminal(self) -> bool {
        self.frame_flags & PHASE5_FRAME_TERMINAL != 0
    }
    pub const fn event_record(self) -> bool {
        self.frame_flags & PHASE5_FRAME_EVENT != 0
    }
}

pub trait Phase5TelemetrySink {
    type Error;
    fn write_header(
        &mut self,
        header: &[u8; PHASE5_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error>;
    fn write_frame(
        &mut self,
        frame: &[u8; PHASE5_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5TelemetryObserverError<E> {
    Codec(Phase5TelemetryError),
    Sink(E),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5TelemetryRunError<E> {
    Mission(crate::phase5_closed_loop::Phase5ClosedLoopError),
    Codec(Phase5TelemetryError),
    Sink(E),
}
pub struct Phase5TelemetryObserver<'a, S: Phase5TelemetrySink> {
    sink: &'a mut S,
    observation_checksum: u32,
}
impl<'a, S: Phase5TelemetrySink> Phase5TelemetryObserver<'a, S> {
    pub const fn new(sink: &'a mut S) -> Self {
        Self {
            sink,
            observation_checksum: CHECKSUM_OFFSET,
        }
    }
    fn emit(
        &mut self,
        mut frame: Phase5TelemetryFrame,
    ) -> Result<(), Phase5TelemetryObserverError<S::Error>> {
        frame.observation_checksum = 0;
        let mut bytes = [0u8; PHASE5_TELEMETRY_FRAME_LENGTH];
        write_phase5_telemetry_frame(&frame, &mut bytes)
            .map_err(Phase5TelemetryObserverError::Codec)?;
        self.observation_checksum = hash_bytes(self.observation_checksum, &bytes[..412]);
        frame.observation_checksum = self.observation_checksum;
        write_phase5_telemetry_frame(&frame, &mut bytes)
            .map_err(Phase5TelemetryObserverError::Codec)?;
        self.sink
            .write_frame(&bytes)
            .map_err(Phase5TelemetryObserverError::Sink)
    }
}
impl<S: Phase5TelemetrySink> Phase5MissionObserver for Phase5TelemetryObserver<'_, S> {
    type Error = Phase5TelemetryObserverError<S::Error>;
    fn observe_initial(
        &mut self,
        case: Phase5MissionCase,
        seed: u32,
        snapshot: Phase5VehicleSnapshot,
    ) -> Result<(), Self::Error> {
        let header = Phase5TelemetryHeader {
            seed,
            case,
            vehicle_signature: crate::phase5_vehicle::PHASE5_VEHICLE_SIGNATURE,
            avionics_signature: PHASE5_AVIONICS_SIGNATURE,
            guidance_signature: ksa64_flight::phase5_guidance::GUIDANCE_SIGNATURE,
        };
        let mut bytes = [0u8; PHASE5_TELEMETRY_HEADER_LENGTH];
        write_phase5_telemetry_header(header, &mut bytes)
            .map_err(Phase5TelemetryObserverError::Codec)?;
        self.sink
            .write_header(&bytes)
            .map_err(Phase5TelemetryObserverError::Sink)?;
        self.emit(initial_frame(snapshot))
    }
    fn observe_step(
        &mut self,
        _case: Phase5MissionCase,
        step: Phase5ClosedLoopStep,
        terminal: bool,
    ) -> Result<(), Self::Error> {
        self.emit(frame_from_step(step, terminal))
    }
}
pub fn run_phase5_mission_with_telemetry<S: Phase5TelemetrySink>(
    case: Phase5MissionCase,
    sink: &mut S,
) -> Result<Phase5MissionSummary, Phase5TelemetryRunError<S::Error>> {
    let mut observer = Phase5TelemetryObserver::new(sink);
    match run_phase5_mission_observed(case, &mut observer) {
        Ok(v) => Ok(v),
        Err(Phase5ObservedMissionError::Mission(e)) => Err(Phase5TelemetryRunError::Mission(e)),
        Err(Phase5ObservedMissionError::Observer(Phase5TelemetryObserverError::Codec(e))) => {
            Err(Phase5TelemetryRunError::Codec(e))
        }
        Err(Phase5ObservedMissionError::Observer(Phase5TelemetryObserverError::Sink(e))) => {
            Err(Phase5TelemetryRunError::Sink(e))
        }
    }
}
pub fn write_phase5_telemetry_header(
    header: Phase5TelemetryHeader,
    out: &mut [u8],
) -> Result<(), Phase5TelemetryError> {
    if out.len() != PHASE5_TELEMETRY_HEADER_LENGTH {
        return Err(Phase5TelemetryError::Length);
    }
    out.fill(0);
    out[..4].copy_from_slice(b"KST5");
    pu16(out, 4, PHASE5_TELEMETRY_VERSION);
    pu16(out, 6, 96);
    pu16(out, 8, 424);
    pu16(out, 10, PHASE5_TELEMETRY_CADENCE);
    pu32(out, 12, PHASE5_TELEMETRY_CONTRACT_ID);
    pu32(out, 16, PHASE5_NUMERIC_CONTRACT_ID);
    pu32(out, 20, PHASE5_SCENARIO_ID);
    pu32(out, 24, PHASE5_ENVIRONMENT_ID);
    pu32(out, 28, PHASE5_BASE_KSC2_CRC32);
    pu32(out, 32, header.seed);
    out[36] = header.case as u8;
    out[37] = PHASE5_COORDINATE_FRAME_ECI;
    out[38] = PHASE5_QUATERNION_BODY_TO_ECI_HAMILTON;
    pi32(out, 40, PHASE5_MISSION_STEP_Q16);
    pu32(out, 44, PHASE5_MISSION_STEPS);
    pu32(out, 48, header.vehicle_signature);
    pu32(out, 52, header.avionics_signature);
    pu32(out, 56, header.guidance_signature);
    pu32(
        out,
        HEADER_CRC_OFFSET,
        crc32_ieee(&out[..HEADER_CRC_OFFSET]),
    );
    Ok(())
}
pub fn parse_phase5_telemetry_header(
    input: &[u8],
) -> Result<Phase5TelemetryHeader, Phase5TelemetryError> {
    if input.len() != 96 {
        return Err(Phase5TelemetryError::Length);
    }
    if &input[..4] != b"KST5" {
        return Err(Phase5TelemetryError::Magic);
    }
    if gu16(input, 4) != 5 {
        return Err(Phase5TelemetryError::Version);
    }
    if gu16(input, 6) != 96 {
        return Err(Phase5TelemetryError::HeaderLength);
    }
    if gu16(input, 8) != 424 {
        return Err(Phase5TelemetryError::FrameLength);
    }
    if gu16(input, 10) != 1 {
        return Err(Phase5TelemetryError::Cadence);
    }
    if gu32(input, 12) != PHASE5_TELEMETRY_CONTRACT_ID
        || gu32(input, 16) != PHASE5_NUMERIC_CONTRACT_ID
    {
        return Err(Phase5TelemetryError::Contract);
    }
    if gu32(input, 20) != PHASE5_SCENARIO_ID
        || gu32(input, 24) != PHASE5_ENVIRONMENT_ID
        || gu32(input, 28) != PHASE5_BASE_KSC2_CRC32
        || input[37] != 1
        || input[38] != 1
        || gu32(input, 48) != crate::phase5_vehicle::PHASE5_VEHICLE_SIGNATURE
        || gu32(input, 52) != PHASE5_AVIONICS_SIGNATURE
        || gu32(input, 56) != ksa64_flight::phase5_guidance::GUIDANCE_SIGNATURE
        || gu32(input, 32) != (0x5a00_0000 | input[36] as u32)
    {
        return Err(Phase5TelemetryError::Identity);
    }
    if input[39] != 0 || input[60..92].iter().any(|&b| b != 0) {
        return Err(Phase5TelemetryError::Reserved);
    }
    if gi32(input, 40) != PHASE5_MISSION_STEP_Q16 {
        return Err(Phase5TelemetryError::Timestep);
    }
    if gu32(input, 44) != PHASE5_MISSION_STEPS {
        return Err(Phase5TelemetryError::MissionSteps);
    }
    if gu32(input, HEADER_CRC_OFFSET) != crc32_ieee(&input[..HEADER_CRC_OFFSET]) {
        return Err(Phase5TelemetryError::Checksum);
    }
    Ok(Phase5TelemetryHeader {
        seed: gu32(input, 32),
        case: parse_case(input[36])?,
        vehicle_signature: gu32(input, 48),
        avionics_signature: gu32(input, 52),
        guidance_signature: gu32(input, 56),
    })
}

pub fn write_phase5_telemetry_frame(
    f: &Phase5TelemetryFrame,
    out: &mut [u8],
) -> Result<(), Phase5TelemetryError> {
    if out.len() != 424 {
        return Err(Phase5TelemetryError::Length);
    }
    if f.frame_flags & !FRAME_FLAG_MASK != 0
        || f.events & !EVENT_MASK != 0
        || f.alarms & !ALARM_MASK != 0
        || f.sensor_validity & !SENSOR_VALID_MASK != 0
    {
        return Err(Phase5TelemetryError::Flags);
    }
    out.fill(0);
    pu32(out, 0, f.step);
    pi32(out, 4, f.mission_time_q16);
    pu16(out, 8, f.frame_flags);
    pu16(out, 10, f.events);
    out[12] = f.active_stage;
    out[13] = f.stage_phase as u8;
    out[14] = f.engine_on as u8;
    out[15] = f.mode as u8;
    pu16(out, 16, f.alarms);
    pu16(out, 18, f.sensor_validity);
    for (o, v) in [
        (20, f.total_mass_q12),
        (24, f.propellant_q12),
        (28, f.rcs_propellant_q12),
        (32, f.mach_q16),
        (36, f.dynamic_pressure_q16),
        (40, f.angle_of_attack_sine_q16),
    ] {
        pi32(out, o, v)
    }
    pia(out, 44, &f.position_q12);
    pia(out, 56, &f.velocity_q24);
    pia(out, 68, &f.acceleration_q28);
    pia(out, 80, &f.attitude_q30);
    pia(out, 96, &f.angular_rate_q24);
    pia(out, 108, &f.flexible_q24);
    pia(out, 140, &f.inertia_q12);
    pia(out, 152, &f.gimbal_requested_q16);
    pia(out, 160, &f.gimbal_lagged_q16);
    pia(out, 168, &f.gimbal_applied_q16);
    write_spatial_sensor_frame(
        &f.sensor,
        &mut out[SENSOR_OFFSET..SENSOR_OFFSET + SPATIAL_SENSOR_FRAME_LENGTH],
    )
    .map_err(|_| Phase5TelemetryError::EmbeddedSensor)?;
    pu32(out, NAVIGATION_OFFSET, f.navigation.sequence);
    pi32(out, 308, f.navigation.time_q16);
    pia(out, 312, &f.navigation.position_q12);
    pia(out, 324, &f.navigation.velocity_q24);
    pia(out, 336, &f.navigation.attitude_q30);
    pia(out, 352, &f.navigation.angular_rate_q24);
    let aids = u16::from(f.navigation.gps_aided)
        | (u16::from(f.navigation.star_aided) << 1)
        | (u16::from(f.navigation.barometer_aided) << 2);
    pu16(out, 364, aids);
    write_spatial_actuator_command(
        &f.command,
        &mut out[COMMAND_OFFSET..COMMAND_OFFSET + SPATIAL_ACTUATOR_COMMAND_LENGTH],
    )
    .map_err(|_| Phase5TelemetryError::EmbeddedCommand)?;
    pu32(out, 400, f.sensor_checksum);
    pu32(out, 404, f.navigation_checksum);
    pu32(out, 408, f.flight_checksum);
    pu32(out, 412, f.observation_checksum);
    pu32(out, FRAME_CRC_OFFSET, crc32_ieee(&out[..FRAME_CRC_OFFSET]));
    Ok(())
}
pub fn parse_phase5_telemetry_frame(
    input: &[u8],
) -> Result<Phase5TelemetryFrame, Phase5TelemetryError> {
    if input.len() != 424 {
        return Err(Phase5TelemetryError::Length);
    }
    if gu32(input, FRAME_CRC_OFFSET) != crc32_ieee(&input[..FRAME_CRC_OFFSET]) {
        return Err(Phase5TelemetryError::Checksum);
    }
    if input[416..420].iter().any(|&b| b != 0) {
        return Err(Phase5TelemetryError::Reserved);
    }
    let flags = gu16(input, 8);
    let events = gu16(input, 10);
    let alarms = gu16(input, 16);
    let validity = gu16(input, 18);
    if flags & !FRAME_FLAG_MASK != 0
        || events & !EVENT_MASK != 0
        || alarms & !ALARM_MASK != 0
        || validity & !SENSOR_VALID_MASK != 0
        || input[14] > 1
        || (input[14] != 0) != (input[13] == Phase5StagePhase::Burning as u8)
    {
        return Err(Phase5TelemetryError::Flags);
    }
    let sensor = parse_spatial_sensor_frame(
        &input[SENSOR_OFFSET..SENSOR_OFFSET + SPATIAL_SENSOR_FRAME_LENGTH],
    )
    .map_err(|_| Phase5TelemetryError::EmbeddedSensor)?;
    let command = parse_spatial_actuator_command(
        &input[COMMAND_OFFSET..COMMAND_OFFSET + SPATIAL_ACTUATOR_COMMAND_LENGTH],
    )
    .map_err(|_| Phase5TelemetryError::EmbeddedCommand)?;
    let aids = gu16(input, 364);
    if aids & !7 != 0 || gu16(input, 366) != 0 {
        return Err(Phase5TelemetryError::Reserved);
    }
    let step = gu32(input, 0);
    let sequence = gu32(input, NAVIGATION_OFFSET);
    let expected = if step == 0 { 0 } else { step - 1 };
    if sensor.sequence != expected
        || sequence != expected
        || command.sequence != expected
        || validity != sensor.validity
    {
        return Err(Phase5TelemetryError::Sequence);
    }
    let navigation_checksum = gu32(input, 404);
    Ok(Phase5TelemetryFrame {
        step,
        mission_time_q16: gi32(input, 4),
        frame_flags: flags,
        events,
        active_stage: input[12],
        stage_phase: parse_stage(input[13])?,
        engine_on: input[14] != 0,
        mode: parse_mode(input[15])?,
        alarms,
        sensor_validity: validity,
        total_mass_q12: gi32(input, 20),
        propellant_q12: gi32(input, 24),
        rcs_propellant_q12: gi32(input, 28),
        mach_q16: gi32(input, 32),
        dynamic_pressure_q16: gi32(input, 36),
        angle_of_attack_sine_q16: gi32(input, 40),
        position_q12: gia(input, 44),
        velocity_q24: gia(input, 56),
        acceleration_q28: gia(input, 68),
        attitude_q30: gia(input, 80),
        angular_rate_q24: gia(input, 96),
        flexible_q24: gia(input, 108),
        inertia_q12: gia(input, 140),
        gimbal_requested_q16: gia(input, 152),
        gimbal_lagged_q16: gia(input, 160),
        gimbal_applied_q16: gia(input, 168),
        sensor,
        navigation: SpatialNavigationState {
            sequence,
            time_q16: gi32(input, 308),
            position_q12: gia(input, 312),
            velocity_q24: gia(input, 324),
            attitude_q30: gia(input, 336),
            angular_rate_q24: gia(input, 352),
            gps_aided: aids & 1 != 0,
            star_aided: aids & 2 != 0,
            barometer_aided: aids & 4 != 0,
            checksum: navigation_checksum,
        },
        command,
        sensor_checksum: gu32(input, 400),
        navigation_checksum,
        flight_checksum: gu32(input, 408),
        observation_checksum: gu32(input, 412),
    })
}

pub(crate) fn initial_frame(s: Phase5VehicleSnapshot) -> Phase5TelemetryFrame {
    let mut sensor = SpatialSensorFrame::ZERO;
    sensor.rcs_propellant_q12 = s.rcs_propellant_q12;
    sensor.active_stage = s.truth.active_stage();
    sensor.stage_phase = map_stage(s.truth.phase());
    frame_from_parts(
        s,
        sensor,
        SpatialNavigation::new().state(),
        SpatialActuatorCommand::SAFE,
        FlightMode::Boot,
        0,
        CHECKSUM_OFFSET,
        CHECKSUM_OFFSET,
        CHECKSUM_OFFSET,
        false,
    )
}
fn frame_from_step(s: Phase5ClosedLoopStep, terminal: bool) -> Phase5TelemetryFrame {
    frame_from_parts(
        s.vehicle,
        s.sensor,
        s.flight.navigation,
        s.flight.command,
        s.flight.mode,
        s.flight.alarms,
        s.sensor_checksum,
        s.flight.navigation.checksum,
        s.flight.flight_checksum,
        terminal,
    )
}
#[allow(clippy::too_many_arguments)]
fn frame_from_parts(
    s: Phase5VehicleSnapshot,
    sensor: SpatialSensorFrame,
    navigation: SpatialNavigationState,
    command: SpatialActuatorCommand,
    mode: FlightMode,
    alarms: u16,
    sensor_checksum: u32,
    navigation_checksum: u32,
    flight_checksum: u32,
    terminal: bool,
) -> Phase5TelemetryFrame {
    let t = s.truth;
    let spatial = t.spatial();
    let rigid = t.rigid();
    let flex = t.flexible();
    let g = s.gimbal;
    let events = s.events | sensor.events;
    let mut frame_flags = if terminal { PHASE5_FRAME_TERMINAL } else { 0 };
    if events != 0 {
        frame_flags |= PHASE5_FRAME_EVENT
    }
    Phase5TelemetryFrame {
        step: t.step(),
        mission_time_q16: t.time_q16(),
        frame_flags,
        events,
        active_stage: t.active_stage(),
        stage_phase: t.phase(),
        engine_on: t.phase() == Phase5StagePhase::Burning,
        mode,
        alarms,
        sensor_validity: sensor.validity,
        total_mass_q12: t.total_mass_q12(),
        propellant_q12: t.active_propellant_q12(),
        rcs_propellant_q12: s.rcs_propellant_q12,
        mach_q16: s.mach.raw(),
        dynamic_pressure_q16: s.dynamic_pressure_q16,
        angle_of_attack_sine_q16: s.angle_of_attack_sine_q16,
        position_q12: v3(spatial.position()),
        velocity_q24: v3(spatial.velocity()),
        acceleration_q28: v3(spatial.acceleration()),
        attitude_q30: q4(rigid.attitude()),
        angular_rate_q24: v3(rigid.angular_rate()),
        flexible_q24: flex8(flex),
        inertia_q12: [s.inertia.x(), s.inertia.y(), s.inertia.z()],
        gimbal_requested_q16: [g.requested.pitch, g.requested.yaw],
        gimbal_lagged_q16: [g.lagged.pitch, g.lagged.yaw],
        gimbal_applied_q16: [g.applied.pitch, g.applied.yaw],
        sensor,
        navigation,
        command,
        sensor_checksum,
        navigation_checksum,
        flight_checksum,
        observation_checksum: 0,
    }
}
fn v3<const F: u8>(v: ksa64_core::spatial_numeric::FixedVec3<F>) -> [i32; 3] {
    [v.x(), v.y(), v.z()]
}
fn q4(q: ksa64_core::spatial_numeric::QuaternionQ30) -> [i32; 4] {
    [q.w(), q.x(), q.y(), q.z()]
}
fn flex8(v: ksa64_core::flexible::FlexibleStateQ24) -> [i32; 8] {
    [
        v.y().bending().displacement(),
        v.y().bending().rate(),
        v.y().slosh().displacement(),
        v.y().slosh().rate(),
        v.z().bending().displacement(),
        v.z().bending().rate(),
        v.z().slosh().displacement(),
        v.z().slosh().rate(),
    ]
}
fn map_stage(v: Phase5StagePhase) -> ksa64_interface::StagePhase {
    match v {
        Phase5StagePhase::CoastBeforeIgnition => ksa64_interface::StagePhase::CoastBeforeIgnition,
        Phase5StagePhase::Burning => ksa64_interface::StagePhase::Burning,
        Phase5StagePhase::CoastBeforeSeparation => {
            ksa64_interface::StagePhase::CoastBeforeSeparation
        }
        Phase5StagePhase::Complete => ksa64_interface::StagePhase::Complete,
    }
}
fn parse_stage(v: u8) -> Result<Phase5StagePhase, Phase5TelemetryError> {
    match v {
        0 => Ok(Phase5StagePhase::CoastBeforeIgnition),
        1 => Ok(Phase5StagePhase::Burning),
        2 => Ok(Phase5StagePhase::CoastBeforeSeparation),
        3 => Ok(Phase5StagePhase::Complete),
        _ => Err(Phase5TelemetryError::Enum),
    }
}
fn parse_case(v: u8) -> Result<Phase5MissionCase, Phase5TelemetryError> {
    match v {
        0 => Ok(Phase5MissionCase::Nominal),
        1 => Ok(Phase5MissionCase::GustAndSlosh),
        2 => Ok(Phase5MissionCase::StarOutageAndGyroBias),
        3 => Ok(Phase5MissionCase::GimbalJamAbort),
        4 => Ok(Phase5MissionCase::DampingLossAbort),
        5 => Ok(Phase5MissionCase::RcsLeakAndDepletion),
        _ => Err(Phase5TelemetryError::Enum),
    }
}
fn parse_mode(v: u8) -> Result<FlightMode, Phase5TelemetryError> {
    match v {
        0 => Ok(FlightMode::Boot),
        1 => Ok(FlightMode::Prelaunch),
        2 => Ok(FlightMode::ProgrammedAscent),
        3 => Ok(FlightMode::StageTransition),
        4 => Ok(FlightMode::Insertion),
        5 => Ok(FlightMode::Coast),
        6 => Ok(FlightMode::Complete),
        7 => Ok(FlightMode::Abort),
        _ => Err(Phase5TelemetryError::Enum),
    }
}
fn pu16(o: &mut [u8], p: usize, v: u16) {
    o[p..p + 2].copy_from_slice(&v.to_le_bytes())
}
fn pu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn pi32(o: &mut [u8], p: usize, v: i32) {
    pu32(o, p, v as u32)
}
fn gu16(i: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([i[p], i[p + 1]])
}
fn gu32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([i[p], i[p + 1], i[p + 2], i[p + 3]])
}
fn gi32(i: &[u8], p: usize) -> i32 {
    gu32(i, p) as i32
}
fn pia(o: &mut [u8], p: usize, v: &[i32]) {
    for (n, x) in v.iter().enumerate() {
        pi32(o, p + n * 4, *x)
    }
}
fn gia<const N: usize>(i: &[u8], p: usize) -> [i32; N] {
    let mut v = [0; N];
    let mut n = 0;
    while n < N {
        v[n] = gi32(i, p + n * 4);
        n += 1
    }
    v
}
fn hash_bytes(mut h: u32, b: &[u8]) -> u32 {
    for x in b {
        h ^= *x as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
