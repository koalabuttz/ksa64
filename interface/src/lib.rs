#![no_std]

//! Transport-neutral Phase 3 messages and strict fixed-width codecs.
pub mod phase10;
pub mod phase11;
mod phase11_procedure;
pub mod phase5;
pub mod phase6;
pub mod phase6_transport;
pub mod phase8_5;
pub mod phase9_5;

pub const SENSOR_FRAME_LENGTH: usize = 56;
pub const ACTUATOR_COMMAND_LENGTH: usize = 16;
pub const FLIGHT_OUTPUT_LENGTH: usize = 52;

pub const SENSOR_VALID_ACCEL: u16 = 1 << 0;
pub const SENSOR_VALID_GYRO: u16 = 1 << 1;
pub const SENSOR_VALID_STEERING: u16 = 1 << 2;
pub const SENSOR_VALID_ALTIMETER: u16 = 1 << 3;
pub const SENSOR_VALID_GPS: u16 = 1 << 4;
pub const SENSOR_VALID_CLOCK: u16 = 1 << 5;
pub const SENSOR_VALID_MASK: u16 = (1 << 6) - 1;
pub const EVENT_IGNITION: u16 = 1 << 0;
pub const EVENT_CUTOFF: u16 = 1 << 1;
pub const EVENT_SEPARATION: u16 = 1 << 2;
pub const EVENT_IMPACT: u16 = 1 << 3;
pub const EVENT_END: u16 = 1 << 4;
pub const EVENT_GPS_ACQUIRED: u16 = 1 << 5;
pub const EVENT_GPS_LOST: u16 = 1 << 6;
pub const EVENT_ABORT: u16 = 1 << 7;
pub const EVENT_MASK: u16 = (1 << 8) - 1;
pub const ALARM_SENSOR_FRAME: u16 = 1 << 0;
pub const ALARM_NAVIGATION: u16 = 1 << 1;
pub const ALARM_STEERING: u16 = 1 << 2;
pub const ALARM_ABORT: u16 = 1 << 3;
pub const ALARM_COMMAND_REJECTED: u16 = 1 << 4;
pub const ALARM_MASK: u16 = (1 << 5) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StagePhase {
    CoastBeforeIgnition,
    Burning,
    CoastBeforeSeparation,
    Complete,
}
impl StagePhase {
    fn parse(v: u8) -> Result<Self, CodecError> {
        match v {
            0 => Ok(Self::CoastBeforeIgnition),
            1 => Ok(Self::Burning),
            2 => Ok(Self::CoastBeforeSeparation),
            3 => Ok(Self::Complete),
            _ => Err(CodecError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EngineAction {
    Hold,
    Ignite,
    Cutoff,
}
impl EngineAction {
    fn parse(v: u8) -> Result<Self, CodecError> {
        match v {
            0 => Ok(Self::Hold),
            1 => Ok(Self::Ignite),
            2 => Ok(Self::Cutoff),
            _ => Err(CodecError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FlightMode {
    Boot,
    Prelaunch,
    ProgrammedAscent,
    StageTransition,
    Insertion,
    Coast,
    Complete,
    Abort,
}
impl FlightMode {
    fn parse(v: u8) -> Result<Self, CodecError> {
        match v {
            0 => Ok(Self::Boot),
            1 => Ok(Self::Prelaunch),
            2 => Ok(Self::ProgrammedAscent),
            3 => Ok(Self::StageTransition),
            4 => Ok(Self::Insertion),
            5 => Ok(Self::Coast),
            6 => Ok(Self::Complete),
            7 => Ok(Self::Abort),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Length,
    Checksum,
    Reserved,
    Flags,
    Enum,
    Sequence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorFrame {
    pub sequence: u32,
    pub onboard_time_q16: i32,
    pub accel_radial_q28: i32,
    pub accel_tangential_q28: i32,
    pub gyro_rate_q24: i32,
    pub steering_pitch: u16,
    pub validity: u16,
    pub altitude_q12: i32,
    pub gps_radius_q12: i32,
    pub gps_downrange_q32: i32,
    pub gps_radial_velocity_q24: i32,
    pub gps_tangential_velocity_q24: i32,
    pub events: u16,
    pub active_stage: u8,
    pub stage_phase: StagePhase,
    pub engine_on: bool,
}
impl SensorFrame {
    pub const ZERO: Self = Self {
        sequence: 0,
        onboard_time_q16: 0,
        accel_radial_q28: 0,
        accel_tangential_q28: 0,
        gyro_rate_q24: 0,
        steering_pitch: 0,
        validity: 0,
        altitude_q12: 0,
        gps_radius_q12: 0,
        gps_downrange_q32: 0,
        gps_radial_velocity_q24: 0,
        gps_tangential_velocity_q24: 0,
        events: 0,
        active_stage: 0,
        stage_phase: StagePhase::CoastBeforeIgnition,
        engine_on: false,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActuatorCommand {
    pub sequence: u32,
    pub desired_pitch: u16,
    pub engine_action: EngineAction,
    pub separate: bool,
    pub abort_safeing: bool,
    pub recovery_requested: bool,
}
impl ActuatorCommand {
    pub const SAFE: Self = Self {
        sequence: 0,
        desired_pitch: 0,
        engine_action: EngineAction::Cutoff,
        separate: false,
        abort_safeing: true,
        recovery_requested: false,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightOutput {
    pub sequence: u32,
    pub nav_time_q16: i32,
    pub nav_radius_q12: i32,
    pub nav_downrange_q32: i32,
    pub nav_radial_velocity_q24: i32,
    pub nav_tangential_velocity_q24: i32,
    pub nav_pitch: u16,
    pub mode: FlightMode,
    pub alarms: u16,
    pub command: ActuatorCommand,
    pub nav_checksum: u32,
    pub flight_checksum: u32,
}

fn put_u16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
}
fn put_u32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes())
}
fn put_i32(o: &mut [u8], i: usize, v: i32) {
    put_u32(o, i, v as u32)
}
fn get_u16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn get_u32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn get_i32(b: &[u8], i: usize) -> i32 {
    get_u32(b, i) as i32
}
pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    let mut i = 0;
    while i < bytes.len() {
        c ^= bytes[i] as u32;
        let mut bit = 0;
        while bit < 8 {
            c = (c >> 1) ^ (0xedb8_8320u32 & (0u32.wrapping_sub(c & 1)));
            bit += 1
        }
        i += 1
    }
    !c
}

pub fn write_sensor_frame(f: &SensorFrame, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != SENSOR_FRAME_LENGTH {
        return Err(CodecError::Length);
    }
    if f.validity & !SENSOR_VALID_MASK != 0 || f.events & !EVENT_MASK != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    put_u32(o, 0, f.sequence);
    put_i32(o, 4, f.onboard_time_q16);
    put_i32(o, 8, f.accel_radial_q28);
    put_i32(o, 12, f.accel_tangential_q28);
    put_i32(o, 16, f.gyro_rate_q24);
    put_u16(o, 20, f.steering_pitch);
    put_u16(o, 22, f.validity);
    put_i32(o, 24, f.altitude_q12);
    put_i32(o, 28, f.gps_radius_q12);
    put_i32(o, 32, f.gps_downrange_q32);
    put_i32(o, 36, f.gps_radial_velocity_q24);
    put_i32(o, 40, f.gps_tangential_velocity_q24);
    put_u16(o, 44, f.events);
    o[46] = f.active_stage;
    o[47] = f.stage_phase as u8;
    o[48] = f.engine_on as u8;
    put_u32(o, 52, crc32_ieee(&o[..52]));
    Ok(())
}
pub fn parse_sensor_frame(b: &[u8]) -> Result<SensorFrame, CodecError> {
    if b.len() != SENSOR_FRAME_LENGTH {
        return Err(CodecError::Length);
    }
    if crc32_ieee(&b[..52]) != get_u32(b, 52) {
        return Err(CodecError::Checksum);
    }
    if b[49] != 0 || b[50] != 0 || b[51] != 0 {
        return Err(CodecError::Reserved);
    }
    let validity = get_u16(b, 22);
    let events = get_u16(b, 44);
    if validity & !SENSOR_VALID_MASK != 0 || events & !EVENT_MASK != 0 || b[48] > 1 {
        return Err(CodecError::Flags);
    }
    Ok(SensorFrame {
        sequence: get_u32(b, 0),
        onboard_time_q16: get_i32(b, 4),
        accel_radial_q28: get_i32(b, 8),
        accel_tangential_q28: get_i32(b, 12),
        gyro_rate_q24: get_i32(b, 16),
        steering_pitch: get_u16(b, 20),
        validity,
        altitude_q12: get_i32(b, 24),
        gps_radius_q12: get_i32(b, 28),
        gps_downrange_q32: get_i32(b, 32),
        gps_radial_velocity_q24: get_i32(b, 36),
        gps_tangential_velocity_q24: get_i32(b, 40),
        events,
        active_stage: b[46],
        stage_phase: StagePhase::parse(b[47])?,
        engine_on: b[48] != 0,
    })
}
pub fn write_actuator_command(c: &ActuatorCommand, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != ACTUATOR_COMMAND_LENGTH {
        return Err(CodecError::Length);
    }
    o.fill(0);
    put_u32(o, 0, c.sequence);
    put_u16(o, 4, c.desired_pitch);
    o[6] = c.engine_action as u8;
    o[7] = c.separate as u8;
    o[8] = c.abort_safeing as u8;
    o[9] = c.recovery_requested as u8;
    put_u32(o, 12, crc32_ieee(&o[..12]));
    Ok(())
}
pub fn parse_actuator_command(b: &[u8]) -> Result<ActuatorCommand, CodecError> {
    if b.len() != ACTUATOR_COMMAND_LENGTH {
        return Err(CodecError::Length);
    }
    if crc32_ieee(&b[..12]) != get_u32(b, 12) {
        return Err(CodecError::Checksum);
    }
    if b[10] != 0 || b[11] != 0 {
        return Err(CodecError::Reserved);
    }
    if b[7] > 1 || b[8] > 1 || b[9] > 1 {
        return Err(CodecError::Flags);
    }
    Ok(ActuatorCommand {
        sequence: get_u32(b, 0),
        desired_pitch: get_u16(b, 4),
        engine_action: EngineAction::parse(b[6])?,
        separate: b[7] != 0,
        abort_safeing: b[8] != 0,
        recovery_requested: b[9] != 0,
    })
}
pub fn write_flight_output(f: &FlightOutput, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != FLIGHT_OUTPUT_LENGTH {
        return Err(CodecError::Length);
    }
    if f.alarms & !ALARM_MASK != 0 || f.command.sequence != f.sequence {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    put_u32(o, 0, f.sequence);
    put_i32(o, 4, f.nav_time_q16);
    put_i32(o, 8, f.nav_radius_q12);
    put_i32(o, 12, f.nav_downrange_q32);
    put_i32(o, 16, f.nav_radial_velocity_q24);
    put_i32(o, 20, f.nav_tangential_velocity_q24);
    put_u16(o, 24, f.nav_pitch);
    o[26] = f.mode as u8;
    put_u16(o, 28, f.alarms);
    put_u16(o, 30, f.command.desired_pitch);
    o[32] = f.command.engine_action as u8;
    o[33] = f.command.separate as u8;
    o[34] = f.command.abort_safeing as u8;
    o[35] = f.command.recovery_requested as u8;
    put_u32(o, 36, f.nav_checksum);
    put_u32(o, 40, f.flight_checksum);
    put_u32(o, 44, f.command.sequence);
    put_u32(o, 48, crc32_ieee(&o[..48]));
    Ok(())
}
pub fn parse_flight_output(b: &[u8]) -> Result<FlightOutput, CodecError> {
    if b.len() != FLIGHT_OUTPUT_LENGTH {
        return Err(CodecError::Length);
    }
    if crc32_ieee(&b[..48]) != get_u32(b, 48) {
        return Err(CodecError::Checksum);
    }
    if b[27] != 0 || b[33] > 1 || b[34] > 1 || b[35] > 1 {
        return Err(CodecError::Reserved);
    }
    let alarms = get_u16(b, 28);
    if alarms & !ALARM_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let sequence = get_u32(b, 0);
    let command_sequence = get_u32(b, 44);
    if sequence != command_sequence {
        return Err(CodecError::Sequence);
    }
    Ok(FlightOutput {
        sequence,
        nav_time_q16: get_i32(b, 4),
        nav_radius_q12: get_i32(b, 8),
        nav_downrange_q32: get_i32(b, 12),
        nav_radial_velocity_q24: get_i32(b, 16),
        nav_tangential_velocity_q24: get_i32(b, 20),
        nav_pitch: get_u16(b, 24),
        mode: FlightMode::parse(b[26])?,
        alarms,
        command: ActuatorCommand {
            sequence: command_sequence,
            desired_pitch: get_u16(b, 30),
            engine_action: EngineAction::parse(b[32])?,
            separate: b[33] != 0,
            abort_safeing: b[34] != 0,
            recovery_requested: b[35] != 0,
        },
        nav_checksum: get_u32(b, 36),
        flight_checksum: get_u32(b, 40),
    })
}
