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

pub const KAT8_HEADER_LENGTH: usize = 128;
pub const KAT8_FRAME_LENGTH: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kat8Header {
    pub identity: u32,
    pub evaluation_request_identity: u32,
    pub session: u16,
    pub release_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kat8Frame {
    pub epoch: u16,
    pub phase: u8,
    pub flags: u8,
    pub time_q18: i32,
    pub director_checksum: u32,
    pub inertial: LocalInertialCell,
    pub command: LocalCommandCell,
    pub status: Option<LocalStatusCell>,
    pub aid_crc16: u16,
    pub aid_validity: u16,
    pub truth_altitude_q13: i32,
    pub truth_velocity_q19: [i32; 3],
    pub applied_gimbal: [i16; 2],
    pub events: u16,
    pub deployment_feedback: u16,
}

fn crc32_ieee_local(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
            bit += 1;
        }
    }
    !crc
}

pub fn write_kat8_header(value: Kat8Header, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KAT8_HEADER_LENGTH || value.identity == 0 || value.session == 0 {
        return Err(CodecError::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KAT8");
    p16(output, 4, 8);
    p16(output, 6, KAT8_HEADER_LENGTH as u16);
    p16(output, 8, KAT8_FRAME_LENGTH as u16);
    p32(output, 12, value.identity);
    p32(output, 16, value.evaluation_request_identity);
    p32(output, 20, KLR8_CONTRACT_ID);
    p16(output, 24, value.session);
    p32(output, 28, value.release_count);
    p32(output, 124, crc32_ieee_local(&output[..124]));
    Ok(())
}

pub fn parse_kat8_header(input: &[u8]) -> Result<Kat8Header, CodecError> {
    if input.len() != KAT8_HEADER_LENGTH {
        return Err(CodecError::Length);
    }
    if &input[..4] != b"KAT8" || g16(input, 4) != 8 || g16(input, 6) != 128 || g16(input, 8) != 160
    {
        return Err(CodecError::Enum);
    }
    if input[10..12]
        .iter()
        .chain(input[26..28].iter())
        .chain(input[32..124].iter())
        .any(|v| *v != 0)
    {
        return Err(CodecError::Flags);
    }
    if g32(input, 20) != KLR8_CONTRACT_ID || g32(input, 124) != crc32_ieee_local(&input[..124]) {
        return Err(CodecError::Checksum);
    }
    Ok(Kat8Header {
        identity: g32(input, 12),
        evaluation_request_identity: g32(input, 16),
        session: g16(input, 24),
        release_count: g32(input, 28),
    })
}

pub fn write_kat8_frame(value: &Kat8Frame, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KAT8_FRAME_LENGTH {
        return Err(CodecError::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KTF8");
    p16(output, 4, value.epoch);
    output[6] = value.flags;
    output[7] = value.phase;
    pi32(output, 8, value.time_q18);
    p32(output, 12, value.director_checksum);
    write_local_inertial(&value.inertial, &mut output[16..56])?;
    write_local_command(&value.command, &mut output[56..80])?;
    if let Some(status) = value.status {
        write_local_status(&status, &mut output[80..128])?;
    }
    p16(output, 128, value.aid_crc16);
    p16(output, 130, value.aid_validity);
    pi32(output, 132, value.truth_altitude_q13);
    for (index, velocity) in value.truth_velocity_q19.iter().enumerate() {
        pi32(output, 136 + index * 4, *velocity);
    }
    pi16(output, 148, value.applied_gimbal[0]);
    pi16(output, 150, value.applied_gimbal[1]);
    p16(output, 152, value.events);
    p16(output, 154, value.deployment_feedback);
    p32(output, 156, crc32_ieee_local(&output[..156]));
    Ok(())
}

pub fn parse_kat8_frame(input: &[u8]) -> Result<Kat8Frame, CodecError> {
    if input.len() != KAT8_FRAME_LENGTH || &input[..4] != b"KTF8" {
        return Err(CodecError::Length);
    }
    if g32(input, 156) != crc32_ieee_local(&input[..156]) {
        return Err(CodecError::Checksum);
    }
    let status = if input[80..128].iter().all(|value| *value == 0) {
        None
    } else {
        Some(parse_local_status(&input[80..128])?)
    };
    Ok(Kat8Frame {
        epoch: g16(input, 4),
        flags: input[6],
        phase: input[7],
        time_q18: gi32(input, 8),
        director_checksum: g32(input, 12),
        inertial: parse_local_inertial(&input[16..56])?,
        command: parse_local_command(&input[56..80])?,
        status,
        aid_crc16: g16(input, 128),
        aid_validity: g16(input, 130),
        truth_altitude_q13: gi32(input, 132),
        truth_velocity_q19: [gi32(input, 136), gi32(input, 140), gi32(input, 144)],
        applied_gimbal: [gi16(input, 148), gi16(input, 150)],
        events: g16(input, 152),
        deployment_feedback: g16(input, 154),
    })
}

#[cfg(test)]
mod kat8_tests {
    use super::*;
    #[test]
    fn kat8_header_and_frame_are_strict() {
        let header = Kat8Header {
            identity: 1,
            evaluation_request_identity: 2,
            session: 3,
            release_count: 4,
        };
        let mut hb = [0; KAT8_HEADER_LENGTH];
        write_kat8_header(header, &mut hb).unwrap();
        assert_eq!(parse_kat8_header(&hb), Ok(header));
        let inertial = LocalInertialCell {
            session: 3,
            measurement_epoch: 0,
            production_epoch: 0,
            validity: LOCAL_INERTIAL_VALID_MASK,
            flags: 0,
            platform_angle: [0; 3],
            angular_rate: [0; 3],
            delta_velocity: [0; 3],
            gimbal_applied: [0; 2],
            vehicle_status: 0,
            actuator_feedback: 0,
        };
        let command = LocalCommandCell {
            session: 3,
            source_epoch: 0,
            effective_epoch: 1,
            flags: 0,
            discrete: 0,
            gimbal: [0; 2],
            control_demand: [0; 2],
            status: 0,
        };
        let frame = Kat8Frame {
            epoch: 0,
            phase: 1,
            flags: 0,
            time_q18: 0,
            director_checksum: 4,
            inertial,
            command,
            status: None,
            aid_crc16: 0,
            aid_validity: 0,
            truth_altitude_q13: 0,
            truth_velocity_q19: [0; 3],
            applied_gimbal: [0; 2],
            events: 0,
            deployment_feedback: 0,
        };
        let mut fb = [0; KAT8_FRAME_LENGTH];
        write_kat8_frame(&frame, &mut fb).unwrap();
        assert_eq!(parse_kat8_frame(&fb), Ok(frame));
        fb[140] ^= 1;
        assert_eq!(parse_kat8_frame(&fb), Err(CodecError::Checksum));
    }
}
