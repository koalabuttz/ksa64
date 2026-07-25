//! Deterministic keyed Phase 8 spatial uncertainty campaigns.

use crate::phase4::campaign::keyed_word_raw;
use ksa64_core::numeric::{add, multiply_scaled, NumericStatus};
use ksa64_core::phase2_numeric::sin_cos_binary_q15;
use ksa64_core::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordError,
    Phase8RecordKind, KSC8_LENGTH, KSC8_MAX_DISTRIBUTIONS, KWP8_MAX_WIND_KNOTS,
};
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_numeric::{SpatialAngle, SpatialPosition, SpatialWind};
use ksa64_core::phase8_pack::{SpatialMissionPack, WindKnot, WindProfilePack};
use ksa64_core::scenario::{crc32_ieee, fnv1a_32};

pub const SPATIAL_ROUTINE_RUNS: u32 = 64;
pub const SPATIAL_REFERENCE_RUNS: u32 = 1_024;
pub const SPATIAL_REFERENCE_SEED: u32 = 0x4b53_4138;
pub const SPATIAL_CATALOG_ID: u32 = 0x0800_0001;
pub const SPATIAL_PARAMETER_COUNT: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpatialParameterId {
    MassPpm = 0,
    ThrustPpm = 1,
    AxialDragPpm = 2,
    NormalForcePpm = 3,
    CpOffset = 4,
    DensityPpm = 5,
    WindSpeed = 6,
    WindDirection = 7,
    GustAmplitude = 8,
    LaunchAzimuth = 9,
    RecoveryCdaPpm = 10,
    InflationPpm = 11,
}
impl SpatialParameterId {
    pub const ALL: [Self; SPATIAL_PARAMETER_COUNT] = [
        Self::MassPpm,
        Self::ThrustPpm,
        Self::AxialDragPpm,
        Self::NormalForcePpm,
        Self::CpOffset,
        Self::DensityPpm,
        Self::WindSpeed,
        Self::WindDirection,
        Self::GustAmplitude,
        Self::LaunchAzimuth,
        Self::RecoveryCdaPpm,
        Self::InflationPpm,
    ];
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialRange {
    pub minimum: i32,
    pub maximum: i32,
}
pub const SPATIAL_CATALOG: [SpatialRange; SPATIAL_PARAMETER_COUNT] = [
    SpatialRange {
        minimum: -10_000,
        maximum: 10_000,
    },
    SpatialRange {
        minimum: -20_000,
        maximum: 20_000,
    },
    SpatialRange {
        minimum: -50_000,
        maximum: 50_000,
    },
    SpatialRange {
        minimum: -50_000,
        maximum: 50_000,
    },
    SpatialRange {
        minimum: -1_449_552,
        maximum: 1_449_552,
    },
    SpatialRange {
        minimum: -20_000,
        maximum: 20_000,
    },
    SpatialRange {
        minimum: 0,
        maximum: 2 << 22,
    },
    SpatialRange {
        minimum: 0,
        maximum: 65_535,
    },
    SpatialRange {
        minimum: 0,
        maximum: 1 << 20,
    },
    SpatialRange {
        minimum: -2_342_321,
        maximum: 2_342_321,
    },
    SpatialRange {
        minimum: -50_000,
        maximum: 50_000,
    },
    SpatialRange {
        minimum: -50_000,
        maximum: 50_000,
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialCampaignConfig {
    pub master_seed: u32,
    pub run_count: u32,
}
impl SpatialCampaignConfig {
    pub const ROUTINE: Self = Self {
        master_seed: SPATIAL_REFERENCE_SEED,
        run_count: SPATIAL_ROUTINE_RUNS,
    };
    pub const REFERENCE: Self = Self {
        master_seed: SPATIAL_REFERENCE_SEED,
        run_count: SPATIAL_REFERENCE_RUNS,
    };
    pub const fn is_valid(self) -> bool {
        self.master_seed != 0 && self.run_count > 0 && self.run_count <= 65_535
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialUncertaintyVector {
    pub values: [i32; SPATIAL_PARAMETER_COUNT],
    pub checksum: u32,
}
fn uniform(word: u32, range: SpatialRange) -> i32 {
    let span = (range.maximum as i64 - range.minimum as i64 + 1) as u64;
    range.minimum + (((word as u64 * span) >> 32) as i32)
}
pub fn derive_spatial_uncertainty(
    config: SpatialCampaignConfig,
    run_index: u32,
) -> SpatialUncertaintyVector {
    if run_index == 0 {
        return SpatialUncertaintyVector {
            values: [0; SPATIAL_PARAMETER_COUNT],
            checksum: 0,
        };
    }
    let mut values = [0i32; SPATIAL_PARAMETER_COUNT];
    let mut bytes = [0u8; SPATIAL_PARAMETER_COUNT * 4];
    for (index, range) in SPATIAL_CATALOG.iter().enumerate() {
        values[index] = uniform(
            keyed_word_raw(config.master_seed, run_index, index as u8, 0, 0),
            *range,
        );
        bytes[index * 4..index * 4 + 4].copy_from_slice(&values[index].to_le_bytes());
    }
    SpatialUncertaintyVector {
        values,
        checksum: crc32_ieee(&bytes),
    }
}
fn value(vector: SpatialUncertaintyVector, id: SpatialParameterId) -> i32 {
    vector.values[id as usize]
}
pub fn materialize_spatial_case(
    mut mission: SpatialMissionPack,
    base_wind: &WindProfilePack,
    vector: SpatialUncertaintyVector,
    run_index: u32,
) -> (SpatialMissionPack, WindProfilePack, SpatialMissionVariation) {
    let speed = value(vector, SpatialParameterId::WindSpeed);
    let direction = value(vector, SpatialParameterId::WindDirection) as u16;
    let (sine, cosine) = sin_cos_binary_q15(direction);
    let mut status = NumericStatus::CLEAR;
    let east = multiply_scaled(speed, sine as i32, 15, &mut status);
    let north = multiply_scaled(speed, cosine as i32, 15, &mut status);
    let gust = value(vector, SpatialParameterId::GustAmplitude);
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    knots[0] = WindKnot {
        altitude: SpatialPosition::ZERO,
        east: SpatialWind::from_raw(east),
        north: SpatialWind::from_raw(north),
    };
    knots[1] = WindKnot {
        altitude: SpatialPosition::from_raw(100_000 << 13),
        east: SpatialWind::from_raw(east),
        north: SpatialWind::from_raw(north),
    };
    let mut identity_bytes = [0u8; 12];
    identity_bytes[0..4].copy_from_slice(&base_wind.identity.to_le_bytes());
    identity_bytes[4..8].copy_from_slice(&vector.checksum.to_le_bytes());
    identity_bytes[8..12].copy_from_slice(&run_index.to_le_bytes());
    let wind_identity = fnv1a_32(&identity_bytes);
    let wind = WindProfilePack {
        identity: wind_identity,
        gust_seed: base_wind.gust_seed ^ run_index,
        gust_cadence: base_wind.gust_cadence,
        gust_amplitude_east: SpatialWind::from_raw(gust),
        gust_amplitude_north: SpatialWind::from_raw(gust),
        max_gust: SpatialWind::from_raw(gust.saturating_mul(2)),
        knot_count: 2,
        knots,
    };
    mission.wind_identity = wind.identity;
    mission.case_seed ^= run_index;
    mission.launch_azimuth = SpatialAngle::from_raw(add(
        mission.launch_azimuth.raw(),
        value(vector, SpatialParameterId::LaunchAzimuth),
        &mut status,
    ));
    let variation = SpatialMissionVariation {
        mass_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::MassPpm),
        thrust_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::ThrustPpm),
        axial_drag_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::AxialDragPpm),
        normal_force_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::NormalForcePpm),
        cp_offset_q28: value(vector, SpatialParameterId::CpOffset),
        density_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::DensityPpm),
        wind_scale_ppm: 1_000_000,
        recovery_cda_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::RecoveryCdaPpm),
        inflation_scale_ppm: 1_000_000 + value(vector, SpatialParameterId::InflationPpm),
    };
    (mission, wind, variation)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ksc8Error {
    Record(Phase8RecordError),
    Config,
    Catalog,
    Reserved,
}
impl From<Phase8RecordError> for Ksc8Error {
    fn from(value: Phase8RecordError) -> Self {
        Self::Record(value)
    }
}
fn w32(o: &mut [u8], p: usize, v: i32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn wu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn r32(i: &[u8], p: usize) -> i32 {
    i32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
fn ru32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
pub fn encode_ksc8(
    config: SpatialCampaignConfig,
    output: &mut [u8; KSC8_LENGTH],
) -> Result<(), Ksc8Error> {
    if !config.is_valid() {
        return Err(Ksc8Error::Config);
    };
    write_phase8_header(output, Phase8RecordKind::Campaign, SPATIAL_CATALOG_ID)?;
    wu32(output, 32, config.master_seed);
    wu32(output, 36, config.run_count);
    wu32(output, 40, SPATIAL_CATALOG_ID);
    output[44] = SPATIAL_PARAMETER_COUNT as u8;
    for (index, range) in SPATIAL_CATALOG.iter().enumerate() {
        let offset = 48 + index * 16;
        output[offset] = index as u8;
        output[offset + 1] = 1;
        w32(output, offset + 4, range.minimum);
        w32(output, offset + 8, range.maximum);
    }
    seal_phase8_record(output)?;
    Ok(())
}
pub fn parse_ksc8(input: &[u8]) -> Result<SpatialCampaignConfig, Ksc8Error> {
    validate_phase8_record(input, Phase8RecordKind::Campaign)?;
    if ru32(input, 40) != SPATIAL_CATALOG_ID || input[44] as usize != SPATIAL_PARAMETER_COUNT {
        return Err(Ksc8Error::Catalog);
    };
    for (index, range) in SPATIAL_CATALOG.iter().enumerate() {
        let offset = 48 + index * 16;
        if input[offset] != index as u8
            || input[offset + 1] != 1
            || input[offset + 2..offset + 4].iter().any(|v| *v != 0)
            || r32(input, offset + 4) != range.minimum
            || r32(input, offset + 8) != range.maximum
            || input[offset + 12..offset + 16].iter().any(|v| *v != 0)
        {
            return Err(Ksc8Error::Catalog);
        }
    }
    let used = 48 + SPATIAL_PARAMETER_COUNT * 16;
    if input[45..48].iter().any(|v| *v != 0) || input[used..KSC8_LENGTH - 4].iter().any(|v| *v != 0)
    {
        return Err(Ksc8Error::Reserved);
    };
    let config = SpatialCampaignConfig {
        master_seed: ru32(input, 32),
        run_count: ru32(input, 36),
    };
    if !config.is_valid() {
        return Err(Ksc8Error::Config);
    };
    Ok(config)
}
const _: () = assert!(SPATIAL_PARAMETER_COUNT <= KSC8_MAX_DISTRIBUTIONS);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_zero_and_ksc_round_trip() {
        assert_eq!(
            derive_spatial_uncertainty(SpatialCampaignConfig::ROUTINE, 0).checksum,
            0
        );
        assert_eq!(
            derive_spatial_uncertainty(SpatialCampaignConfig::ROUTINE, 1),
            derive_spatial_uncertainty(SpatialCampaignConfig::ROUTINE, 1)
        );
        let mut bytes = [0u8; KSC8_LENGTH];
        encode_ksc8(SpatialCampaignConfig::REFERENCE, &mut bytes).unwrap();
        assert_eq!(
            parse_ksc8(&bytes).unwrap(),
            SpatialCampaignConfig::REFERENCE
        );
        bytes[99] ^= 1;
        assert!(parse_ksc8(&bytes).is_err());
    }
}
