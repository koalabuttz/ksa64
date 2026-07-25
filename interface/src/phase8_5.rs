//! Phase 8.5 local-ENU raw cells. KLF6 remains the outer transport.

use crate::phase6::crc16_ccitt;
use crate::CodecError;

pub const KLR8_CONTRACT_ID: u32 = 0x0852_0001;
pub const KLR8_SYNC: [u8; 2] = [0xd8, 0x5a];
pub const LOCAL_INERTIAL_LENGTH: usize = 40;
pub const LOCAL_COMMAND_LENGTH: usize = 24;
pub const LOCAL_AID_LENGTH: usize = 64;
pub const LOCAL_STATUS_LENGTH: usize = 48;

pub const LOCAL_INERTIAL_VALID_PLATFORM: u8 = 1;
pub const LOCAL_INERTIAL_VALID_RATE: u8 = 2;
pub const LOCAL_INERTIAL_VALID_DELTA_V: u8 = 4;
pub const LOCAL_INERTIAL_VALID_ACTUATOR: u8 = 8;
pub const LOCAL_INERTIAL_VALID_MASK: u8 = 15;
pub const LOCAL_AID_BAROMETER: u16 = 1;
pub const LOCAL_AID_GPS: u16 = 2;
pub const LOCAL_AID_ATTITUDE: u16 = 4;
pub const LOCAL_AID_CONTINUITY: u16 = 8;
pub const LOCAL_AID_DEPLOYMENT_FEEDBACK: u16 = 16;
pub const LOCAL_AID_VALID_MASK: u16 = 31;
pub const LOCAL_COMMAND_DROGUE: u8 = 1;
pub const LOCAL_COMMAND_MAIN: u8 = 2;
pub const LOCAL_COMMAND_SAFE: u8 = 4;
pub const LOCAL_COMMAND_DISCRETE_MASK: u8 = 7;

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
    o[..2].copy_from_slice(&KLR8_SYNC);
    o[2] = 8;
    o[3] = kind;
    p16(o, 4, session);
    p16(o, 6, a);
    p16(o, 8, b);
}
fn check(b: &[u8], length: usize, kind: u8) -> Result<(), CodecError> {
    if b.len() != length {
        return Err(CodecError::Length);
    }
    if b[..2] != KLR8_SYNC || b[2] != 8 || b[3] != kind {
        return Err(CodecError::Enum);
    }
    if crc16_ccitt(&b[..length - 2]) != g16(b, length - 2) {
        return Err(CodecError::Checksum);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalInertialCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub validity: u8,
    pub flags: u8,
    pub platform_angle: [i16; 3],
    pub angular_rate: [i16; 3],
    pub delta_velocity: [i16; 3],
    pub gimbal_applied: [i16; 2],
    pub vehicle_status: u16,
    pub actuator_feedback: u16,
}
pub fn write_local_inertial(v: &LocalInertialCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != LOCAL_INERTIAL_LENGTH || v.validity & !LOCAL_INERTIAL_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 1, v.session, v.measurement_epoch, v.production_epoch);
    o[10] = v.validity;
    o[11] = v.flags;
    let mut a = 0;
    while a < 3 {
        pi16(o, 12 + a * 2, v.platform_angle[a]);
        pi16(o, 18 + a * 2, v.angular_rate[a]);
        pi16(o, 24 + a * 2, v.delta_velocity[a]);
        a += 1;
    }
    pi16(o, 30, v.gimbal_applied[0]);
    pi16(o, 32, v.gimbal_applied[1]);
    p16(o, 34, v.vehicle_status);
    p16(o, 36, v.actuator_feedback);
    p16(o, 38, crc16_ccitt(&o[..38]));
    Ok(())
}
pub fn parse_local_inertial(b: &[u8]) -> Result<LocalInertialCell, CodecError> {
    check(b, LOCAL_INERTIAL_LENGTH, 1)?;
    if b[10] & !LOCAL_INERTIAL_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let mut p = [0; 3];
    let mut r = [0; 3];
    let mut d = [0; 3];
    let mut a = 0;
    while a < 3 {
        p[a] = gi16(b, 12 + a * 2);
        r[a] = gi16(b, 18 + a * 2);
        d[a] = gi16(b, 24 + a * 2);
        a += 1;
    }
    Ok(LocalInertialCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity: b[10],
        flags: b[11],
        platform_angle: p,
        angular_rate: r,
        delta_velocity: d,
        gimbal_applied: [gi16(b, 30), gi16(b, 32)],
        vehicle_status: g16(b, 34),
        actuator_feedback: g16(b, 36),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalCommandCell {
    pub session: u16,
    pub source_epoch: u16,
    pub effective_epoch: u16,
    pub flags: u8,
    pub discrete: u8,
    pub gimbal: [i16; 2],
    pub control_demand: [i16; 2],
    pub status: u16,
}
pub fn write_local_command(v: &LocalCommandCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != LOCAL_COMMAND_LENGTH || v.discrete & !LOCAL_COMMAND_DISCRETE_MASK != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 2, v.session, v.source_epoch, v.effective_epoch);
    o[10] = v.flags;
    o[11] = v.discrete;
    pi16(o, 12, v.gimbal[0]);
    pi16(o, 14, v.gimbal[1]);
    pi16(o, 16, v.control_demand[0]);
    pi16(o, 18, v.control_demand[1]);
    p16(o, 20, v.status);
    p16(o, 22, crc16_ccitt(&o[..22]));
    Ok(())
}
pub fn parse_local_command(b: &[u8]) -> Result<LocalCommandCell, CodecError> {
    check(b, LOCAL_COMMAND_LENGTH, 2)?;
    if b[11] & !LOCAL_COMMAND_DISCRETE_MASK != 0 {
        return Err(CodecError::Flags);
    }
    Ok(LocalCommandCell {
        session: g16(b, 4),
        source_epoch: g16(b, 6),
        effective_epoch: g16(b, 8),
        flags: b[10],
        discrete: b[11],
        gimbal: [gi16(b, 12), gi16(b, 14)],
        control_demand: [gi16(b, 16), gi16(b, 18)],
        status: g16(b, 20),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAidCell {
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
pub fn write_local_aid(v: &LocalAidCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != LOCAL_AID_LENGTH || v.validity & !LOCAL_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    prefix(o, 3, v.session, v.measurement_epoch, v.production_epoch);
    p16(o, 10, v.validity);
    p16(o, 12, v.events);
    pi32(o, 14, v.onboard_time_q18);
    pi32(o, 18, v.barometer_q13);
    let mut a = 0;
    while a < 3 {
        pi32(o, 22 + a * 4, v.gps_position_q13[a]);
        pi32(o, 34 + a * 4, v.gps_velocity_q19[a]);
        pi16(o, 46 + a * 2, v.attitude_vector[a]);
        a += 1;
    }
    p16(o, 52, v.continuity);
    p16(o, 54, v.deployment_feedback);
    p32(o, 56, v.vehicle_status);
    p16(o, 60, v.clock_flags);
    p16(o, 62, crc16_ccitt(&o[..62]));
    Ok(())
}
pub fn parse_local_aid(b: &[u8]) -> Result<LocalAidCell, CodecError> {
    check(b, LOCAL_AID_LENGTH, 3)?;
    let validity = g16(b, 10);
    if validity & !LOCAL_AID_VALID_MASK != 0 {
        return Err(CodecError::Flags);
    }
    let mut p = [0; 3];
    let mut v = [0; 3];
    let mut q = [0; 3];
    let mut a = 0;
    while a < 3 {
        p[a] = gi32(b, 22 + a * 4);
        v[a] = gi32(b, 34 + a * 4);
        q[a] = gi16(b, 46 + a * 2);
        a += 1;
    }
    Ok(LocalAidCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity,
        events: g16(b, 12),
        onboard_time_q18: gi32(b, 14),
        barometer_q13: gi32(b, 18),
        gps_position_q13: p,
        gps_velocity_q19: v,
        attitude_vector: q,
        continuity: g16(b, 52),
        deployment_feedback: g16(b, 54),
        vehicle_status: g32(b, 56),
        clock_flags: g16(b, 60),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalStatusCell {
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
}
pub fn write_local_status(v: &LocalStatusCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != LOCAL_STATUS_LENGTH {
        return Err(CodecError::Length);
    }
    o.fill(0);
    prefix(o, 4, v.session, v.source_epoch, v.production_epoch);
    o[10] = v.mode;
    o[11] = v.flags;
    p16(o, 12, v.alarms);
    let mut a = 0;
    while a < 3 {
        pi32(o, 14 + a * 4, v.navigation_position_q13[a]);
        pi32(o, 26 + a * 4, v.navigation_velocity_q19[a]);
        a += 1;
    }
    p32(o, 38, v.flight_checksum);
    p16(o, 42, v.deadline_misses);
    p16(o, 44, v.navigation_checksum);
    p16(o, 46, crc16_ccitt(&o[..46]));
    Ok(())
}
pub fn parse_local_status(b: &[u8]) -> Result<LocalStatusCell, CodecError> {
    check(b, LOCAL_STATUS_LENGTH, 4)?;
    let mut p = [0; 3];
    let mut v = [0; 3];
    let mut a = 0;
    while a < 3 {
        p[a] = gi32(b, 14 + a * 4);
        v[a] = gi32(b, 26 + a * 4);
        a += 1;
    }
    Ok(LocalStatusCell {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cells_round_trip_and_reject_cross_contract() {
        let i = LocalInertialCell {
            session: 7,
            measurement_epoch: 8,
            production_epoch: 8,
            validity: 15,
            flags: 0,
            platform_angle: [1, -2, 3],
            angular_rate: [4, 5, 6],
            delta_velocity: [7, 8, 9],
            gimbal_applied: [10, -11],
            vehicle_status: 12,
            actuator_feedback: 13,
        };
        let mut ib = [0; LOCAL_INERTIAL_LENGTH];
        write_local_inertial(&i, &mut ib).unwrap();
        assert_eq!(parse_local_inertial(&ib), Ok(i));
        ib[2] = 6;
        assert!(parse_local_inertial(&ib).is_err());
        let c = LocalCommandCell {
            session: 7,
            source_epoch: 8,
            effective_epoch: 9,
            flags: 0,
            discrete: 3,
            gimbal: [1, -1],
            control_demand: [2, -2],
            status: 5,
        };
        let mut cb = [0; LOCAL_COMMAND_LENGTH];
        write_local_command(&c, &mut cb).unwrap();
        assert_eq!(parse_local_command(&cb), Ok(c));
        let a = LocalAidCell {
            session: 7,
            measurement_epoch: 8,
            production_epoch: 8,
            validity: 31,
            events: 3,
            onboard_time_q18: 8192,
            barometer_q13: 4,
            gps_position_q13: [5, 6, 7],
            gps_velocity_q19: [8, 9, 10],
            attitude_vector: [11, 12, 13],
            continuity: 3,
            deployment_feedback: 1,
            vehicle_status: 14,
            clock_flags: 0,
        };
        let mut ab = [0; LOCAL_AID_LENGTH];
        write_local_aid(&a, &mut ab).unwrap();
        assert_eq!(parse_local_aid(&ab), Ok(a));
        let s = LocalStatusCell {
            session: 7,
            source_epoch: 8,
            production_epoch: 8,
            mode: 2,
            flags: 0,
            alarms: 0,
            navigation_position_q13: [1, 2, 3],
            navigation_velocity_q19: [4, 5, 6],
            flight_checksum: 7,
            deadline_misses: 8,
            navigation_checksum: 9,
        };
        let mut sb = [0; LOCAL_STATUS_LENGTH];
        write_local_status(&s, &mut sb).unwrap();
        assert_eq!(parse_local_status(&sb), Ok(s));
    }
}
