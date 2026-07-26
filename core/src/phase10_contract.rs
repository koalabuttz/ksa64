//! Strict Earth and transform contracts for Phase 10.

use crate::phase10_numeric::{GlobalAngularRateVec, MissionTimeQ16, GLOBAL_AVIONICS_PERIOD_Q16};
use crate::scenario::crc32_ieee;
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

pub use crate::phase8_5_contract::ReferenceFrameId;

pub const PHASE10_CONTRACT_ID: u32 = 0x10e0_0001;
pub const KEM10_LENGTH: usize = 512;
pub const KFT10_HEADER_LENGTH: usize = 128;
pub const KFT10_KNOT_LENGTH: usize = 48;
pub const KFT10_MAX_KNOTS: usize = 128;
pub const KFT10_LENGTH: usize = KFT10_HEADER_LENGTH + KFT10_KNOT_LENGTH * KFT10_MAX_KNOTS + 4;
const VERSION: u16 = 10;
const HEADER_LENGTH: usize = 32;

pub const WGS84_SEMI_MAJOR_Q12_KM: i32 = 26_124_849;
pub const WGS84_SEMI_MINOR_Q12_KM: i32 = 26_037_257;
pub const WGS84_INVERSE_FLATTENING_Q20: i32 = 312_745_366;
pub const WGS84_MU_Q8_KM3_S2: i32 = 102_041_713;
pub const WGS84_J2_Q30: i32 = 1_162_465;
pub const EARTH_ROTATION_Q30_RAD_S: i32 = 78_298;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GravityModelId {
    CentralJ2V1 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EarthOrientationModelId {
    Iers2010CompiledV1 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AtmosphereModelId {
    CompiledProfileV1 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalSegment {
    LocalLaunch = 1,
    EcefAscent = 2,
    EciCoast = 3,
    EcefEntry = 4,
    LocalRecovery = 5,
}

impl GlobalSegment {
    pub const fn frame(self) -> ReferenceFrameId {
        match self {
            Self::LocalLaunch | Self::LocalRecovery => ReferenceFrameId::LocalEnuV1,
            Self::EcefAscent | Self::EcefEntry => ReferenceFrameId::EarthFixedEcefV1,
            Self::EciCoast => ReferenceFrameId::EarthInertialEciV1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct LeapRecord {
    pub effective_unix_day: i32,
    pub tai_minus_utc_after: i16,
}

pub const KEM10_MAX_LEAPS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct EarthModelPack {
    pub identity: u32,
    pub gravity: GravityModelId,
    pub orientation: EarthOrientationModelId,
    pub atmosphere: AtmosphereModelId,
    pub semi_major_q12_km: i32,
    pub semi_minor_q12_km: i32,
    pub inverse_flattening_q20: i32,
    pub mu_q8_km3_s2: i32,
    pub j2_q30: i32,
    pub rotation_q30_rad_s: i32,
    pub epoch_unix_day: i32,
    pub epoch_tai_minus_utc: i16,
    pub eop_start_unix_day: i32,
    pub eop_end_unix_day: i32,
    pub leap_source_hash: u32,
    pub eop_source_hash: u32,
    pub convention_hash: u32,
    pub leap_count: u8,
    pub initial_tai_minus_utc: i16,
    pub leaps: [LeapRecord; KEM10_MAX_LEAPS],
}

impl EarthModelPack {
    pub fn validate(&self) -> Result<(), Phase10ContractError> {
        if self.identity == 0
            || self.gravity != GravityModelId::CentralJ2V1
            || self.orientation != EarthOrientationModelId::Iers2010CompiledV1
            || self.atmosphere != AtmosphereModelId::CompiledProfileV1
            || self.semi_major_q12_km != WGS84_SEMI_MAJOR_Q12_KM
            || self.semi_minor_q12_km != WGS84_SEMI_MINOR_Q12_KM
            || self.inverse_flattening_q20 != WGS84_INVERSE_FLATTENING_Q20
            || self.mu_q8_km3_s2 != WGS84_MU_Q8_KM3_S2
            || self.j2_q30 != WGS84_J2_Q30
            || self.rotation_q30_rad_s != EARTH_ROTATION_Q30_RAD_S
            || self.epoch_tai_minus_utc != 37
            || self.eop_start_unix_day > self.epoch_unix_day
            || self.eop_end_unix_day < self.epoch_unix_day
            || self.leap_source_hash == 0
            || self.eop_source_hash == 0
            || self.convention_hash == 0
            || self.leap_count as usize > KEM10_MAX_LEAPS
        {
            return Err(Phase10ContractError::Range);
        }
        let mut previous_day = i32::MIN;
        let mut previous_offset = self.initial_tai_minus_utc;
        for leap in &self.leaps[..self.leap_count as usize] {
            if leap.effective_unix_day <= previous_day
                || leap.tai_minus_utc_after != previous_offset + 1
            {
                return Err(Phase10ContractError::Range);
            }
            previous_day = leap.effective_unix_day;
            previous_offset = leap.tai_minus_utc_after;
        }
        if self.leaps[self.leap_count as usize..]
            .iter()
            .any(|entry| *entry != LeapRecord::default())
        {
            return Err(Phase10ContractError::Reserved);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), Phase10ContractError> {
        self.validate()?;
        write_header(output, RecordKind::Earth, self.identity)?;
        output[32] = self.gravity as u8;
        output[33] = self.orientation as u8;
        output[34] = self.atmosphere as u8;
        pi32(output, 36, self.semi_major_q12_km);
        pi32(output, 40, self.semi_minor_q12_km);
        pi32(output, 44, self.inverse_flattening_q20);
        pi32(output, 48, self.mu_q8_km3_s2);
        pi32(output, 52, self.j2_q30);
        pi32(output, 56, self.rotation_q30_rad_s);
        pi32(output, 60, self.epoch_unix_day);
        pi16(output, 64, self.epoch_tai_minus_utc);
        pi32(output, 68, self.eop_start_unix_day);
        pi32(output, 72, self.eop_end_unix_day);
        p32(output, 76, self.leap_source_hash);
        p32(output, 80, self.eop_source_hash);
        p32(output, 84, self.convention_hash);
        output[88] = self.leap_count;
        pi16(output, 90, self.initial_tai_minus_utc);
        for (index, leap) in self.leaps.iter().enumerate() {
            let at = 96 + index * 8;
            pi32(output, at, leap.effective_unix_day);
            pi16(output, at + 4, leap.tai_minus_utc_after);
        }
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Phase10ContractError> {
        let identity = validate_record(bytes, RecordKind::Earth)?;
        if bytes[35] != 0
            || bytes[65..68].iter().any(|byte| *byte != 0)
            || bytes[89] != 0
            || bytes[92..96].iter().any(|byte| *byte != 0)
            || bytes[352..KEM10_LENGTH - 4].iter().any(|byte| *byte != 0)
        {
            return Err(Phase10ContractError::Reserved);
        }
        let gravity = match bytes[32] {
            1 => GravityModelId::CentralJ2V1,
            _ => return Err(Phase10ContractError::Enum),
        };
        let orientation = match bytes[33] {
            1 => EarthOrientationModelId::Iers2010CompiledV1,
            _ => return Err(Phase10ContractError::Enum),
        };
        let atmosphere = match bytes[34] {
            1 => AtmosphereModelId::CompiledProfileV1,
            _ => return Err(Phase10ContractError::Enum),
        };
        let leap_count = bytes[88];
        if leap_count as usize > KEM10_MAX_LEAPS {
            return Err(Phase10ContractError::Range);
        }
        let mut leaps = [LeapRecord::default(); KEM10_MAX_LEAPS];
        for (index, leap) in leaps.iter_mut().enumerate() {
            let at = 96 + index * 8;
            if bytes[at + 6] != 0 || bytes[at + 7] != 0 {
                return Err(Phase10ContractError::Reserved);
            }
            leap.effective_unix_day = gi32(bytes, at);
            leap.tai_minus_utc_after = gi16(bytes, at + 4);
        }
        let pack = Self {
            identity,
            gravity,
            orientation,
            atmosphere,
            semi_major_q12_km: gi32(bytes, 36),
            semi_minor_q12_km: gi32(bytes, 40),
            inverse_flattening_q20: gi32(bytes, 44),
            mu_q8_km3_s2: gi32(bytes, 48),
            j2_q30: gi32(bytes, 52),
            rotation_q30_rad_s: gi32(bytes, 56),
            epoch_unix_day: gi32(bytes, 60),
            epoch_tai_minus_utc: gi16(bytes, 64),
            eop_start_unix_day: gi32(bytes, 68),
            eop_end_unix_day: gi32(bytes, 72),
            leap_source_hash: g32(bytes, 76),
            eop_source_hash: g32(bytes, 80),
            convention_hash: g32(bytes, 84),
            leap_count,
            initial_tai_minus_utc: gi16(bytes, 90),
            leaps,
        };
        pack.validate()?;
        Ok(pack)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TransformKnot {
    pub time: MissionTimeQ16,
    pub ecef_to_gcrf: QuaternionQ30,
    pub angular_velocity_gcrf: GlobalAngularRateVec,
    pub angular_acceleration_gcrf: FixedVec3<28>,
}

impl TransformKnot {
    pub const ZERO: Self = Self {
        time: MissionTimeQ16::ZERO,
        ecef_to_gcrf: QuaternionQ30::IDENTITY,
        angular_velocity_gcrf: GlobalAngularRateVec::ZERO,
        angular_acceleration_gcrf: FixedVec3::ZERO,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TransformPack {
    pub identity: u32,
    pub earth_identity: u32,
    pub knot_spacing_q16: u32,
    pub count: u8,
    pub knots: [TransformKnot; KFT10_MAX_KNOTS],
}

impl TransformPack {
    pub fn validate(&self) -> Result<(), Phase10ContractError> {
        if self.identity == 0
            || self.earth_identity == 0
            || self.count < 2
            || self.count as usize > KFT10_MAX_KNOTS
            || self.knot_spacing_q16 != 60 * 65_536
        {
            return Err(Phase10ContractError::Range);
        }
        let active = &self.knots[..self.count as usize];
        for (index, knot) in active.iter().enumerate() {
            if index > 0 && knot.time.raw() - active[index - 1].time.raw() != self.knot_spacing_q16
            {
                return Err(Phase10ContractError::Range);
            }
        }
        if self.knots[self.count as usize..]
            .iter()
            .any(|knot| *knot != TransformKnot::ZERO)
        {
            return Err(Phase10ContractError::Reserved);
        }
        Ok(())
    }

    pub fn covers(&self, time: MissionTimeQ16) -> bool {
        self.count >= 2
            && time.raw() >= self.knots[0].time.raw()
            && time.raw() <= self.knots[self.count as usize - 1].time.raw()
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), Phase10ContractError> {
        self.validate()?;
        write_header(output, RecordKind::Transforms, self.identity)?;
        p32(output, 32, self.earth_identity);
        p32(output, 36, self.knot_spacing_q16);
        output[40] = self.count;
        p32(output, 44, self.knots[0].time.raw());
        p32(output, 48, self.knots[self.count as usize - 1].time.raw());
        for (index, knot) in self.knots.iter().enumerate() {
            let at = KFT10_HEADER_LENGTH + index * KFT10_KNOT_LENGTH;
            p32(output, at, knot.time.raw());
            write_quaternion(output, at + 4, knot.ecef_to_gcrf);
            write_vec(output, at + 20, knot.angular_velocity_gcrf);
            write_vec(output, at + 32, knot.angular_acceleration_gcrf);
        }
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Phase10ContractError> {
        let identity = validate_record(bytes, RecordKind::Transforms)?;
        if bytes[41..44].iter().any(|byte| *byte != 0)
            || bytes[52..KFT10_HEADER_LENGTH].iter().any(|byte| *byte != 0)
        {
            return Err(Phase10ContractError::Reserved);
        }
        let count = bytes[40];
        if count < 2 || count as usize > KFT10_MAX_KNOTS {
            return Err(Phase10ContractError::Range);
        }
        let mut knots = [TransformKnot::ZERO; KFT10_MAX_KNOTS];
        for (index, knot) in knots.iter_mut().enumerate() {
            let at = KFT10_HEADER_LENGTH + index * KFT10_KNOT_LENGTH;
            if bytes[at + 44..at + 48].iter().any(|byte| *byte != 0) {
                return Err(Phase10ContractError::Reserved);
            }
            knot.time =
                MissionTimeQ16::from_raw(g32(bytes, at)).ok_or(Phase10ContractError::Range)?;
            knot.ecef_to_gcrf = read_quaternion(bytes, at + 4);
            knot.angular_velocity_gcrf = read_vec(bytes, at + 20);
            knot.angular_acceleration_gcrf = read_vec(bytes, at + 32);
        }
        let pack = Self {
            identity,
            earth_identity: g32(bytes, 32),
            knot_spacing_q16: g32(bytes, 36),
            count,
            knots,
        };
        if g32(bytes, 44) != pack.knots[0].time.raw()
            || g32(bytes, 48) != pack.knots[count as usize - 1].time.raw()
        {
            return Err(Phase10ContractError::Identity);
        }
        pack.validate()?;
        Ok(pack)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase10ContractError {
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
    Coverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
enum RecordKind {
    Earth = 1,
    Transforms = 2,
}

impl RecordKind {
    const fn magic(self) -> [u8; 5] {
        match self {
            Self::Earth => *b"KEM10",
            Self::Transforms => *b"KFT10",
        }
    }

    const fn length(self) -> usize {
        match self {
            Self::Earth => KEM10_LENGTH,
            Self::Transforms => KFT10_LENGTH,
        }
    }
}

fn p16(output: &mut [u8], at: usize, value: u16) {
    output[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn pi16(output: &mut [u8], at: usize, value: i16) {
    p16(output, at, value as u16);
}

fn p32(output: &mut [u8], at: usize, value: u32) {
    output[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn pi32(output: &mut [u8], at: usize, value: i32) {
    p32(output, at, value as u32);
}

fn g16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn gi16(bytes: &[u8], at: usize) -> i16 {
    g16(bytes, at) as i16
}

fn g32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn gi32(bytes: &[u8], at: usize) -> i32 {
    g32(bytes, at) as i32
}

fn write_header(
    output: &mut [u8],
    kind: RecordKind,
    identity: u32,
) -> Result<(), Phase10ContractError> {
    if output.len() != kind.length() {
        return Err(Phase10ContractError::Length);
    }
    if identity == 0 {
        return Err(Phase10ContractError::Identity);
    }
    output.fill(0);
    output[..5].copy_from_slice(&kind.magic());
    output[5] = 0;
    p16(output, 6, VERSION);
    p16(output, 8, HEADER_LENGTH as u16);
    p16(output, 10, kind as u16);
    p32(output, 12, output.len() as u32);
    p32(output, 16, PHASE10_CONTRACT_ID);
    p32(output, 20, identity);
    Ok(())
}

fn seal(output: &mut [u8]) {
    let at = output.len() - 4;
    let checksum = crc32_ieee(&output[..at]);
    p32(output, at, checksum);
}

fn validate_record(bytes: &[u8], kind: RecordKind) -> Result<u32, Phase10ContractError> {
    if bytes.len() != kind.length() {
        return Err(Phase10ContractError::Length);
    }
    if bytes[..5] != kind.magic() || bytes[5] != 0 {
        return Err(Phase10ContractError::Magic);
    }
    if g16(bytes, 6) != VERSION || g16(bytes, 8) as usize != HEADER_LENGTH {
        return Err(Phase10ContractError::Version);
    }
    if g16(bytes, 10) != kind as u16 || g32(bytes, 12) as usize != bytes.len() {
        return Err(Phase10ContractError::Kind);
    }
    if g32(bytes, 16) != PHASE10_CONTRACT_ID {
        return Err(Phase10ContractError::Contract);
    }
    let identity = g32(bytes, 20);
    if identity == 0 {
        return Err(Phase10ContractError::Identity);
    }
    if bytes[24..HEADER_LENGTH].iter().any(|byte| *byte != 0) {
        return Err(Phase10ContractError::Reserved);
    }
    let checksum_at = bytes.len() - 4;
    if g32(bytes, checksum_at) != crc32_ieee(&bytes[..checksum_at]) {
        return Err(Phase10ContractError::Checksum);
    }
    Ok(identity)
}

fn write_quaternion(output: &mut [u8], at: usize, value: QuaternionQ30) {
    pi32(output, at, value.w());
    pi32(output, at + 4, value.x());
    pi32(output, at + 8, value.y());
    pi32(output, at + 12, value.z());
}

fn read_quaternion(bytes: &[u8], at: usize) -> QuaternionQ30 {
    QuaternionQ30::new(
        gi32(bytes, at),
        gi32(bytes, at + 4),
        gi32(bytes, at + 8),
        gi32(bytes, at + 12),
    )
}

fn write_vec<const F: u8>(output: &mut [u8], at: usize, value: FixedVec3<F>) {
    pi32(output, at, value.x());
    pi32(output, at + 4, value.y());
    pi32(output, at + 8, value.z());
}

fn read_vec<const F: u8>(bytes: &[u8], at: usize) -> FixedVec3<F> {
    FixedVec3::new(gi32(bytes, at), gi32(bytes, at + 4), gi32(bytes, at + 8))
}

pub const fn phase10_contract_is_valid() -> bool {
    WGS84_SEMI_MAJOR_Q12_KM > WGS84_SEMI_MINOR_Q12_KM
        && WGS84_MU_Q8_KM3_S2 > 0
        && WGS84_J2_Q30 > 0
        && EARTH_ROTATION_Q30_RAD_S > 0
        && KFT10_LENGTH < u16::MAX as usize
        && GLOBAL_AVIONICS_PERIOD_Q16 == 2_048
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_pack() -> EarthModelPack {
        let mut leaps = [LeapRecord::default(); KEM10_MAX_LEAPS];
        leaps[0] = LeapRecord {
            effective_unix_day: 17_167,
            tai_minus_utc_after: 37,
        };
        EarthModelPack {
            identity: 0x1020_3040,
            gravity: GravityModelId::CentralJ2V1,
            orientation: EarthOrientationModelId::Iers2010CompiledV1,
            atmosphere: AtmosphereModelId::CompiledProfileV1,
            semi_major_q12_km: WGS84_SEMI_MAJOR_Q12_KM,
            semi_minor_q12_km: WGS84_SEMI_MINOR_Q12_KM,
            inverse_flattening_q20: WGS84_INVERSE_FLATTENING_Q20,
            mu_q8_km3_s2: WGS84_MU_Q8_KM3_S2,
            j2_q30: WGS84_J2_Q30,
            rotation_q30_rad_s: EARTH_ROTATION_Q30_RAD_S,
            epoch_unix_day: 19_723,
            epoch_tai_minus_utc: 37,
            eop_start_unix_day: 19_722,
            eop_end_unix_day: 19_725,
            leap_source_hash: 1,
            eop_source_hash: 2,
            convention_hash: 3,
            leap_count: 1,
            initial_tai_minus_utc: 36,
            leaps,
        }
    }

    #[test]
    fn earth_round_trip_and_corruption_are_strict() {
        assert!(phase10_contract_is_valid());
        let pack = earth_pack();
        let mut bytes = [0u8; KEM10_LENGTH];
        pack.encode(&mut bytes).unwrap();
        assert_eq!(EarthModelPack::decode(&bytes).unwrap(), pack);
        bytes[400] = 1;
        seal(&mut bytes);
        assert_eq!(
            EarthModelPack::decode(&bytes),
            Err(Phase10ContractError::Reserved)
        );
    }

    #[test]
    fn transform_table_requires_exact_sixty_second_knots() {
        let mut knots = [TransformKnot::ZERO; KFT10_MAX_KNOTS];
        knots[0] = TransformKnot::ZERO;
        knots[1] = TransformKnot {
            time: MissionTimeQ16::from_raw(60 * 65_536).unwrap(),
            ..TransformKnot::ZERO
        };
        let pack = TransformPack {
            identity: 7,
            earth_identity: 8,
            knot_spacing_q16: 60 * 65_536,
            count: 2,
            knots,
        };
        let mut bytes = [0u8; KFT10_LENGTH];
        pack.encode(&mut bytes).unwrap();
        assert_eq!(TransformPack::decode(&bytes).unwrap(), pack);
        assert!(pack.covers(MissionTimeQ16::from_raw(1).unwrap()));
        assert!(!pack.covers(MissionTimeQ16::from_raw(61 * 65_536).unwrap()));
    }
}
