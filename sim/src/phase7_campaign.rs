//! Phase 7 design and uncertainty materialization over complete bounded packs.

use crate::phase4::campaign::keyed_word_raw;
use ksa64_core::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordError,
    Phase7RecordKind, KSC7_LENGTH,
};
use ksa64_core::phase7_numeric::{HOBBY_ALTITUDE_FRACTIONAL_BITS, HOBBY_TIME_FRACTIONAL_BITS};
use ksa64_core::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};
use ksa64_core::scenario::fnv1a_32;

pub const HOBBY_ROUTINE_RUNS: u32 = 64;
pub const HOBBY_REFERENCE_RUNS: u32 = 1_024;
pub const HOBBY_REFERENCE_SEED: u32 = 0x4b53_4137;
pub const HOBBY_PARAMETER_COUNT: usize = 8;
pub const HOBBY_CATALOG_ID: u32 = 0x0700_0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HobbyParameterId {
    VehicleDryMassPpm = 0,
    MotorPerformancePpm = 1,
    BodyDragPpm = 2,
    DrogueCdaPpm = 3,
    MainCdaPpm = 4,
    MainDeploymentAltitude = 5,
    RailLength = 6,
    RecoveryDelay = 7,
}

impl HobbyParameterId {
    pub const ALL: [Self; HOBBY_PARAMETER_COUNT] = [
        Self::VehicleDryMassPpm,
        Self::MotorPerformancePpm,
        Self::BodyDragPpm,
        Self::DrogueCdaPpm,
        Self::MainCdaPpm,
        Self::MainDeploymentAltitude,
        Self::RailLength,
        Self::RecoveryDelay,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyRange {
    pub minimum: i32,
    pub maximum: i32,
}

pub const HOBBY_UNCERTAINTY_CATALOG: [HobbyRange; HOBBY_PARAMETER_COUNT] = [
    HobbyRange {
        minimum: -30_000,
        maximum: 30_000,
    },
    HobbyRange {
        minimum: -50_000,
        maximum: 50_000,
    },
    HobbyRange {
        minimum: -100_000,
        maximum: 100_000,
    },
    HobbyRange {
        minimum: -100_000,
        maximum: 100_000,
    },
    HobbyRange {
        minimum: -100_000,
        maximum: 100_000,
    },
    HobbyRange {
        minimum: -(20 << HOBBY_ALTITUDE_FRACTIONAL_BITS),
        maximum: 20 << HOBBY_ALTITUDE_FRACTIONAL_BITS,
    },
    HobbyRange {
        minimum: -(1 << (HOBBY_ALTITUDE_FRACTIONAL_BITS - 1)),
        maximum: 1 << (HOBBY_ALTITUDE_FRACTIONAL_BITS - 1),
    },
    HobbyRange {
        minimum: 0,
        maximum: 1 << (HOBBY_TIME_FRACTIONAL_BITS - 1),
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyCampaignConfig {
    pub master_seed: u32,
    pub run_count: u32,
}

impl HobbyCampaignConfig {
    pub const ROUTINE: Self = Self {
        master_seed: HOBBY_REFERENCE_SEED,
        run_count: HOBBY_ROUTINE_RUNS,
    };
    pub const REFERENCE: Self = Self {
        master_seed: HOBBY_REFERENCE_SEED,
        run_count: HOBBY_REFERENCE_RUNS,
    };
    pub const fn is_valid(self) -> bool {
        self.master_seed != 0 && self.run_count != 0 && self.run_count <= 65_535
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyUncertaintyVector {
    pub values: [i32; HOBBY_PARAMETER_COUNT],
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyDesignVector {
    pub dry_mass_scale_ppm: i32,
    pub body_drag_scale_ppm: i32,
    pub main_deployment_altitude_raw: i32,
    pub rail_length_raw: i32,
}

impl HobbyDesignVector {
    pub const NOMINAL: Self = Self {
        dry_mass_scale_ppm: 1_000_000,
        body_drag_scale_ppm: 1_000_000,
        main_deployment_altitude_raw: 0,
        rail_length_raw: 0,
    };
}

fn uniform(word: u32, range: HobbyRange) -> i32 {
    let span = (range.maximum as i64 - range.minimum as i64 + 1) as u64;
    range.minimum + (((word as u64 * span) >> 32) as i32)
}

pub fn derive_hobby_uncertainty(
    config: HobbyCampaignConfig,
    run_index: u32,
) -> HobbyUncertaintyVector {
    if run_index == 0 {
        return HobbyUncertaintyVector {
            values: [0; HOBBY_PARAMETER_COUNT],
            checksum: 0,
        };
    }
    let mut values = [0i32; HOBBY_PARAMETER_COUNT];
    let mut bytes = [0u8; HOBBY_PARAMETER_COUNT * 4];
    for (index, range) in HOBBY_UNCERTAINTY_CATALOG.iter().enumerate() {
        values[index] = uniform(
            keyed_word_raw(config.master_seed, run_index, index as u8, 0, 0),
            *range,
        );
        bytes[index * 4..index * 4 + 4].copy_from_slice(&values[index].to_le_bytes());
    }
    HobbyUncertaintyVector {
        values,
        checksum: ksa64_interface::crc32_ieee(&bytes),
    }
}

fn scale_ppm(value: i32, delta_ppm: i32) -> i32 {
    ((value as i64 * (1_000_000 + delta_ppm) as i64 + 500_000) / 1_000_000)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub fn materialize_design(
    mut vehicle: VerticalVehiclePack,
    mut mission: HobbyMissionPack,
    design: HobbyDesignVector,
) -> (VerticalVehiclePack, HobbyMissionPack) {
    vehicle.dry_mass = ksa64_core::phase7_numeric::HobbyMass::from_raw(scale_ppm(
        vehicle.dry_mass.raw(),
        design.dry_mass_scale_ppm - 1_000_000,
    ));
    vehicle.body_cd_q16 = scale_ppm(vehicle.body_cd_q16, design.body_drag_scale_ppm - 1_000_000);
    if design.main_deployment_altitude_raw > 0 {
        mission.main_deployment_altitude = ksa64_core::phase7_numeric::HobbyAltitude::from_raw(
            design.main_deployment_altitude_raw,
        );
    }
    if design.rail_length_raw > 0 {
        mission.rail_length =
            ksa64_core::phase7_numeric::HobbyAltitude::from_raw(design.rail_length_raw);
    }
    let mut identity_bytes = [0u8; 16];
    identity_bytes[0..4].copy_from_slice(&vehicle.identity.to_le_bytes());
    identity_bytes[4..8].copy_from_slice(&mission.identity.to_le_bytes());
    identity_bytes[8..12].copy_from_slice(&design.dry_mass_scale_ppm.to_le_bytes());
    identity_bytes[12..16].copy_from_slice(&design.body_drag_scale_ppm.to_le_bytes());
    vehicle.identity = fnv1a_32(&identity_bytes);
    mission.vehicle_identity = vehicle.identity;
    let mut mission_identity_bytes = [0u8; 24];
    mission_identity_bytes[..16].copy_from_slice(&identity_bytes);
    mission_identity_bytes[16..20]
        .copy_from_slice(&design.main_deployment_altitude_raw.to_le_bytes());
    mission_identity_bytes[20..24].copy_from_slice(&design.rail_length_raw.to_le_bytes());
    mission.identity = fnv1a_32(&mission_identity_bytes);
    (vehicle, mission)
}

pub fn materialize_uncertainty(
    mut vehicle: VerticalVehiclePack,
    mut motor: MotorPack,
    mut mission: HobbyMissionPack,
    variation: HobbyUncertaintyVector,
) -> (VerticalVehiclePack, MotorPack, HobbyMissionPack) {
    let value = |id: HobbyParameterId| variation.values[id as usize];
    vehicle.dry_mass = ksa64_core::phase7_numeric::HobbyMass::from_raw(scale_ppm(
        vehicle.dry_mass.raw(),
        value(HobbyParameterId::VehicleDryMassPpm),
    ));
    vehicle.body_cd_q16 = scale_ppm(vehicle.body_cd_q16, value(HobbyParameterId::BodyDragPpm));
    vehicle.drogue_cda = ksa64_core::phase7_numeric::HobbyRecoveryCda::from_raw(scale_ppm(
        vehicle.drogue_cda.raw(),
        value(HobbyParameterId::DrogueCdaPpm),
    ));
    vehicle.main_cda = ksa64_core::phase7_numeric::HobbyRecoveryCda::from_raw(scale_ppm(
        vehicle.main_cda.raw(),
        value(HobbyParameterId::MainCdaPpm),
    ));
    let performance = value(HobbyParameterId::MotorPerformancePpm);
    motor.total_impulse_raw_q16 = scale_ppm(motor.total_impulse_raw_q16, performance);
    for knot in motor.knots.iter_mut().take(motor.knot_count as usize) {
        knot.thrust_raw_q13 = scale_ppm(knot.thrust_raw_q13, performance);
    }
    mission.main_deployment_altitude = ksa64_core::phase7_numeric::HobbyAltitude::from_raw(
        (mission.main_deployment_altitude.raw() + value(HobbyParameterId::MainDeploymentAltitude))
            .max(1),
    );
    mission.rail_length = ksa64_core::phase7_numeric::HobbyAltitude::from_raw(
        (mission.rail_length.raw() + value(HobbyParameterId::RailLength)).max(1),
    );
    mission.drogue_inflation_time = ksa64_core::phase7_numeric::HobbyTime::from_raw(
        mission.drogue_inflation_time.raw() + value(HobbyParameterId::RecoveryDelay),
    );
    mission.main_inflation_time = ksa64_core::phase7_numeric::HobbyTime::from_raw(
        mission.main_inflation_time.raw() + value(HobbyParameterId::RecoveryDelay),
    );
    (vehicle, motor, mission)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ksc7Error {
    Record(Phase7RecordError),
    Config,
    Catalog,
    Reserved,
}

impl From<Phase7RecordError> for Ksc7Error {
    fn from(value: Phase7RecordError) -> Self {
        Self::Record(value)
    }
}

fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

pub fn encode_ksc7(
    config: HobbyCampaignConfig,
    output: &mut [u8; KSC7_LENGTH],
) -> Result<(), Ksc7Error> {
    if !config.is_valid() {
        return Err(Ksc7Error::Config);
    }
    write_phase7_header(output, Phase7RecordKind::Campaign, HOBBY_CATALOG_ID)?;
    wu32(output, 32, config.master_seed);
    wu32(output, 36, config.run_count);
    wu32(output, 40, HOBBY_CATALOG_ID);
    output[44] = HOBBY_PARAMETER_COUNT as u8;
    for (index, range) in HOBBY_UNCERTAINTY_CATALOG.iter().enumerate() {
        let offset = 48 + index * 12;
        output[offset] = index as u8;
        output[offset + 1] = 1;
        w32(output, offset + 4, range.minimum);
        w32(output, offset + 8, range.maximum);
    }
    seal_phase7_record(output)?;
    Ok(())
}

pub fn parse_ksc7(input: &[u8]) -> Result<HobbyCampaignConfig, Ksc7Error> {
    let header = validate_phase7_record(input, Phase7RecordKind::Campaign)?;
    if header.identity != HOBBY_CATALOG_ID
        || ru32(input, 40) != HOBBY_CATALOG_ID
        || input[44] as usize != HOBBY_PARAMETER_COUNT
    {
        return Err(Ksc7Error::Catalog);
    }
    if input[45..48].iter().any(|value| *value != 0)
        || input[48 + HOBBY_PARAMETER_COUNT * 12..KSC7_LENGTH - 4]
            .iter()
            .any(|value| *value != 0)
    {
        return Err(Ksc7Error::Reserved);
    }
    for (index, range) in HOBBY_UNCERTAINTY_CATALOG.iter().enumerate() {
        let offset = 48 + index * 12;
        if input[offset] != index as u8
            || input[offset + 1] != 1
            || input[offset + 2] != 0
            || input[offset + 3] != 0
            || i32::from_le_bytes(input[offset + 4..offset + 8].try_into().unwrap())
                != range.minimum
            || i32::from_le_bytes(input[offset + 8..offset + 12].try_into().unwrap())
                != range.maximum
        {
            return Err(Ksc7Error::Catalog);
        }
    }
    let config = HobbyCampaignConfig {
        master_seed: ru32(input, 32),
        run_count: ru32(input, 36),
    };
    if !config.is_valid() {
        return Err(Ksc7Error::Config);
    }
    Ok(config)
}
