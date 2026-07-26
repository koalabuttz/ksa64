//! Strict Phase 9.5 KAT9 canonical advanced-effector telemetry.

// Explicit wire indices keep the MOS layout auditable.
#![allow(clippy::needless_range_loop)]

use crate::phase9_5_contract::{KAT9_FRAME_LENGTH, KAT9_HEADER_LENGTH, PHASE95_CONTRACT_ID};
use crate::scenario::crc32_ieee;

const VERSION: u16 = 9;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kat9Error {
    Length,
    Magic,
    Version,
    Contract,
    Identity,
    Reserved,
    Checksum,
    Range,
}
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
fn is_zero(b: &[u8], s: usize, e: usize) -> bool {
    b[s..e].iter().all(|x| *x == 0)
}
fn seal(o: &mut [u8]) {
    let at = o.len() - 4;
    let c = crc32_ieee(&o[..at]);
    p32(o, at, c)
}
fn crc_ok(b: &[u8]) -> bool {
    let at = b.len() - 4;
    g32(b, at) == crc32_ieee(&b[..at])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedTelemetryHeader {
    pub identity: u32,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub wind_identity: u32,
    pub avionics_identity: u32,
    pub effector_identity: u32,
    pub allocator_identity: u32,
    pub uncertainty_identity: u32,
    pub frame_count: u32,
    pub period_q18: i32,
    pub start_time_q18: i32,
    pub source_checksum: u32,
}
pub fn write_kat9_header(v: AdvancedTelemetryHeader, o: &mut [u8]) -> Result<(), Kat9Error> {
    if o.len() != KAT9_HEADER_LENGTH {
        return Err(Kat9Error::Length);
    }
    if v.identity == 0
        || v.vehicle_identity == 0
        || v.motor_identity == 0
        || v.mission_identity == 0
        || v.wind_identity == 0
        || v.avionics_identity == 0
        || v.effector_identity == 0
        || v.allocator_identity == 0
        || v.period_q18 <= 0
    {
        return Err(Kat9Error::Identity);
    }
    o.fill(0);
    o[..4].copy_from_slice(b"KAT9");
    p16(o, 4, VERSION);
    p16(o, 6, KAT9_HEADER_LENGTH as u16);
    p16(o, 8, KAT9_FRAME_LENGTH as u16);
    p32(o, 12, PHASE95_CONTRACT_ID);
    p32(o, 16, v.identity);
    for (i, x) in [
        v.vehicle_identity,
        v.motor_identity,
        v.mission_identity,
        v.wind_identity,
        v.avionics_identity,
        v.effector_identity,
        v.allocator_identity,
        v.uncertainty_identity,
    ]
    .into_iter()
    .enumerate()
    {
        p32(o, 20 + i * 4, x)
    }
    p32(o, 52, v.frame_count);
    pi32(o, 56, v.period_q18);
    pi32(o, 60, v.start_time_q18);
    p32(o, 64, v.source_checksum);
    seal(o);
    Ok(())
}
pub fn parse_kat9_header(b: &[u8]) -> Result<AdvancedTelemetryHeader, Kat9Error> {
    if b.len() != KAT9_HEADER_LENGTH {
        return Err(Kat9Error::Length);
    }
    if b[..4] != *b"KAT9" {
        return Err(Kat9Error::Magic);
    }
    if g16(b, 4) != VERSION
        || g16(b, 6) as usize != KAT9_HEADER_LENGTH
        || g16(b, 8) as usize != KAT9_FRAME_LENGTH
    {
        return Err(Kat9Error::Version);
    }
    if g16(b, 10) != 0 || !is_zero(b, 68, KAT9_HEADER_LENGTH - 4) {
        return Err(Kat9Error::Reserved);
    }
    if g32(b, 12) != PHASE95_CONTRACT_ID {
        return Err(Kat9Error::Contract);
    }
    if !crc_ok(b) {
        return Err(Kat9Error::Checksum);
    }
    let v = AdvancedTelemetryHeader {
        identity: g32(b, 16),
        vehicle_identity: g32(b, 20),
        motor_identity: g32(b, 24),
        mission_identity: g32(b, 28),
        wind_identity: g32(b, 32),
        avionics_identity: g32(b, 36),
        effector_identity: g32(b, 40),
        allocator_identity: g32(b, 44),
        uncertainty_identity: g32(b, 48),
        frame_count: g32(b, 52),
        period_q18: gi32(b, 56),
        start_time_q18: gi32(b, 60),
        source_checksum: g32(b, 64),
    };
    if v.identity == 0
        || v.vehicle_identity == 0
        || v.motor_identity == 0
        || v.mission_identity == 0
        || v.wind_identity == 0
        || v.avionics_identity == 0
        || v.effector_identity == 0
        || v.allocator_identity == 0
        || v.period_q18 <= 0
    {
        return Err(Kat9Error::Identity);
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedTelemetryFrame {
    pub time_q18: i32,
    pub epoch: u16,
    pub phase: u8,
    pub events: u16,
    pub flags: u16,
    pub truth_position_q13: [i32; 3],
    pub truth_velocity_q19: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub angular_rate_q24: [i32; 3],
    pub navigation_position_q13: [i32; 3],
    pub navigation_velocity_q19: [i32; 3],
    pub dynamic_pressure_q10: i32,
    pub mach_q12: i16,
    pub mass_q21: i32,
    pub cg_q28: i32,
    pub gimbal: [i16; 2],
    pub canards: [i16; 4],
    pub valve_mask: u16,
    pub authority_state: u16,
    pub propellant_q21: i32,
    pub pressure_q8: i32,
    pub supply_scale_q15: u16,
    pub requested_torque_q12: [i32; 3],
    pub achieved_torque_q12: [i32; 3],
    pub residual_torque_q12: [i32; 3],
    pub hinge_q24: [i32; 4],
    pub pulse_quanta: [u8; 12],
    pub alarms: u16,
    pub saturation_count: u16,
    pub checksums: [u32; 8],
    pub rcs_force_body_q23: [i32; 3],
    pub rcs_torque_body_q12: [i32; 3],
}
pub fn write_kat9_frame(v: &AdvancedTelemetryFrame, o: &mut [u8]) -> Result<(), Kat9Error> {
    if o.len() != KAT9_FRAME_LENGTH {
        return Err(Kat9Error::Length);
    }
    if v.phase > 6 || v.valve_mask & !0x0fff != 0 || v.pulse_quanta.iter().any(|q| *q > 8) {
        return Err(Kat9Error::Range);
    }
    o.fill(0);
    pi32(o, 0, v.time_q18);
    p16(o, 4, v.epoch);
    o[6] = v.phase;
    p16(o, 8, v.events);
    p16(o, 10, v.flags);
    for i in 0..3 {
        pi32(o, 12 + i * 4, v.truth_position_q13[i]);
        pi32(o, 24 + i * 4, v.truth_velocity_q19[i]);
        pi32(o, 52 + i * 4, v.angular_rate_q24[i]);
        pi32(o, 64 + i * 4, v.navigation_position_q13[i]);
        pi32(o, 76 + i * 4, v.navigation_velocity_q19[i]);
    }
    for i in 0..4 {
        pi32(o, 36 + i * 4, v.attitude_q30[i]);
        pi16(o, 108 + i * 2, v.canards[i]);
        pi32(o, 168 + i * 4, v.hinge_q24[i]);
    }
    pi32(o, 88, v.dynamic_pressure_q10);
    pi16(o, 92, v.mach_q12);
    pi32(o, 96, v.mass_q21);
    pi32(o, 100, v.cg_q28);
    for i in 0..2 {
        pi16(o, 104 + i * 2, v.gimbal[i]);
    }
    p16(o, 116, v.valve_mask);
    p16(o, 118, v.authority_state);
    pi32(o, 120, v.propellant_q21);
    pi32(o, 124, v.pressure_q8);
    p16(o, 128, v.supply_scale_q15);
    for i in 0..3 {
        pi32(o, 132 + i * 4, v.requested_torque_q12[i]);
        pi32(o, 144 + i * 4, v.achieved_torque_q12[i]);
        pi32(o, 156 + i * 4, v.residual_torque_q12[i]);
        pi32(o, 232 + i * 4, v.rcs_force_body_q23[i]);
        pi32(o, 244 + i * 4, v.rcs_torque_body_q12[i]);
    }
    o[184..196].copy_from_slice(&v.pulse_quanta);
    p16(o, 196, v.alarms);
    p16(o, 198, v.saturation_count);
    for i in 0..8 {
        p32(o, 200 + i * 4, v.checksums[i]);
    }
    seal(o);
    Ok(())
}
pub fn parse_kat9_frame(b: &[u8]) -> Result<AdvancedTelemetryFrame, Kat9Error> {
    if b.len() != KAT9_FRAME_LENGTH {
        return Err(Kat9Error::Length);
    }
    if b[6] > 6 || b[7] != 0 || g16(b, 116) & !0x0fff != 0 || b[184..196].iter().any(|q| *q > 8) {
        return Err(Kat9Error::Range);
    }
    if !is_zero(b, 130, 132) || !is_zero(b, 256, KAT9_FRAME_LENGTH - 4) {
        return Err(Kat9Error::Reserved);
    }
    if !crc_ok(b) {
        return Err(Kat9Error::Checksum);
    }
    let mut tp = [0; 3];
    let mut tv = [0; 3];
    let mut ar = [0; 3];
    let mut np = [0; 3];
    let mut nv = [0; 3];
    let mut rq = [0; 3];
    let mut aq = [0; 3];
    let mut xq = [0; 3];
    let mut rf = [0; 3];
    let mut rt = [0; 3];
    let mut att = [0; 4];
    let mut can = [0; 4];
    let mut hinge = [0; 4];
    let mut pulse = [0; 12];
    let mut checks = [0; 8];
    for i in 0..3 {
        tp[i] = gi32(b, 12 + i * 4);
        tv[i] = gi32(b, 24 + i * 4);
        ar[i] = gi32(b, 52 + i * 4);
        np[i] = gi32(b, 64 + i * 4);
        nv[i] = gi32(b, 76 + i * 4);
        rq[i] = gi32(b, 132 + i * 4);
        aq[i] = gi32(b, 144 + i * 4);
        xq[i] = gi32(b, 156 + i * 4);
        rf[i] = gi32(b, 232 + i * 4);
        rt[i] = gi32(b, 244 + i * 4);
    }
    for i in 0..4 {
        att[i] = gi32(b, 36 + i * 4);
        can[i] = gi16(b, 108 + i * 2);
        hinge[i] = gi32(b, 168 + i * 4);
    }
    pulse.copy_from_slice(&b[184..196]);
    for i in 0..8 {
        checks[i] = g32(b, 200 + i * 4);
    }
    Ok(AdvancedTelemetryFrame {
        time_q18: gi32(b, 0),
        epoch: g16(b, 4),
        phase: b[6],
        events: g16(b, 8),
        flags: g16(b, 10),
        truth_position_q13: tp,
        truth_velocity_q19: tv,
        attitude_q30: att,
        angular_rate_q24: ar,
        navigation_position_q13: np,
        navigation_velocity_q19: nv,
        dynamic_pressure_q10: gi32(b, 88),
        mach_q12: gi16(b, 92),
        mass_q21: gi32(b, 96),
        cg_q28: gi32(b, 100),
        gimbal: [gi16(b, 104), gi16(b, 106)],
        canards: can,
        valve_mask: g16(b, 116),
        authority_state: g16(b, 118),
        propellant_q21: gi32(b, 120),
        pressure_q8: gi32(b, 124),
        supply_scale_q15: g16(b, 128),
        requested_torque_q12: rq,
        achieved_torque_q12: aq,
        residual_torque_q12: xq,
        hinge_q24: hinge,
        pulse_quanta: pulse,
        alarms: g16(b, 196),
        saturation_count: g16(b, 198),
        checksums: checks,
        rcs_force_body_q23: rf,
        rcs_torque_body_q12: rt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_round_trip() {
        let h = AdvancedTelemetryHeader {
            identity: 1,
            vehicle_identity: 2,
            motor_identity: 3,
            mission_identity: 4,
            wind_identity: 5,
            avionics_identity: 6,
            effector_identity: 7,
            allocator_identity: 8,
            uncertainty_identity: 0,
            frame_count: 9,
            period_q18: 8192,
            start_time_q18: 0,
            source_checksum: 10,
        };
        let mut hb = [0u8; KAT9_HEADER_LENGTH];
        write_kat9_header(h, &mut hb).unwrap();
        assert_eq!(parse_kat9_header(&hb), Ok(h));
        let f = AdvancedTelemetryFrame {
            time_q18: 0,
            epoch: 0,
            phase: 1,
            events: 0,
            flags: 0,
            truth_position_q13: [1, 2, 3],
            truth_velocity_q19: [4, 5, 6],
            attitude_q30: [1 << 30, 0, 0, 0],
            angular_rate_q24: [0; 3],
            navigation_position_q13: [1, 2, 3],
            navigation_velocity_q19: [4, 5, 6],
            dynamic_pressure_q10: 7,
            mach_q12: 8,
            mass_q21: 9,
            cg_q28: 10,
            gimbal: [0; 2],
            canards: [0; 4],
            valve_mask: 0,
            authority_state: 0,
            propellant_q21: 11,
            pressure_q8: 12,
            supply_scale_q15: 13,
            requested_torque_q12: [14; 3],
            achieved_torque_q12: [15; 3],
            residual_torque_q12: [-1; 3],
            hinge_q24: [0; 4],
            pulse_quanta: [0; 12],
            alarms: 0,
            saturation_count: 0,
            checksums: [0; 8],
            rcs_force_body_q23: [0; 3],
            rcs_torque_body_q12: [0; 3],
        };
        let mut fb = [0u8; KAT9_FRAME_LENGTH];
        write_kat9_frame(&f, &mut fb).unwrap();
        assert_eq!(parse_kat9_frame(&fb), Ok(f));
        fb[300] = 1;
        seal(&mut fb);
        assert_eq!(parse_kat9_frame(&fb), Err(Kat9Error::Reserved));
    }
}
