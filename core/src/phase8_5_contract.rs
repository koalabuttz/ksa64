//! Strict additive Phase 8.5 avionics, capability, and evaluation packs.

use crate::scenario::crc32_ieee;

pub const PHASE85_AVIONICS_CONTRACT_ID: u32 = 0x0850_0001;
pub const KAP8_LENGTH: usize = 512;
pub const KAC8_LENGTH: usize = 256;
pub const KLE8_LENGTH: usize = 128;
pub const KAT8_HEADER_LENGTH: usize = 128;
pub const KAT8_FRAME_LENGTH: usize = 160;
pub const KAS8_LENGTH: usize = 256;
pub const KMR8_HEADER_LENGTH: usize = 128;
const HEADER_LENGTH: usize = 32;
const VERSION: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReferenceFrameId {
    LocalEnuV1 = 1,
    EarthFixedEcefV1 = 2,
    EarthInertialEciV1 = 3,
}
impl ReferenceFrameId {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::LocalEnuV1)
    }
    fn parse(v: u8) -> Result<Self, Phase85ContractError> {
        match v {
            1 => Ok(Self::LocalEnuV1),
            2 => Ok(Self::EarthFixedEcefV1),
            3 => Ok(Self::EarthInertialEciV1),
            _ => Err(Phase85ContractError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AvionicsProfileId {
    Ksa6R = 1,
    LocalEnuRecoveryV1 = 2,
    LocalEnuGimbalV1 = 3,
}
impl AvionicsProfileId {
    fn parse(v: u8) -> Result<Self, Phase85ContractError> {
        match v {
            1 => Ok(Self::Ksa6R),
            2 => Ok(Self::LocalEnuRecoveryV1),
            3 => Ok(Self::LocalEnuGimbalV1),
            _ => Err(Phase85ContractError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ActuatorCapabilityId {
    MonitorOnlyV1 = 1,
    TwoAxisMotorGimbalV1 = 2,
}
impl ActuatorCapabilityId {
    fn parse(v: u8) -> Result<Self, Phase85ContractError> {
        match v {
            1 => Ok(Self::MonitorOnlyV1),
            2 => Ok(Self::TwoAxisMotorGimbalV1),
            _ => Err(Phase85ContractError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
enum RecordKind {
    Avionics = 1,
    Capability = 2,
    Evaluation = 3,
}
impl RecordKind {
    const fn magic(self) -> [u8; 4] {
        match self {
            Self::Avionics => *b"KAP8",
            Self::Capability => *b"KAC8",
            Self::Evaluation => *b"KLE8",
        }
    }
    const fn length(self) -> usize {
        match self {
            Self::Avionics => KAP8_LENGTH,
            Self::Capability => KAC8_LENGTH,
            Self::Evaluation => KLE8_LENGTH,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase85ContractError {
    Length,
    Magic,
    Version,
    Kind,
    Contract,
    Identity,
    Reserved,
    Checksum,
    Enum,
    Range,
    Unsupported,
}
fn p16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
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
fn g32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn gi32(b: &[u8], i: usize) -> i32 {
    g32(b, i) as i32
}
fn write_header(o: &mut [u8], kind: RecordKind, identity: u32) -> Result<(), Phase85ContractError> {
    if o.len() != kind.length() {
        return Err(Phase85ContractError::Length);
    }
    if identity == 0 {
        return Err(Phase85ContractError::Identity);
    }
    o.fill(0);
    o[..4].copy_from_slice(&kind.magic());
    p16(o, 4, VERSION);
    p16(o, 6, HEADER_LENGTH as u16);
    p16(o, 8, o.len() as u16);
    p16(o, 10, kind as u16);
    p32(o, 12, PHASE85_AVIONICS_CONTRACT_ID);
    p32(o, 16, identity);
    Ok(())
}
fn seal(o: &mut [u8]) {
    let at = o.len() - 4;
    let c = crc32_ieee(&o[..at]);
    p32(o, at, c)
}
fn validate(b: &[u8], kind: RecordKind) -> Result<u32, Phase85ContractError> {
    if b.len() != kind.length() {
        return Err(Phase85ContractError::Length);
    }
    if b[..4] != kind.magic() {
        return Err(Phase85ContractError::Magic);
    }
    if g16(b, 4) != VERSION || g16(b, 6) as usize != HEADER_LENGTH {
        return Err(Phase85ContractError::Version);
    }
    if g16(b, 8) as usize != b.len() || g16(b, 10) != kind as u16 {
        return Err(Phase85ContractError::Kind);
    }
    if g32(b, 12) != PHASE85_AVIONICS_CONTRACT_ID {
        return Err(Phase85ContractError::Contract);
    }
    if b[20..32].iter().any(|v| *v != 0) {
        return Err(Phase85ContractError::Reserved);
    }
    let id = g32(b, 16);
    if id == 0 {
        return Err(Phase85ContractError::Identity);
    }
    let at = b.len() - 4;
    if g32(b, at) != crc32_ieee(&b[..at]) {
        return Err(Phase85ContractError::Checksum);
    }
    Ok(id)
}
fn reserved(b: &[u8], start: usize) -> Result<(), Phase85ContractError> {
    if b[start..b.len() - 4].iter().any(|v| *v != 0) {
        Err(Phase85ContractError::Reserved)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvionicsProfilePack {
    pub identity: u32,
    pub profile: AvionicsProfileId,
    pub frame: ReferenceFrameId,
    pub fast_hz: u8,
    pub navigation_hz: u8,
    pub guidance_hz: u8,
    pub flags: u8,
    pub sensor_flags: u16,
    pub minimum_arming_time_q18: i32,
    pub minimum_arming_altitude_q13: i32,
    pub drogue_backup_time_q18: i32,
    pub main_backup_time_q18: i32,
    pub main_altitude_q13: i32,
    pub minimum_deployment_separation_q18: i32,
    pub sensor_seed: u32,
    pub hold_epochs: u8,
    pub safe_epochs: u8,
    pub barometer_delay_epochs: u16,
    pub gps_delay_epochs: u16,
}
impl AvionicsProfilePack {
    pub const fn is_valid(self) -> bool {
        self.identity != 0
            && self.frame.is_supported()
            && self.fast_hz == 32
            && self.navigation_hz == 8
            && self.guidance_hz == 1
            && self.minimum_arming_time_q18 >= 0
            && self.minimum_arming_altitude_q13 >= 0
            && self.drogue_backup_time_q18 > self.minimum_arming_time_q18
            && self.main_backup_time_q18 > self.drogue_backup_time_q18
            && self.main_altitude_q13 > 0
            && self.minimum_deployment_separation_q18 > 0
            && self.hold_epochs == 2
            && self.safe_epochs == 3
            && self.barometer_delay_epochs <= 32
            && self.gps_delay_epochs <= 32
    }
}
pub fn write_avionics_profile(
    v: AvionicsProfilePack,
    o: &mut [u8],
) -> Result<(), Phase85ContractError> {
    if !v.is_valid() {
        return Err(Phase85ContractError::Range);
    }
    write_header(o, RecordKind::Avionics, v.identity)?;
    o[32] = v.profile as u8;
    o[33] = v.frame as u8;
    o[34] = v.fast_hz;
    o[35] = v.navigation_hz;
    o[36] = v.guidance_hz;
    o[37] = v.flags;
    p16(o, 38, v.sensor_flags);
    pi32(o, 40, v.minimum_arming_time_q18);
    pi32(o, 44, v.minimum_arming_altitude_q13);
    pi32(o, 48, v.drogue_backup_time_q18);
    pi32(o, 52, v.main_backup_time_q18);
    pi32(o, 56, v.main_altitude_q13);
    pi32(o, 60, v.minimum_deployment_separation_q18);
    p32(o, 64, v.sensor_seed);
    o[68] = v.hold_epochs;
    o[69] = v.safe_epochs;
    p16(o, 70, v.barometer_delay_epochs);
    p16(o, 72, v.gps_delay_epochs);
    seal(o);
    Ok(())
}
pub fn parse_avionics_profile(b: &[u8]) -> Result<AvionicsProfilePack, Phase85ContractError> {
    let identity = validate(b, RecordKind::Avionics)?;
    reserved(b, 74)?;
    let v = AvionicsProfilePack {
        identity,
        profile: AvionicsProfileId::parse(b[32])?,
        frame: ReferenceFrameId::parse(b[33])?,
        fast_hz: b[34],
        navigation_hz: b[35],
        guidance_hz: b[36],
        flags: b[37],
        sensor_flags: g16(b, 38),
        minimum_arming_time_q18: gi32(b, 40),
        minimum_arming_altitude_q13: gi32(b, 44),
        drogue_backup_time_q18: gi32(b, 48),
        main_backup_time_q18: gi32(b, 52),
        main_altitude_q13: gi32(b, 56),
        minimum_deployment_separation_q18: gi32(b, 60),
        sensor_seed: g32(b, 64),
        hold_epochs: b[68],
        safe_epochs: b[69],
        barometer_delay_epochs: g16(b, 70),
        gps_delay_epochs: g16(b, 72),
    };
    if !v.is_valid() {
        return Err(if v.frame.is_supported() {
            Phase85ContractError::Range
        } else {
            Phase85ContractError::Unsupported
        });
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActuatorCapabilityPack {
    pub identity: u32,
    pub capability: ActuatorCapabilityId,
    pub flags: u8,
    pub lag_releases: u8,
    pub vehicle_identity: u32,
    pub gimbal_limit_q16_deg: i32,
    pub slew_q16_deg_per_s: i32,
    pub pivot_from_nose_q28: i32,
    pub actuator_mass_q21: i32,
    pub proportional_gain_q15: i32,
    pub derivative_gain_q15: i32,
}
impl ActuatorCapabilityPack {
    pub const fn is_valid(self) -> bool {
        if self.identity == 0 || self.vehicle_identity == 0 {
            return false;
        }
        match self.capability {
            ActuatorCapabilityId::MonitorOnlyV1 => {
                self.gimbal_limit_q16_deg == 0
                    && self.slew_q16_deg_per_s == 0
                    && self.lag_releases == 0
            }
            ActuatorCapabilityId::TwoAxisMotorGimbalV1 => {
                self.gimbal_limit_q16_deg > 0
                    && self.gimbal_limit_q16_deg <= 10 * 65536
                    && self.slew_q16_deg_per_s > 0
                    && self.slew_q16_deg_per_s <= 90 * 65536
                    && self.lag_releases <= 8
                    && self.pivot_from_nose_q28 > 0
                    && self.actuator_mass_q21 > 0
                    && self.proportional_gain_q15 >= 0
                    && self.derivative_gain_q15 >= 0
            }
        }
    }
}
pub fn write_actuator_capability(
    v: ActuatorCapabilityPack,
    o: &mut [u8],
) -> Result<(), Phase85ContractError> {
    if !v.is_valid() {
        return Err(Phase85ContractError::Range);
    }
    write_header(o, RecordKind::Capability, v.identity)?;
    o[32] = v.capability as u8;
    o[33] = v.flags;
    o[34] = v.lag_releases;
    p32(o, 36, v.vehicle_identity);
    pi32(o, 40, v.gimbal_limit_q16_deg);
    pi32(o, 44, v.slew_q16_deg_per_s);
    pi32(o, 48, v.pivot_from_nose_q28);
    pi32(o, 52, v.actuator_mass_q21);
    pi32(o, 56, v.proportional_gain_q15);
    pi32(o, 60, v.derivative_gain_q15);
    seal(o);
    Ok(())
}
pub fn parse_actuator_capability(b: &[u8]) -> Result<ActuatorCapabilityPack, Phase85ContractError> {
    let identity = validate(b, RecordKind::Capability)?;
    if b[35] != 0 {
        return Err(Phase85ContractError::Reserved);
    }
    reserved(b, 64)?;
    let v = ActuatorCapabilityPack {
        identity,
        capability: ActuatorCapabilityId::parse(b[32])?,
        flags: b[33],
        lag_releases: b[34],
        vehicle_identity: g32(b, 36),
        gimbal_limit_q16_deg: gi32(b, 40),
        slew_q16_deg_per_s: gi32(b, 44),
        pivot_from_nose_q28: gi32(b, 48),
        actuator_mass_q21: gi32(b, 52),
        proportional_gain_q15: gi32(b, 56),
        derivative_gain_q15: gi32(b, 60),
    };
    if !v.is_valid() {
        return Err(Phase85ContractError::Range);
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvionicsEvaluationRequest {
    pub identity: u32,
    pub model_profile: u8,
    pub frame: ReferenceFrameId,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub environment_identity: u32,
    pub avionics_identity: u32,
    pub actuator_identity: u32,
    pub uncertainty_identity: u32,
    pub evaluator_identity: u32,
}
impl AvionicsEvaluationRequest {
    pub const fn is_valid(self) -> bool {
        self.identity != 0
            && self.model_profile == 4
            && self.frame.is_supported()
            && self.vehicle_identity != 0
            && self.motor_identity != 0
            && self.mission_identity != 0
            && self.environment_identity != 0
            && self.avionics_identity != 0
            && self.actuator_identity != 0
            && self.evaluator_identity != 0
    }
}
pub fn write_evaluation_request(
    v: AvionicsEvaluationRequest,
    o: &mut [u8],
) -> Result<(), Phase85ContractError> {
    if !v.is_valid() {
        return Err(Phase85ContractError::Range);
    }
    write_header(o, RecordKind::Evaluation, v.identity)?;
    o[32] = v.model_profile;
    o[33] = v.frame as u8;
    p32(o, 36, v.vehicle_identity);
    p32(o, 40, v.motor_identity);
    p32(o, 44, v.mission_identity);
    p32(o, 48, v.environment_identity);
    p32(o, 52, v.avionics_identity);
    p32(o, 56, v.actuator_identity);
    p32(o, 60, v.uncertainty_identity);
    p32(o, 64, v.evaluator_identity);
    seal(o);
    Ok(())
}
pub fn parse_evaluation_request(
    b: &[u8],
) -> Result<AvionicsEvaluationRequest, Phase85ContractError> {
    let identity = validate(b, RecordKind::Evaluation)?;
    if b[34] != 0 || b[35] != 0 {
        return Err(Phase85ContractError::Reserved);
    }
    reserved(b, 68)?;
    let v = AvionicsEvaluationRequest {
        identity,
        model_profile: b[32],
        frame: ReferenceFrameId::parse(b[33])?,
        vehicle_identity: g32(b, 36),
        motor_identity: g32(b, 40),
        mission_identity: g32(b, 44),
        environment_identity: g32(b, 48),
        avionics_identity: g32(b, 52),
        actuator_identity: g32(b, 56),
        uncertainty_identity: g32(b, 60),
        evaluator_identity: g32(b, 64),
    };
    if !v.is_valid() {
        return Err(if v.frame.is_supported() {
            Phase85ContractError::Range
        } else {
            Phase85ContractError::Unsupported
        });
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn av() -> AvionicsProfilePack {
        AvionicsProfilePack {
            identity: 1,
            profile: AvionicsProfileId::LocalEnuRecoveryV1,
            frame: ReferenceFrameId::LocalEnuV1,
            fast_hz: 32,
            navigation_hz: 8,
            guidance_hz: 1,
            flags: 0,
            sensor_flags: 0,
            minimum_arming_time_q18: 2 << 18,
            minimum_arming_altitude_q13: 20 << 13,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            sensor_seed: 7,
            hold_epochs: 2,
            safe_epochs: 3,
            barometer_delay_epochs: 0,
            gps_delay_epochs: 0,
        }
    }
    #[test]
    fn packs_round_trip_and_corruption_fails() {
        let a = av();
        let mut ab = [0; KAP8_LENGTH];
        write_avionics_profile(a, &mut ab).unwrap();
        assert_eq!(parse_avionics_profile(&ab), Ok(a));
        ab[80] = 1;
        assert_eq!(
            parse_avionics_profile(&ab),
            Err(Phase85ContractError::Checksum)
        );
        let c = ActuatorCapabilityPack {
            identity: 2,
            capability: ActuatorCapabilityId::TwoAxisMotorGimbalV1,
            flags: 0,
            lag_releases: 2,
            vehicle_identity: 3,
            gimbal_limit_q16_deg: 5 * 65536,
            slew_q16_deg_per_s: 30 * 65536,
            pivot_from_nose_q28: 500_000_000,
            actuator_mass_q21: 314_573,
            proportional_gain_q15: 8192,
            derivative_gain_q15: 4096,
        };
        let mut cb = [0; KAC8_LENGTH];
        write_actuator_capability(c, &mut cb).unwrap();
        assert_eq!(parse_actuator_capability(&cb), Ok(c));
        let e = AvionicsEvaluationRequest {
            identity: 4,
            model_profile: 4,
            frame: ReferenceFrameId::LocalEnuV1,
            vehicle_identity: 3,
            motor_identity: 5,
            mission_identity: 6,
            environment_identity: 7,
            avionics_identity: 1,
            actuator_identity: 2,
            uncertainty_identity: 0,
            evaluator_identity: 8,
        };
        let mut eb = [0; KLE8_LENGTH];
        write_evaluation_request(e, &mut eb).unwrap();
        assert_eq!(parse_evaluation_request(&eb), Ok(e));
    }
    #[test]
    fn reserved_global_frames_fail_closed() {
        let mut a = av();
        a.frame = ReferenceFrameId::EarthFixedEcefV1;
        assert!(!a.is_valid());
    }
}
