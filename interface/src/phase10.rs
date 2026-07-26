//! Phase 10 global-flight raw cells. KLF6 remains the outer transport.

#![allow(clippy::needless_range_loop)]

use crate::phase6::crc16_ccitt;
use crate::CodecError;

pub const KLR10_CONTRACT_ID: u32 = 0x1052_0001;
pub const KLR10_SYNC: [u8; 2] = [0xda, 0x5a];
pub const GLOBAL_FAST_SENSOR_LENGTH: usize = 64;
pub const GLOBAL_AID_FRAME_LENGTH: usize = 96;
pub const GLOBAL_TRANSITION_LENGTH: usize = 192;
pub const GLOBAL_COMMAND_LENGTH: usize = 64;
pub const GLOBAL_STATUS_LENGTH: usize = 96;

pub const GLOBAL_FAST_DELTA_V: u8 = 1;
pub const GLOBAL_FAST_DELTA_ANGLE: u8 = 2;
pub const GLOBAL_FAST_ATTITUDE: u8 = 4;
pub const GLOBAL_FAST_AIR_DATA: u8 = 8;
pub const GLOBAL_FAST_ACTUATOR: u8 = 16;
pub const GLOBAL_FAST_SUPPLY: u8 = 32;
pub const GLOBAL_FAST_VALID_MASK: u8 = 63;

pub const GLOBAL_AID_BAROMETER: u8 = 1;
pub const GLOBAL_AID_GNSS: u8 = 2;
pub const GLOBAL_AID_ATTITUDE: u8 = 4;
pub const GLOBAL_AID_FRAME_SERVICE: u8 = 8;
pub const GLOBAL_AID_CONTINUITY: u8 = 16;
pub const GLOBAL_AID_DEPLOYMENT_FEEDBACK: u8 = 32;
pub const GLOBAL_AID_VALID_MASK: u8 = 63;

pub const GLOBAL_COMMAND_DROGUE: u8 = 1;
pub const GLOBAL_COMMAND_MAIN: u8 = 2;
pub const GLOBAL_COMMAND_SAFE: u8 = 4;
pub const GLOBAL_COMMAND_DISCRETE_MASK: u8 = 7;
pub const GLOBAL_COMMAND_HOLD: u8 = 1;
pub const GLOBAL_COMMAND_FRAME_PENDING: u8 = 2;
pub const GLOBAL_COMMAND_RCS_RESERVED: u8 = 4;
pub const GLOBAL_COMMAND_FLAG_MASK: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalFrameId {
    LocalEnuV1 = 1,
    EarthFixedEcefV1 = 2,
    EarthInertialEciV1 = 3,
}

impl GlobalFrameId {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::LocalEnuV1),
            2 => Ok(Self::EarthFixedEcefV1),
            3 => Ok(Self::EarthInertialEciV1),
            _ => Err(CodecError::Enum),
        }
    }
}

fn p16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes())
}
fn pi16(output: &mut [u8], offset: usize, value: i16) {
    p16(output, offset, value as u16)
}
fn p32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
}
fn pi32(output: &mut [u8], offset: usize, value: i32) {
    p32(output, offset, value as u32)
}
fn g16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn gi16(bytes: &[u8], offset: usize) -> i16 {
    g16(bytes, offset) as i16
}
fn g32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
fn gi32(bytes: &[u8], offset: usize) -> i32 {
    g32(bytes, offset) as i32
}
fn prefix(output: &mut [u8], kind: u8, session: u16, first: u16, second: u16) {
    output[..2].copy_from_slice(&KLR10_SYNC);
    output[2] = 10;
    output[3] = kind;
    p16(output, 4, session);
    p16(output, 6, first);
    p16(output, 8, second);
}
fn check(bytes: &[u8], length: usize, kind: u8) -> Result<(), CodecError> {
    if bytes.len() != length {
        return Err(CodecError::Length);
    }
    if bytes[..2] != KLR10_SYNC || bytes[2] != 10 || bytes[3] != kind {
        return Err(CodecError::Enum);
    }
    if crc16_ccitt(&bytes[..length - 2]) != g16(bytes, length - 2) {
        return Err(CodecError::Checksum);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalFastSensorCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub frame: GlobalFrameId,
    pub validity: u8,
    pub mission_time_q16: u32,
    pub delta_velocity_q24: [i16; 3],
    pub delta_angle_q24: [i16; 3],
    pub attitude_vector_q15: [i16; 3],
    pub angular_rate_q15: [i16; 3],
    pub dynamic_pressure_q10: i32,
    pub mach_q12: i16,
    pub gimbal_applied_q15: [i16; 2],
    pub rcs_propellant_q21: i32,
    pub actuator_feedback: u16,
    pub vehicle_status: u16,
    pub sensor_checksum: u16,
}

pub fn write_global_fast_sensor(
    value: &GlobalFastSensorCell,
    output: &mut [u8],
) -> Result<(), CodecError> {
    if output.len() != GLOBAL_FAST_SENSOR_LENGTH || value.validity & !GLOBAL_FAST_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    prefix(
        output,
        1,
        value.session,
        value.measurement_epoch,
        value.production_epoch,
    );
    output[10] = value.frame as u8;
    output[11] = value.validity;
    p32(output, 12, value.mission_time_q16);
    for axis in 0..3 {
        pi16(output, 16 + axis * 2, value.delta_velocity_q24[axis]);
        pi16(output, 22 + axis * 2, value.delta_angle_q24[axis]);
        pi16(output, 28 + axis * 2, value.attitude_vector_q15[axis]);
        pi16(output, 34 + axis * 2, value.angular_rate_q15[axis]);
    }
    pi32(output, 40, value.dynamic_pressure_q10);
    pi16(output, 44, value.mach_q12);
    pi16(output, 46, value.gimbal_applied_q15[0]);
    pi16(output, 48, value.gimbal_applied_q15[1]);
    pi32(output, 50, value.rcs_propellant_q21);
    p16(output, 54, value.actuator_feedback);
    p16(output, 56, value.vehicle_status);
    p16(output, 58, value.sensor_checksum);
    p16(output, 62, crc16_ccitt(&output[..62]));
    Ok(())
}

pub fn parse_global_fast_sensor(bytes: &[u8]) -> Result<GlobalFastSensorCell, CodecError> {
    check(bytes, GLOBAL_FAST_SENSOR_LENGTH, 1)?;
    if bytes[11] & !GLOBAL_FAST_VALID_MASK != 0 || bytes[60] != 0 || bytes[61] != 0 {
        return Err(CodecError::Reserved);
    }
    let mut delta_velocity_q24 = [0; 3];
    let mut delta_angle_q24 = [0; 3];
    let mut attitude_vector_q15 = [0; 3];
    let mut angular_rate_q15 = [0; 3];
    for axis in 0..3 {
        delta_velocity_q24[axis] = gi16(bytes, 16 + axis * 2);
        delta_angle_q24[axis] = gi16(bytes, 22 + axis * 2);
        attitude_vector_q15[axis] = gi16(bytes, 28 + axis * 2);
        angular_rate_q15[axis] = gi16(bytes, 34 + axis * 2);
    }
    Ok(GlobalFastSensorCell {
        session: g16(bytes, 4),
        measurement_epoch: g16(bytes, 6),
        production_epoch: g16(bytes, 8),
        frame: GlobalFrameId::parse(bytes[10])?,
        validity: bytes[11],
        mission_time_q16: g32(bytes, 12),
        delta_velocity_q24,
        delta_angle_q24,
        attitude_vector_q15,
        angular_rate_q15,
        dynamic_pressure_q10: gi32(bytes, 40),
        mach_q12: gi16(bytes, 44),
        gimbal_applied_q15: [gi16(bytes, 46), gi16(bytes, 48)],
        rcs_propellant_q21: gi32(bytes, 50),
        actuator_feedback: g16(bytes, 54),
        vehicle_status: g16(bytes, 56),
        sensor_checksum: g16(bytes, 58),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalAidFrameCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub frame: GlobalFrameId,
    pub validity: u8,
    pub mission_time_q16: u32,
    pub barometer_q12_km: i32,
    pub gnss_position_q12_km: [i32; 3],
    pub gnss_velocity_q24_km_s: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub frame_rotation_q30: [i32; 4],
    pub frame_omega_q24: [i32; 3],
    pub events: u16,
    pub continuity: u16,
    pub deployment_feedback: u16,
}

pub fn write_global_aid_frame(
    value: &GlobalAidFrameCell,
    output: &mut [u8],
) -> Result<(), CodecError> {
    if output.len() != GLOBAL_AID_FRAME_LENGTH || value.validity & !GLOBAL_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    prefix(
        output,
        2,
        value.session,
        value.measurement_epoch,
        value.production_epoch,
    );
    output[10] = value.frame as u8;
    output[11] = value.validity;
    p32(output, 12, value.mission_time_q16);
    pi32(output, 16, value.barometer_q12_km);
    for axis in 0..3 {
        pi32(output, 20 + axis * 4, value.gnss_position_q12_km[axis]);
        pi32(output, 32 + axis * 4, value.gnss_velocity_q24_km_s[axis]);
        pi32(output, 76 + axis * 4, value.frame_omega_q24[axis]);
    }
    for component in 0..4 {
        pi32(output, 44 + component * 4, value.attitude_q30[component]);
        pi32(
            output,
            60 + component * 4,
            value.frame_rotation_q30[component],
        );
    }
    p16(output, 88, value.events);
    p16(output, 90, value.continuity);
    p16(output, 92, value.deployment_feedback);
    p16(output, 94, crc16_ccitt(&output[..94]));
    Ok(())
}

pub fn parse_global_aid_frame(bytes: &[u8]) -> Result<GlobalAidFrameCell, CodecError> {
    check(bytes, GLOBAL_AID_FRAME_LENGTH, 2)?;
    if bytes[11] & !GLOBAL_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let mut position = [0; 3];
    let mut velocity = [0; 3];
    let mut attitude = [0; 4];
    let mut rotation = [0; 4];
    let mut omega = [0; 3];
    for axis in 0..3 {
        position[axis] = gi32(bytes, 20 + axis * 4);
        velocity[axis] = gi32(bytes, 32 + axis * 4);
        omega[axis] = gi32(bytes, 76 + axis * 4);
    }
    for component in 0..4 {
        attitude[component] = gi32(bytes, 44 + component * 4);
        rotation[component] = gi32(bytes, 60 + component * 4);
    }
    Ok(GlobalAidFrameCell {
        session: g16(bytes, 4),
        measurement_epoch: g16(bytes, 6),
        production_epoch: g16(bytes, 8),
        frame: GlobalFrameId::parse(bytes[10])?,
        validity: bytes[11],
        mission_time_q16: g32(bytes, 12),
        barometer_q12_km: gi32(bytes, 16),
        gnss_position_q12_km: position,
        gnss_velocity_q24_km_s: velocity,
        attitude_q30: attitude,
        frame_rotation_q30: rotation,
        frame_omega_q24: omega,
        events: g16(bytes, 88),
        continuity: g16(bytes, 90),
        deployment_feedback: g16(bytes, 92),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalTransitionCell {
    pub session: u16,
    pub source_epoch: u16,
    pub effective_epoch: u16,
    pub from: GlobalFrameId,
    pub to: GlobalFrameId,
    pub flags: u16,
    pub mission_time_q16: u32,
    pub transform_identity: u32,
    pub rotation_q30: [i32; 4],
    pub omega_q24: [i32; 3],
    pub pre_position_q12: [i32; 3],
    pub post_position_q12: [i32; 3],
    pub pre_velocity_q24: [i32; 3],
    pub post_velocity_q24: [i32; 3],
    pub pre_attitude_q30: [i32; 4],
    pub post_attitude_q30: [i32; 4],
    pub pre_rate_q24: [i32; 3],
    pub post_rate_q24: [i32; 3],
    pub translation_q12: [i32; 3],
    pub velocity_bias_q24: [i32; 3],
    pub transition_checksum: u32,
}

pub fn write_global_transition(
    value: &GlobalTransitionCell,
    output: &mut [u8],
) -> Result<(), CodecError> {
    if output.len() != GLOBAL_TRANSITION_LENGTH || value.flags != 0 {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    prefix(
        output,
        3,
        value.session,
        value.source_epoch,
        value.effective_epoch,
    );
    output[10] = value.from as u8;
    output[11] = value.to as u8;
    p16(output, 12, value.flags);
    p32(output, 16, value.mission_time_q16);
    p32(output, 20, value.transform_identity);
    for component in 0..4 {
        pi32(output, 24 + component * 4, value.rotation_q30[component]);
        pi32(
            output,
            112 + component * 4,
            value.pre_attitude_q30[component],
        );
        pi32(
            output,
            128 + component * 4,
            value.post_attitude_q30[component],
        );
    }
    for axis in 0..3 {
        pi32(output, 40 + axis * 4, value.omega_q24[axis]);
        pi32(output, 52 + axis * 4, value.pre_position_q12[axis]);
        pi32(output, 64 + axis * 4, value.post_position_q12[axis]);
        pi32(output, 76 + axis * 4, value.pre_velocity_q24[axis]);
        pi32(output, 88 + axis * 4, value.post_velocity_q24[axis]);
        pi32(output, 100 + axis * 4, value.pre_rate_q24[axis]);
        pi32(output, 144 + axis * 4, value.post_rate_q24[axis]);
        pi32(output, 156 + axis * 4, value.translation_q12[axis]);
        pi32(output, 168 + axis * 4, value.velocity_bias_q24[axis]);
    }
    p32(output, 180, value.transition_checksum);
    p16(output, 190, crc16_ccitt(&output[..190]));
    Ok(())
}

pub fn parse_global_transition(bytes: &[u8]) -> Result<GlobalTransitionCell, CodecError> {
    check(bytes, GLOBAL_TRANSITION_LENGTH, 3)?;
    if bytes[14] != 0
        || bytes[15] != 0
        || g16(bytes, 12) != 0
        || bytes[184..190].iter().any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut rotation = [0; 4];
    let mut pre_attitude = [0; 4];
    let mut post_attitude = [0; 4];
    let mut omega = [0; 3];
    let mut pre_position = [0; 3];
    let mut post_position = [0; 3];
    let mut pre_velocity = [0; 3];
    let mut post_velocity = [0; 3];
    let mut pre_rate = [0; 3];
    let mut post_rate = [0; 3];
    let mut translation = [0; 3];
    let mut velocity_bias = [0; 3];
    for component in 0..4 {
        rotation[component] = gi32(bytes, 24 + component * 4);
        pre_attitude[component] = gi32(bytes, 112 + component * 4);
        post_attitude[component] = gi32(bytes, 128 + component * 4);
    }
    for axis in 0..3 {
        omega[axis] = gi32(bytes, 40 + axis * 4);
        pre_position[axis] = gi32(bytes, 52 + axis * 4);
        post_position[axis] = gi32(bytes, 64 + axis * 4);
        pre_velocity[axis] = gi32(bytes, 76 + axis * 4);
        post_velocity[axis] = gi32(bytes, 88 + axis * 4);
        pre_rate[axis] = gi32(bytes, 100 + axis * 4);
        post_rate[axis] = gi32(bytes, 144 + axis * 4);
        translation[axis] = gi32(bytes, 156 + axis * 4);
        velocity_bias[axis] = gi32(bytes, 168 + axis * 4);
    }
    Ok(GlobalTransitionCell {
        session: g16(bytes, 4),
        source_epoch: g16(bytes, 6),
        effective_epoch: g16(bytes, 8),
        from: GlobalFrameId::parse(bytes[10])?,
        to: GlobalFrameId::parse(bytes[11])?,
        flags: 0,
        mission_time_q16: g32(bytes, 16),
        transform_identity: g32(bytes, 20),
        rotation_q30: rotation,
        omega_q24: omega,
        pre_position_q12: pre_position,
        post_position_q12: post_position,
        pre_velocity_q24: pre_velocity,
        post_velocity_q24: post_velocity,
        pre_attitude_q30: pre_attitude,
        post_attitude_q30: post_attitude,
        pre_rate_q24: pre_rate,
        post_rate_q24: post_rate,
        translation_q12: translation,
        velocity_bias_q24: velocity_bias,
        transition_checksum: g32(bytes, 180),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalCommandCell {
    pub session: u16,
    pub source_epoch: u16,
    pub effective_epoch: u16,
    pub frame: GlobalFrameId,
    pub flags: u8,
    pub discrete: u8,
    pub gimbal_q15: [i16; 2],
    pub rcs_pulse_quanta: [u8; 12],
    pub torque_demand_q12: [i32; 3],
    pub status: u16,
    pub command_checksum: u32,
}

pub fn write_global_command(
    value: &GlobalCommandCell,
    output: &mut [u8],
) -> Result<(), CodecError> {
    if output.len() != GLOBAL_COMMAND_LENGTH
        || value.flags & !GLOBAL_COMMAND_FLAG_MASK != 0
        || value.discrete & !GLOBAL_COMMAND_DISCRETE_MASK != 0
        || value.rcs_pulse_quanta.iter().any(|quantum| *quantum > 8)
    {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    prefix(
        output,
        4,
        value.session,
        value.source_epoch,
        value.effective_epoch,
    );
    output[10] = value.frame as u8;
    output[11] = value.flags;
    output[12] = value.discrete;
    pi16(output, 14, value.gimbal_q15[0]);
    pi16(output, 16, value.gimbal_q15[1]);
    output[18..30].copy_from_slice(&value.rcs_pulse_quanta);
    for axis in 0..3 {
        pi32(output, 30 + axis * 4, value.torque_demand_q12[axis]);
    }
    p16(output, 42, value.status);
    p32(output, 44, value.command_checksum);
    p16(output, 62, crc16_ccitt(&output[..62]));
    Ok(())
}

pub fn parse_global_command(bytes: &[u8]) -> Result<GlobalCommandCell, CodecError> {
    check(bytes, GLOBAL_COMMAND_LENGTH, 4)?;
    if bytes[11] & !GLOBAL_COMMAND_FLAG_MASK != 0
        || bytes[12] & !GLOBAL_COMMAND_DISCRETE_MASK != 0
        || bytes[13] != 0
        || bytes[18..30].iter().any(|quantum| *quantum > 8)
        || bytes[48..62].iter().any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut pulses = [0; 12];
    let mut torque = [0; 3];
    pulses.copy_from_slice(&bytes[18..30]);
    for axis in 0..3 {
        torque[axis] = gi32(bytes, 30 + axis * 4);
    }
    Ok(GlobalCommandCell {
        session: g16(bytes, 4),
        source_epoch: g16(bytes, 6),
        effective_epoch: g16(bytes, 8),
        frame: GlobalFrameId::parse(bytes[10])?,
        flags: bytes[11],
        discrete: bytes[12],
        gimbal_q15: [gi16(bytes, 14), gi16(bytes, 16)],
        rcs_pulse_quanta: pulses,
        torque_demand_q12: torque,
        status: g16(bytes, 42),
        command_checksum: g32(bytes, 44),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalStatusCell {
    pub session: u16,
    pub source_epoch: u16,
    pub production_epoch: u16,
    pub frame: GlobalFrameId,
    pub mode: u8,
    pub flags: u16,
    pub alarms: u16,
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub navigation_attitude_q30: [i32; 4],
    pub covariance_proxy_q16: [i32; 3],
    pub sensor_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub flight_checksum: u32,
    pub deadline_misses: u16,
    pub transition_count: u8,
}

pub fn write_global_status(value: &GlobalStatusCell, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != GLOBAL_STATUS_LENGTH {
        return Err(CodecError::Length);
    }
    output.fill(0);
    prefix(
        output,
        5,
        value.session,
        value.source_epoch,
        value.production_epoch,
    );
    output[10] = value.frame as u8;
    output[11] = value.mode;
    p16(output, 12, value.flags);
    p16(output, 14, value.alarms);
    for axis in 0..3 {
        pi32(output, 16 + axis * 4, value.navigation_position_q12[axis]);
        pi32(output, 28 + axis * 4, value.navigation_velocity_q24[axis]);
        pi32(output, 56 + axis * 4, value.covariance_proxy_q16[axis]);
    }
    for component in 0..4 {
        pi32(
            output,
            40 + component * 4,
            value.navigation_attitude_q30[component],
        );
    }
    p32(output, 68, value.sensor_checksum);
    p32(output, 72, value.navigation_checksum);
    p32(output, 76, value.command_checksum);
    p32(output, 80, value.flight_checksum);
    p16(output, 84, value.deadline_misses);
    output[86] = value.transition_count;
    p16(output, 94, crc16_ccitt(&output[..94]));
    Ok(())
}

pub fn parse_global_status(bytes: &[u8]) -> Result<GlobalStatusCell, CodecError> {
    check(bytes, GLOBAL_STATUS_LENGTH, 5)?;
    if bytes[87..94].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let mut position = [0; 3];
    let mut velocity = [0; 3];
    let mut attitude = [0; 4];
    let mut covariance = [0; 3];
    for axis in 0..3 {
        position[axis] = gi32(bytes, 16 + axis * 4);
        velocity[axis] = gi32(bytes, 28 + axis * 4);
        covariance[axis] = gi32(bytes, 56 + axis * 4);
    }
    for component in 0..4 {
        attitude[component] = gi32(bytes, 40 + component * 4);
    }
    Ok(GlobalStatusCell {
        session: g16(bytes, 4),
        source_epoch: g16(bytes, 6),
        production_epoch: g16(bytes, 8),
        frame: GlobalFrameId::parse(bytes[10])?,
        mode: bytes[11],
        flags: g16(bytes, 12),
        alarms: g16(bytes, 14),
        navigation_position_q12: position,
        navigation_velocity_q24: velocity,
        navigation_attitude_q30: attitude,
        covariance_proxy_q16: covariance,
        sensor_checksum: g32(bytes, 68),
        navigation_checksum: g32(bytes, 72),
        command_checksum: g32(bytes, 76),
        flight_checksum: g32(bytes, 80),
        deadline_misses: g16(bytes, 84),
        transition_count: bytes[86],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cell_round_trips_and_cross_contract_fails() {
        let fast = GlobalFastSensorCell {
            session: 10,
            measurement_epoch: 11,
            production_epoch: 11,
            frame: GlobalFrameId::EarthFixedEcefV1,
            validity: GLOBAL_FAST_VALID_MASK,
            mission_time_q16: 22_528,
            delta_velocity_q24: [1, -2, 3],
            delta_angle_q24: [4, -5, 6],
            attitude_vector_q15: [7, -8, 9],
            angular_rate_q15: [10, -11, 12],
            dynamic_pressure_q10: 13,
            mach_q12: 14,
            gimbal_applied_q15: [15, -16],
            rcs_propellant_q21: 17,
            actuator_feedback: 18,
            vehicle_status: 19,
            sensor_checksum: 20,
        };
        let mut fast_bytes = [0; GLOBAL_FAST_SENSOR_LENGTH];
        write_global_fast_sensor(&fast, &mut fast_bytes).unwrap();
        assert_eq!(parse_global_fast_sensor(&fast_bytes), Ok(fast));

        let aid = GlobalAidFrameCell {
            session: 10,
            measurement_epoch: 8,
            production_epoch: 9,
            frame: GlobalFrameId::EarthInertialEciV1,
            validity: GLOBAL_AID_VALID_MASK,
            mission_time_q16: 12,
            barometer_q12_km: 13,
            gnss_position_q12_km: [14, 15, 16],
            gnss_velocity_q24_km_s: [17, 18, 19],
            attitude_q30: [1 << 30, 0, 0, 0],
            frame_rotation_q30: [1 << 30, 0, 0, 0],
            frame_omega_q24: [20, 21, 22],
            events: 23,
            continuity: 3,
            deployment_feedback: 1,
        };
        let mut aid_bytes = [0; GLOBAL_AID_FRAME_LENGTH];
        write_global_aid_frame(&aid, &mut aid_bytes).unwrap();
        assert_eq!(parse_global_aid_frame(&aid_bytes), Ok(aid));

        let transition = GlobalTransitionCell {
            session: 10,
            source_epoch: 12,
            effective_epoch: 13,
            from: GlobalFrameId::EarthFixedEcefV1,
            to: GlobalFrameId::EarthInertialEciV1,
            flags: 0,
            mission_time_q16: 14,
            transform_identity: 15,
            rotation_q30: [1 << 30, 0, 0, 0],
            omega_q24: [1, 2, 3],
            pre_position_q12: [4, 5, 6],
            post_position_q12: [7, 8, 9],
            pre_velocity_q24: [10, 11, 12],
            post_velocity_q24: [13, 14, 15],
            pre_attitude_q30: [1 << 30, 0, 0, 0],
            post_attitude_q30: [1 << 30, 0, 0, 0],
            pre_rate_q24: [16, 17, 18],
            post_rate_q24: [19, 20, 21],
            translation_q12: [22, 23, 24],
            velocity_bias_q24: [25, 26, 27],
            transition_checksum: 28,
        };
        let mut transition_bytes = [0; GLOBAL_TRANSITION_LENGTH];
        write_global_transition(&transition, &mut transition_bytes).unwrap();
        assert_eq!(parse_global_transition(&transition_bytes), Ok(transition));

        let command = GlobalCommandCell {
            session: 10,
            source_epoch: 11,
            effective_epoch: 12,
            frame: GlobalFrameId::LocalEnuV1,
            flags: GLOBAL_COMMAND_HOLD,
            discrete: GLOBAL_COMMAND_DROGUE,
            gimbal_q15: [1, -1],
            rcs_pulse_quanta: [1; 12],
            torque_demand_q12: [2, 3, 4],
            status: 5,
            command_checksum: 6,
        };
        let mut command_bytes = [0; GLOBAL_COMMAND_LENGTH];
        write_global_command(&command, &mut command_bytes).unwrap();
        assert_eq!(parse_global_command(&command_bytes), Ok(command));

        let status = GlobalStatusCell {
            session: 10,
            source_epoch: 11,
            production_epoch: 11,
            frame: GlobalFrameId::EarthFixedEcefV1,
            mode: 2,
            flags: 3,
            alarms: 4,
            navigation_position_q12: [5, 6, 7],
            navigation_velocity_q24: [8, 9, 10],
            navigation_attitude_q30: [1 << 30, 0, 0, 0],
            covariance_proxy_q16: [11, 12, 13],
            sensor_checksum: 14,
            navigation_checksum: 15,
            command_checksum: 16,
            flight_checksum: 17,
            deadline_misses: 18,
            transition_count: 3,
        };
        let mut status_bytes = [0; GLOBAL_STATUS_LENGTH];
        write_global_status(&status, &mut status_bytes).unwrap();
        assert_eq!(parse_global_status(&status_bytes), Ok(status));

        fast_bytes[2] = 9;
        assert!(parse_global_fast_sensor(&fast_bytes).is_err());
        status_bytes[93] = 1;
        let crc = crc16_ccitt(&status_bytes[..94]);
        p16(&mut status_bytes, 94, crc);
        assert_eq!(
            parse_global_status(&status_bytes),
            Err(CodecError::Reserved)
        );
    }
}
