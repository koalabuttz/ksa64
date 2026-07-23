//! Additive Phase 5 spatial sensor and actuator transport contracts.
//!
//! Every message is fixed-width, little-endian, allocation-free, and rejects
//! unknown flags, non-zero reserved bytes, invalid enums, and bad CRCs.

use super::{crc32_ieee, CodecError, EngineAction, StagePhase};

pub const SPATIAL_SENSOR_FRAME_LENGTH: usize = 128;
pub const SPATIAL_ACTUATOR_COMMAND_LENGTH: usize = 32;

pub const SENSOR_VALID_IMU: u16 = 1 << 0;
pub const SENSOR_VALID_BAROMETER: u16 = 1 << 1;
pub const SENSOR_VALID_GPS: u16 = 1 << 2;
pub const SENSOR_VALID_STAR_TRACKER: u16 = 1 << 3;
pub const SENSOR_VALID_CLOCK: u16 = 1 << 4;
pub const SENSOR_VALID_ACTUATOR: u16 = 1 << 5;
pub const SENSOR_VALID_MASK: u16 = (1 << 6) - 1;

pub const EVENT_IGNITION: u16 = 1 << 0;
pub const EVENT_CUTOFF: u16 = 1 << 1;
pub const EVENT_SEPARATION: u16 = 1 << 2;
pub const EVENT_IMPACT: u16 = 1 << 3;
pub const EVENT_END: u16 = 1 << 4;
pub const EVENT_GPS_ACQUIRED: u16 = 1 << 5;
pub const EVENT_GPS_LOST: u16 = 1 << 6;
pub const EVENT_ABORT: u16 = 1 << 7;
pub const EVENT_RCS_DEPLETED: u16 = 1 << 8;
pub const EVENT_GIMBAL_JAMMED: u16 = 1 << 9;
pub const EVENT_STAR_ACQUIRED: u16 = 1 << 10;
pub const EVENT_STAR_LOST: u16 = 1 << 11;
pub const EVENT_MASK: u16 = (1 << 12) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialSensorFrame {
    pub sequence: u32,
    pub onboard_time_q16: i32,
    pub validity: u16,
    pub events: u16,
    pub accel_body_q28: [i32; 3],
    pub gyro_body_q24: [i32; 3],
    pub baro_altitude_q12: i32,
    pub gps_position_q12: [i32; 3],
    pub gps_velocity_q24: [i32; 3],
    pub star_attitude_q30: [i32; 4],
    pub gimbal_applied_q16: [i32; 2],
    pub rcs_propellant_q12: i32,
    pub active_stage: u8,
    pub stage_phase: StagePhase,
    pub engine_on: bool,
}

impl SpatialSensorFrame {
    pub const ZERO: Self = Self {
        sequence: 0,
        onboard_time_q16: 0,
        validity: 0,
        events: 0,
        accel_body_q28: [0; 3],
        gyro_body_q24: [0; 3],
        baro_altitude_q12: 0,
        gps_position_q12: [0; 3],
        gps_velocity_q24: [0; 3],
        star_attitude_q30: [1 << 30, 0, 0, 0],
        gimbal_applied_q16: [0; 2],
        rcs_propellant_q12: 0,
        active_stage: 0,
        stage_phase: StagePhase::CoastBeforeIgnition,
        engine_on: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialActuatorCommand {
    pub sequence: u32,
    pub gimbal_q16: [i32; 2],
    pub rcs_q15: [i32; 3],
    pub engine_action: EngineAction,
    pub separate: bool,
    pub abort_safeing: bool,
}

impl SpatialActuatorCommand {
    pub const SAFE: Self = Self {
        sequence: 0,
        gimbal_q16: [0; 2],
        rcs_q15: [0; 3],
        engine_action: EngineAction::Cutoff,
        separate: false,
        abort_safeing: true,
    };
}

fn put_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_i32(out: &mut [u8], at: usize, value: i32) {
    put_u32(out, at, value as u32);
}
fn get_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn get_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
fn get_i32(bytes: &[u8], at: usize) -> i32 {
    get_u32(bytes, at) as i32
}
fn parse_bool(value: u8) -> Result<bool, CodecError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CodecError::Enum),
    }
}
fn parse_stage(value: u8) -> Result<StagePhase, CodecError> {
    match value {
        0 => Ok(StagePhase::CoastBeforeIgnition),
        1 => Ok(StagePhase::Burning),
        2 => Ok(StagePhase::CoastBeforeSeparation),
        3 => Ok(StagePhase::Complete),
        _ => Err(CodecError::Enum),
    }
}
fn parse_engine(value: u8) -> Result<EngineAction, CodecError> {
    match value {
        0 => Ok(EngineAction::Hold),
        1 => Ok(EngineAction::Ignite),
        2 => Ok(EngineAction::Cutoff),
        _ => Err(CodecError::Enum),
    }
}

pub fn write_spatial_sensor_frame(
    frame: &SpatialSensorFrame,
    out: &mut [u8],
) -> Result<(), CodecError> {
    if out.len() != SPATIAL_SENSOR_FRAME_LENGTH {
        return Err(CodecError::Length);
    }
    if frame.validity & !SENSOR_VALID_MASK != 0 || frame.events & !EVENT_MASK != 0 {
        return Err(CodecError::Flags);
    }
    out.fill(0);
    put_u32(out, 0, frame.sequence);
    put_i32(out, 4, frame.onboard_time_q16);
    put_u16(out, 8, frame.validity);
    put_u16(out, 10, frame.events);
    let mut i = 0;
    while i < 3 {
        put_i32(out, 12 + i * 4, frame.accel_body_q28[i]);
        put_i32(out, 24 + i * 4, frame.gyro_body_q24[i]);
        put_i32(out, 40 + i * 4, frame.gps_position_q12[i]);
        put_i32(out, 52 + i * 4, frame.gps_velocity_q24[i]);
        i += 1;
    }
    put_i32(out, 36, frame.baro_altitude_q12);
    i = 0;
    while i < 4 {
        put_i32(out, 64 + i * 4, frame.star_attitude_q30[i]);
        i += 1;
    }
    put_i32(out, 80, frame.gimbal_applied_q16[0]);
    put_i32(out, 84, frame.gimbal_applied_q16[1]);
    put_i32(out, 88, frame.rcs_propellant_q12);
    out[92] = frame.active_stage;
    out[93] = frame.stage_phase as u8;
    out[94] = frame.engine_on as u8;
    let crc = crc32_ieee(&out[..124]);
    put_u32(out, 124, crc);
    Ok(())
}

pub fn parse_spatial_sensor_frame(bytes: &[u8]) -> Result<SpatialSensorFrame, CodecError> {
    if bytes.len() != SPATIAL_SENSOR_FRAME_LENGTH {
        return Err(CodecError::Length);
    }
    // Keep the CRC passes in named temporaries. The pinned rust-mos optimizer
    // otherwise aliases the tail load with the long slice reduction.
    let calculated_crc = crc32_ieee(&bytes[..124]);
    let stored_crc = get_u32(bytes, 124);
    if stored_crc != calculated_crc {
        return Err(CodecError::Checksum);
    }
    if bytes[95..124].iter().any(|&byte| byte != 0) {
        return Err(CodecError::Reserved);
    }
    let validity = get_u16(bytes, 8);
    let events = get_u16(bytes, 10);
    if validity & !SENSOR_VALID_MASK != 0 || events & !EVENT_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let mut accel = [0; 3];
    let mut gyro = [0; 3];
    let mut gps_position = [0; 3];
    let mut gps_velocity = [0; 3];
    let mut i = 0;
    while i < 3 {
        accel[i] = get_i32(bytes, 12 + i * 4);
        gyro[i] = get_i32(bytes, 24 + i * 4);
        gps_position[i] = get_i32(bytes, 40 + i * 4);
        gps_velocity[i] = get_i32(bytes, 52 + i * 4);
        i += 1;
    }
    let mut star = [0; 4];
    i = 0;
    while i < 4 {
        star[i] = get_i32(bytes, 64 + i * 4);
        i += 1;
    }
    Ok(SpatialSensorFrame {
        sequence: get_u32(bytes, 0),
        onboard_time_q16: get_i32(bytes, 4),
        validity,
        events,
        accel_body_q28: accel,
        gyro_body_q24: gyro,
        baro_altitude_q12: get_i32(bytes, 36),
        gps_position_q12: gps_position,
        gps_velocity_q24: gps_velocity,
        star_attitude_q30: star,
        gimbal_applied_q16: [get_i32(bytes, 80), get_i32(bytes, 84)],
        rcs_propellant_q12: get_i32(bytes, 88),
        active_stage: bytes[92],
        stage_phase: parse_stage(bytes[93])?,
        engine_on: parse_bool(bytes[94])?,
    })
}

pub fn write_spatial_actuator_command(
    command: &SpatialActuatorCommand,
    out: &mut [u8],
) -> Result<(), CodecError> {
    if out.len() != SPATIAL_ACTUATOR_COMMAND_LENGTH {
        return Err(CodecError::Length);
    }
    out.fill(0);
    put_u32(out, 0, command.sequence);
    put_i32(out, 4, command.gimbal_q16[0]);
    put_i32(out, 8, command.gimbal_q16[1]);
    let mut i = 0;
    while i < 3 {
        put_i32(out, 12 + i * 4, command.rcs_q15[i]);
        i += 1;
    }
    out[24] = command.engine_action as u8;
    out[25] = command.separate as u8;
    out[26] = command.abort_safeing as u8;
    put_u32(out, 28, crc32_ieee(&out[..28]));
    Ok(())
}

pub fn parse_spatial_actuator_command(bytes: &[u8]) -> Result<SpatialActuatorCommand, CodecError> {
    if bytes.len() != SPATIAL_ACTUATOR_COMMAND_LENGTH {
        return Err(CodecError::Length);
    }
    let calculated_crc = crc32_ieee(&bytes[..28]);
    let stored_crc = get_u32(bytes, 28);
    if stored_crc != calculated_crc {
        return Err(CodecError::Checksum);
    }
    if bytes[27] != 0 {
        return Err(CodecError::Reserved);
    }
    Ok(SpatialActuatorCommand {
        sequence: get_u32(bytes, 0),
        gimbal_q16: [get_i32(bytes, 4), get_i32(bytes, 8)],
        rcs_q15: [get_i32(bytes, 12), get_i32(bytes, 16), get_i32(bytes, 20)],
        engine_action: parse_engine(bytes[24])?,
        separate: parse_bool(bytes[25])?,
        abort_safeing: parse_bool(bytes[26])?,
    })
}
