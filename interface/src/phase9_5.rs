//! Phase 9.5 advanced-effector raw cells. KLF6 remains the outer transport.

// Explicit indices mirror frozen wire offsets and keep MOS code generation auditable.
#![allow(clippy::needless_range_loop)]

use crate::phase6::crc16_ccitt;
use crate::CodecError;

pub const KLR9_CONTRACT_ID: u32 = 0x0952_0001;
pub const KLR9_SYNC: [u8; 2] = [0xd9, 0x5a];
pub const ADVANCED_FAST_SENSOR_LENGTH: usize = 64;
pub const ADVANCED_COMMAND_LENGTH: usize = 64;
pub const ADVANCED_AID_LENGTH: usize = 64;
pub const ADVANCED_STATUS_LENGTH: usize = 80;
pub const ADVANCED_VALID_PLATFORM: u16 = 1;
pub const ADVANCED_VALID_RATE: u16 = 2;
pub const ADVANCED_VALID_DELTA_V: u16 = 4;
pub const ADVANCED_VALID_ACTUATOR: u16 = 8;
pub const ADVANCED_VALID_AIR_DATA: u16 = 16;
pub const ADVANCED_VALID_SUPPLY: u16 = 32;
pub const ADVANCED_VALID_MASK: u16 = 63;
pub const ADVANCED_AID_BAROMETER: u16 = 1;
pub const ADVANCED_AID_GPS: u16 = 2;
pub const ADVANCED_AID_ATTITUDE: u16 = 4;
pub const ADVANCED_AID_CONTINUITY: u16 = 8;
pub const ADVANCED_AID_DEPLOYMENT_FEEDBACK: u16 = 16;
pub const ADVANCED_AID_VALID_MASK: u16 = 31;
pub const ADVANCED_COMMAND_DROGUE: u8 = 1;
pub const ADVANCED_COMMAND_MAIN: u8 = 2;
pub const ADVANCED_COMMAND_SAFE: u8 = 4;
pub const ADVANCED_COMMAND_DISCRETE_MASK: u8 = 7;
pub const ADVANCED_COMMAND_FLAG_HOLD: u8 = 1;
pub const ADVANCED_COMMAND_FLAG_AIRDATA_FALLBACK: u8 = 2;
pub const ADVANCED_COMMAND_FLAG_RCS_RESERVED: u8 = 4;
pub const ADVANCED_COMMAND_FLAG_MASK: u8 = 7;

fn p16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
}
fn pi16(o: &mut [u8], i: usize, v: i16) {
    p16(o, i, v as u16)
}
fn p32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes())
}
fn pi32(o: &mut [u8], i: usize, v: i32) {
    p32(o, i, v as u32)
}
fn g16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn gi16(b: &[u8], i: usize) -> i16 {
    g16(b, i) as i16
}
fn g32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn gi32(b: &[u8], i: usize) -> i32 {
    g32(b, i) as i32
}
fn prefix(o: &mut [u8], kind: u8, session: u16, a: u16, b: u16) {
    o[..2].copy_from_slice(&KLR9_SYNC);
    o[2] = 9;
    o[3] = kind;
    p16(o, 4, session);
    p16(o, 6, a);
    p16(o, 8, b)
}
fn check(b: &[u8], length: usize, kind: u8) -> Result<(), CodecError> {
    if b.len() != length {
        return Err(CodecError::Length);
    }
    if b[..2] != KLR9_SYNC || b[2] != 9 || b[3] != kind {
        return Err(CodecError::Enum);
    }
    if crc16_ccitt(&b[..length - 2]) != g16(b, length - 2) {
        return Err(CodecError::Checksum);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFastSensorCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub validity: u16,
    pub platform_angle: [i16; 3],
    pub angular_rate: [i16; 3],
    pub delta_velocity: [i16; 3],
    pub dynamic_pressure_q10: i32,
    pub mach_q12: i16,
    pub gimbal_applied: [i16; 2],
    pub canard_applied: [i16; 4],
    pub valve_open_mask: u16,
    pub propellant_q21: i32,
    pub supply_scale_q15: u16,
    pub vehicle_status: u16,
    pub actuator_feedback: u16,
    pub flags: u16,
}
pub fn write_advanced_fast_sensor(
    v: &AdvancedFastSensorCell,
    o: &mut [u8],
) -> Result<(), CodecError> {
    if o.len() != ADVANCED_FAST_SENSOR_LENGTH {
        return Err(CodecError::Length);
    }
    if v.validity & !ADVANCED_VALID_MASK != 0 || v.valve_open_mask & !0x0fff != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 1, v.session, v.measurement_epoch, v.production_epoch);
    p16(o, 10, v.validity);
    for i in 0..3 {
        pi16(o, 12 + i * 2, v.platform_angle[i]);
        pi16(o, 18 + i * 2, v.angular_rate[i]);
        pi16(o, 24 + i * 2, v.delta_velocity[i])
    }
    pi32(o, 30, v.dynamic_pressure_q10);
    pi16(o, 34, v.mach_q12);
    for i in 0..2 {
        pi16(o, 36 + i * 2, v.gimbal_applied[i])
    }
    for i in 0..4 {
        pi16(o, 40 + i * 2, v.canard_applied[i])
    }
    p16(o, 48, v.valve_open_mask);
    pi32(o, 50, v.propellant_q21);
    p16(o, 54, v.supply_scale_q15);
    p16(o, 56, v.vehicle_status);
    p16(o, 58, v.actuator_feedback);
    p16(o, 60, v.flags);
    p16(o, 62, crc16_ccitt(&o[..62]));
    Ok(())
}
pub fn parse_advanced_fast_sensor(b: &[u8]) -> Result<AdvancedFastSensorCell, CodecError> {
    check(b, ADVANCED_FAST_SENSOR_LENGTH, 1)?;
    let validity = g16(b, 10);
    let valves = g16(b, 48);
    if validity & !ADVANCED_VALID_MASK != 0 || valves & !0x0fff != 0 {
        return Err(CodecError::Flags);
    }
    let mut p = [0; 3];
    let mut r = [0; 3];
    let mut d = [0; 3];
    let mut c = [0; 4];
    for i in 0..3 {
        p[i] = gi16(b, 12 + i * 2);
        r[i] = gi16(b, 18 + i * 2);
        d[i] = gi16(b, 24 + i * 2)
    }
    for i in 0..4 {
        c[i] = gi16(b, 40 + i * 2)
    }
    Ok(AdvancedFastSensorCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity,
        platform_angle: p,
        angular_rate: r,
        delta_velocity: d,
        dynamic_pressure_q10: gi32(b, 30),
        mach_q12: gi16(b, 34),
        gimbal_applied: [gi16(b, 36), gi16(b, 38)],
        canard_applied: c,
        valve_open_mask: valves,
        propellant_q21: gi32(b, 50),
        supply_scale_q15: g16(b, 54),
        vehicle_status: g16(b, 56),
        actuator_feedback: g16(b, 58),
        flags: g16(b, 60),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedCommandCell {
    pub session: u16,
    pub source_epoch: u16,
    pub effective_epoch: u16,
    pub flags: u8,
    pub discrete: u8,
    pub gimbal: [i16; 2],
    pub canards: [i16; 4],
    pub torque_demand_q12: [i32; 3],
    pub rcs_pulse_quanta: [u8; 12],
    pub status: u16,
    pub authority_mode: u8,
    pub command_checksum: u32,
}
pub fn write_advanced_command(v: &AdvancedCommandCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != ADVANCED_COMMAND_LENGTH {
        return Err(CodecError::Length);
    }
    if v.flags & !ADVANCED_COMMAND_FLAG_MASK != 0
        || v.discrete & !ADVANCED_COMMAND_DISCRETE_MASK != 0
        || v.rcs_pulse_quanta.iter().any(|q| *q > 8)
    {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 2, v.session, v.source_epoch, v.effective_epoch);
    o[10] = v.flags;
    o[11] = v.discrete;
    for i in 0..2 {
        pi16(o, 12 + i * 2, v.gimbal[i])
    }
    for i in 0..4 {
        pi16(o, 16 + i * 2, v.canards[i])
    }
    for i in 0..3 {
        pi32(o, 24 + i * 4, v.torque_demand_q12[i])
    }
    o[36..48].copy_from_slice(&v.rcs_pulse_quanta);
    p16(o, 48, v.status);
    o[50] = v.authority_mode;
    p32(o, 52, v.command_checksum);
    p16(o, 62, crc16_ccitt(&o[..62]));
    Ok(())
}
pub fn parse_advanced_command(b: &[u8]) -> Result<AdvancedCommandCell, CodecError> {
    check(b, ADVANCED_COMMAND_LENGTH, 2)?;
    if b[10] & !ADVANCED_COMMAND_FLAG_MASK != 0
        || b[11] & !ADVANCED_COMMAND_DISCRETE_MASK != 0
        || b[36..48].iter().any(|q| *q > 8)
    {
        return Err(CodecError::Flags);
    }
    if b[51] != 0 || b[56..62].iter().any(|x| *x != 0) {
        return Err(CodecError::Reserved);
    }
    let mut c = [0; 4];
    let mut t = [0; 3];
    let mut q = [0; 12];
    for i in 0..4 {
        c[i] = gi16(b, 16 + i * 2)
    }
    for i in 0..3 {
        t[i] = gi32(b, 24 + i * 4)
    }
    q.copy_from_slice(&b[36..48]);
    Ok(AdvancedCommandCell {
        session: g16(b, 4),
        source_epoch: g16(b, 6),
        effective_epoch: g16(b, 8),
        flags: b[10],
        discrete: b[11],
        gimbal: [gi16(b, 12), gi16(b, 14)],
        canards: c,
        torque_demand_q12: t,
        rcs_pulse_quanta: q,
        status: g16(b, 48),
        authority_mode: b[50],
        command_checksum: g32(b, 52),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedAidCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub validity: u16,
    pub events: u16,
    pub onboard_time_q18: i32,
    pub barometer_q13: i32,
    pub gps_position_q13: [i32; 3],
    pub gps_velocity_q19: [i32; 3],
    pub attitude_vector: [i16; 3],
    pub continuity: u16,
    pub deployment_feedback: u16,
    pub vehicle_status: u32,
    pub clock_flags: u16,
}
pub fn write_advanced_aid(v: &AdvancedAidCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != ADVANCED_AID_LENGTH {
        return Err(CodecError::Length);
    }
    if v.validity & !ADVANCED_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 3, v.session, v.measurement_epoch, v.production_epoch);
    p16(o, 10, v.validity);
    p16(o, 12, v.events);
    pi32(o, 14, v.onboard_time_q18);
    pi32(o, 18, v.barometer_q13);
    for i in 0..3 {
        pi32(o, 22 + i * 4, v.gps_position_q13[i]);
        pi32(o, 34 + i * 4, v.gps_velocity_q19[i]);
        pi16(o, 46 + i * 2, v.attitude_vector[i])
    }
    p16(o, 52, v.continuity);
    p16(o, 54, v.deployment_feedback);
    p32(o, 56, v.vehicle_status);
    p16(o, 60, v.clock_flags);
    p16(o, 62, crc16_ccitt(&o[..62]));
    Ok(())
}
pub fn parse_advanced_aid(b: &[u8]) -> Result<AdvancedAidCell, CodecError> {
    check(b, ADVANCED_AID_LENGTH, 3)?;
    let validity = g16(b, 10);
    if validity & !ADVANCED_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let mut p = [0; 3];
    let mut v = [0; 3];
    let mut a = [0; 3];
    for i in 0..3 {
        p[i] = gi32(b, 22 + i * 4);
        v[i] = gi32(b, 34 + i * 4);
        a[i] = gi16(b, 46 + i * 2)
    }
    Ok(AdvancedAidCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity,
        events: g16(b, 12),
        onboard_time_q18: gi32(b, 14),
        barometer_q13: gi32(b, 18),
        gps_position_q13: p,
        gps_velocity_q19: v,
        attitude_vector: a,
        continuity: g16(b, 52),
        deployment_feedback: g16(b, 54),
        vehicle_status: g32(b, 56),
        clock_flags: g16(b, 60),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedStatusCell {
    pub session: u16,
    pub source_epoch: u16,
    pub production_epoch: u16,
    pub mode: u8,
    pub flags: u8,
    pub alarms: u16,
    pub navigation_position_q13: [i32; 3],
    pub navigation_velocity_q19: [i32; 3],
    pub flight_checksum: u32,
    pub deadline_misses: u16,
    pub navigation_checksum: u16,
    pub authority_state: u16,
    pub requested_torque_q12: [i32; 3],
    pub achieved_torque_q12: [i16; 3],
    pub residual_torque_q12: [i16; 3],
    pub saturation_count: u16,
    pub reserve_q15: u16,
    pub actuator_flags: u16,
}
pub fn write_advanced_status(v: &AdvancedStatusCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != ADVANCED_STATUS_LENGTH {
        return Err(CodecError::Length);
    }
    o.fill(0);
    prefix(o, 4, v.session, v.source_epoch, v.production_epoch);
    o[10] = v.mode;
    o[11] = v.flags;
    p16(o, 12, v.alarms);
    for i in 0..3 {
        pi32(o, 14 + i * 4, v.navigation_position_q13[i]);
        pi32(o, 26 + i * 4, v.navigation_velocity_q19[i]);
        pi32(o, 48 + i * 4, v.requested_torque_q12[i]);
        pi16(o, 60 + i * 2, v.achieved_torque_q12[i]);
        pi16(o, 66 + i * 2, v.residual_torque_q12[i])
    }
    p32(o, 38, v.flight_checksum);
    p16(o, 42, v.deadline_misses);
    p16(o, 44, v.navigation_checksum);
    p16(o, 46, v.authority_state);
    p16(o, 72, v.saturation_count);
    p16(o, 74, v.reserve_q15);
    p16(o, 76, v.actuator_flags);
    p16(o, 78, crc16_ccitt(&o[..78]));
    Ok(())
}
pub fn parse_advanced_status(b: &[u8]) -> Result<AdvancedStatusCell, CodecError> {
    check(b, ADVANCED_STATUS_LENGTH, 4)?;
    let mut p = [0; 3];
    let mut v = [0; 3];
    let mut req = [0; 3];
    let mut got = [0; 3];
    let mut residual = [0; 3];
    for i in 0..3 {
        p[i] = gi32(b, 14 + i * 4);
        v[i] = gi32(b, 26 + i * 4);
        req[i] = gi32(b, 48 + i * 4);
        got[i] = gi16(b, 60 + i * 2);
        residual[i] = gi16(b, 66 + i * 2)
    }
    Ok(AdvancedStatusCell {
        session: g16(b, 4),
        source_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        mode: b[10],
        flags: b[11],
        alarms: g16(b, 12),
        navigation_position_q13: p,
        navigation_velocity_q19: v,
        flight_checksum: g32(b, 38),
        deadline_misses: g16(b, 42),
        navigation_checksum: g16(b, 44),
        authority_state: g16(b, 46),
        requested_torque_q12: req,
        achieved_torque_q12: got,
        residual_torque_q12: residual,
        saturation_count: g16(b, 72),
        reserve_q15: g16(b, 74),
        actuator_flags: g16(b, 76),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(dead_code)]
    mod independent {
        include!("../../phase9_5/generated/contract_vectors_v1.rs");
    }
    #[test]
    fn cells_round_trip_and_reject_klr8() {
        let f = AdvancedFastSensorCell {
            session: 1,
            measurement_epoch: 2,
            production_epoch: 3,
            validity: 63,
            platform_angle: [1, 2, 3],
            angular_rate: [4, 5, 6],
            delta_velocity: [7, 8, 9],
            dynamic_pressure_q10: 10,
            mach_q12: 11,
            gimbal_applied: [12, 13],
            canard_applied: [14, 15, 16, 17],
            valve_open_mask: 0x555,
            propellant_q21: 18,
            supply_scale_q15: 19,
            vehicle_status: 20,
            actuator_feedback: 21,
            flags: 22,
        };
        let mut fb = [0; 64];
        write_advanced_fast_sensor(&f, &mut fb).unwrap();
        assert_eq!(fb, independent::KLR9_FAST_VECTOR);
        assert_eq!(parse_advanced_fast_sensor(&fb), Ok(f));
        fb[2] = 8;
        assert!(parse_advanced_fast_sensor(&fb).is_err());
        let c = AdvancedCommandCell {
            session: 1,
            source_epoch: 2,
            effective_epoch: 3,
            flags: 0,
            discrete: 3,
            gimbal: [1, 2],
            canards: [3, 4, 5, 6],
            torque_demand_q12: [7, 8, 9],
            rcs_pulse_quanta: [1; 12],
            status: 10,
            authority_mode: 2,
            command_checksum: 11,
        };
        let mut cb = [0; 64];
        write_advanced_command(&c, &mut cb).unwrap();
        assert_eq!(cb, independent::KLR9_COMMAND_VECTOR);
        assert_eq!(parse_advanced_command(&cb), Ok(c));
        let a = AdvancedAidCell {
            session: 1,
            measurement_epoch: 2,
            production_epoch: 3,
            validity: 31,
            events: 4,
            onboard_time_q18: 5,
            barometer_q13: 6,
            gps_position_q13: [7, 8, 9],
            gps_velocity_q19: [10, 11, 12],
            attitude_vector: [13, 14, 15],
            continuity: 16,
            deployment_feedback: 17,
            vehicle_status: 18,
            clock_flags: 19,
        };
        let mut ab = [0; 64];
        write_advanced_aid(&a, &mut ab).unwrap();
        assert_eq!(parse_advanced_aid(&ab), Ok(a));
        let s = AdvancedStatusCell {
            session: 1,
            source_epoch: 2,
            production_epoch: 3,
            mode: 4,
            flags: 5,
            alarms: 6,
            navigation_position_q13: [7, 8, 9],
            navigation_velocity_q19: [10, 11, 12],
            flight_checksum: 13,
            deadline_misses: 14,
            navigation_checksum: 15,
            authority_state: 16,
            requested_torque_q12: [17, 18, 19],
            achieved_torque_q12: [20, 21, 22],
            residual_torque_q12: [23, 24, 25],
            saturation_count: 26,
            reserve_q15: 27,
            actuator_flags: 28,
        };
        let mut sb = [0; 80];
        write_advanced_status(&s, &mut sb).unwrap();
        assert_eq!(parse_advanced_status(&sb), Ok(s));
    }
}
