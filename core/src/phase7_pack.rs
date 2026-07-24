//! Bounded Phase 7 vehicle, motor, and mission packs.

use crate::evaluation::ModelProfileId;
use crate::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordError,
    Phase7RecordKind, KMC7_LENGTH, KMP7_LENGTH, KMP7_MAX_KNOTS, KVP7_LENGTH,
};
use crate::phase7_numeric::{
    HobbyAltitude, HobbyArea, HobbyMass, HobbyRecoveryCda, HobbyTime, HobbyVelocity,
    HOBBY_ENVIRONMENT_ID, HOBBY_MAX_ALTITUDE_RAW, HOBBY_MAX_AREA_RAW, HOBBY_MAX_MASS_RAW,
    HOBBY_MAX_RECOVERY_CDA_RAW, HOBBY_MAX_TIME_RAW,
};

const VEHICLE_PAYLOAD_END: usize = 68;
const MOTOR_KNOT_BASE: usize = 64;
const MOTOR_KNOT_LENGTH: usize = 8;
const MISSION_PAYLOAD_END: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase7PackError {
    Record(Phase7RecordError),
    Profile,
    Identity,
    Range,
    Count,
    Ordering,
    Reserved,
    Reference,
}

impl From<Phase7RecordError> for Phase7PackError {
    fn from(value: Phase7RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalVehiclePack {
    pub identity: u32,
    pub dry_mass: HobbyMass,
    pub length: HobbyAltitude,
    pub diameter: HobbyAltitude,
    pub reference_area: HobbyArea,
    pub body_cd_q16: i32,
    pub drogue_cda: HobbyRecoveryCda,
    pub main_cda: HobbyRecoveryCda,
}

impl VerticalVehiclePack {
    pub const fn is_valid(self) -> bool {
        self.identity != 0
            && self.dry_mass.raw() > 0
            && self.dry_mass.raw() <= HOBBY_MAX_MASS_RAW
            && self.length.raw() > 0
            && self.length.raw() <= HOBBY_MAX_ALTITUDE_RAW
            && self.diameter.raw() > 0
            && self.reference_area.raw() > 0
            && self.reference_area.raw() <= HOBBY_MAX_AREA_RAW
            && self.body_cd_q16 > 0
            && self.drogue_cda.raw() > 0
            && self.drogue_cda.raw() <= HOBBY_MAX_RECOVERY_CDA_RAW
            && self.main_cda.raw() > 0
            && self.main_cda.raw() <= HOBBY_MAX_RECOVERY_CDA_RAW
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotorKnot {
    pub time: HobbyTime,
    pub thrust_raw_q13: i32,
}

impl MotorKnot {
    pub const ZERO: Self = Self {
        time: HobbyTime::ZERO,
        thrust_raw_q13: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotorPack {
    pub identity: u32,
    pub loaded_mass: HobbyMass,
    pub propellant_mass: HobbyMass,
    pub total_impulse_raw_q16: i32,
    pub burn_time: HobbyTime,
    pub knot_count: u8,
    pub knots: [MotorKnot; KMP7_MAX_KNOTS],
}

impl MotorPack {
    pub fn is_valid(&self) -> bool {
        if self.identity == 0
            || self.loaded_mass.raw() <= 0
            || self.loaded_mass.raw() > HOBBY_MAX_MASS_RAW
            || self.propellant_mass.raw() <= 0
            || self.propellant_mass.raw() >= self.loaded_mass.raw()
            || self.total_impulse_raw_q16 <= 0
            || self.burn_time.raw() <= 0
            || self.burn_time.raw() > HOBBY_MAX_TIME_RAW
            || self.knot_count < 2
            || self.knot_count as usize > KMP7_MAX_KNOTS
        {
            return false;
        }
        let mut index = 0usize;
        let mut previous = -1;
        while index < self.knot_count as usize {
            let knot = self.knots[index];
            if knot.time.raw() <= previous || knot.thrust_raw_q13 < 0 {
                return false;
            }
            previous = knot.time.raw();
            index += 1;
        }
        self.knots[self.knot_count as usize - 1].time == self.burn_time
            && self.knots[self.knot_count as usize - 1].thrust_raw_q13 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyMissionPack {
    pub identity: u32,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub environment_identity: u32,
    pub launch_altitude: HobbyAltitude,
    pub rail_length: HobbyAltitude,
    pub main_deployment_altitude: HobbyAltitude,
    pub drogue_inflation_time: HobbyTime,
    pub main_inflation_time: HobbyTime,
    pub max_mission_time: HobbyTime,
    pub telemetry_period: HobbyTime,
    pub minimum_rail_exit_velocity: HobbyVelocity,
}

impl HobbyMissionPack {
    pub const fn is_valid(self) -> bool {
        self.identity != 0
            && self.vehicle_identity != 0
            && self.motor_identity != 0
            && self.environment_identity == HOBBY_ENVIRONMENT_ID
            && self.launch_altitude.raw() >= 0
            && self.rail_length.raw() > 0
            && self.main_deployment_altitude.raw() > 0
            && self.drogue_inflation_time.raw() > 0
            && self.main_inflation_time.raw() > 0
            && self.max_mission_time.raw() > 0
            && self.max_mission_time.raw() <= HOBBY_MAX_TIME_RAW
            && self.telemetry_period.raw() > 0
            && self.minimum_rail_exit_velocity.raw() > 0
    }
}

fn r32(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn zeros(input: &[u8], start: usize, end: usize) -> bool {
    input[start..end].iter().all(|value| *value == 0)
}

pub fn encode_vehicle_pack(
    pack: VerticalVehiclePack,
    output: &mut [u8; KVP7_LENGTH],
) -> Result<(), Phase7PackError> {
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    write_phase7_header(output, Phase7RecordKind::VehiclePack, pack.identity)?;
    output[32] = ModelProfileId::HobbyVerticalV1 as u8;
    w32(output, 36, pack.dry_mass.raw());
    w32(output, 40, pack.length.raw());
    w32(output, 44, pack.diameter.raw());
    w32(output, 48, pack.reference_area.raw());
    w32(output, 52, pack.body_cd_q16);
    w32(output, 56, pack.drogue_cda.raw());
    w32(output, 60, pack.main_cda.raw());
    wu32(output, 64, pack.identity);
    seal_phase7_record(output)?;
    Ok(())
}

pub fn parse_vehicle_pack(input: &[u8]) -> Result<VerticalVehiclePack, Phase7PackError> {
    let header = validate_phase7_record(input, Phase7RecordKind::VehiclePack)?;
    if input[32] != ModelProfileId::HobbyVerticalV1 as u8 {
        return Err(Phase7PackError::Profile);
    }
    if !zeros(input, 33, 36) || !zeros(input, VEHICLE_PAYLOAD_END, KVP7_LENGTH - 4) {
        return Err(Phase7PackError::Reserved);
    }
    if ru32(input, 64) != header.identity {
        return Err(Phase7PackError::Identity);
    }
    let pack = VerticalVehiclePack {
        identity: header.identity,
        dry_mass: HobbyMass::from_raw(r32(input, 36)),
        length: HobbyAltitude::from_raw(r32(input, 40)),
        diameter: HobbyAltitude::from_raw(r32(input, 44)),
        reference_area: HobbyArea::from_raw(r32(input, 48)),
        body_cd_q16: r32(input, 52),
        drogue_cda: HobbyRecoveryCda::from_raw(r32(input, 56)),
        main_cda: HobbyRecoveryCda::from_raw(r32(input, 60)),
    };
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    Ok(pack)
}

pub fn encode_motor_pack(
    pack: &MotorPack,
    output: &mut [u8; KMP7_LENGTH],
) -> Result<(), Phase7PackError> {
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    write_phase7_header(output, Phase7RecordKind::MotorPack, pack.identity)?;
    w32(output, 32, pack.loaded_mass.raw());
    w32(output, 36, pack.propellant_mass.raw());
    w32(output, 40, pack.total_impulse_raw_q16);
    w32(output, 44, pack.burn_time.raw());
    output[48] = pack.knot_count;
    let mut index = 0usize;
    while index < pack.knot_count as usize {
        let offset = MOTOR_KNOT_BASE + index * MOTOR_KNOT_LENGTH;
        w32(output, offset, pack.knots[index].time.raw());
        w32(output, offset + 4, pack.knots[index].thrust_raw_q13);
        index += 1;
    }
    seal_phase7_record(output)?;
    Ok(())
}

pub fn parse_motor_pack(input: &[u8]) -> Result<MotorPack, Phase7PackError> {
    let header = validate_phase7_record(input, Phase7RecordKind::MotorPack)?;
    if !zeros(input, 49, MOTOR_KNOT_BASE) {
        return Err(Phase7PackError::Reserved);
    }
    let count = input[48] as usize;
    if !(2..=KMP7_MAX_KNOTS).contains(&count) {
        return Err(Phase7PackError::Count);
    }
    let used_end = MOTOR_KNOT_BASE + count * MOTOR_KNOT_LENGTH;
    if !zeros(input, used_end, KMP7_LENGTH - 4) {
        return Err(Phase7PackError::Reserved);
    }
    let mut knots = [MotorKnot::ZERO; KMP7_MAX_KNOTS];
    let mut index = 0usize;
    while index < count {
        let offset = MOTOR_KNOT_BASE + index * MOTOR_KNOT_LENGTH;
        knots[index] = MotorKnot {
            time: HobbyTime::from_raw(r32(input, offset)),
            thrust_raw_q13: r32(input, offset + 4),
        };
        index += 1;
    }
    let pack = MotorPack {
        identity: header.identity,
        loaded_mass: HobbyMass::from_raw(r32(input, 32)),
        propellant_mass: HobbyMass::from_raw(r32(input, 36)),
        total_impulse_raw_q16: r32(input, 40),
        burn_time: HobbyTime::from_raw(r32(input, 44)),
        knot_count: count as u8,
        knots,
    };
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    Ok(pack)
}

pub fn encode_mission_pack(
    pack: HobbyMissionPack,
    output: &mut [u8; KMC7_LENGTH],
) -> Result<(), Phase7PackError> {
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    write_phase7_header(output, Phase7RecordKind::MissionPack, pack.identity)?;
    wu32(output, 32, pack.vehicle_identity);
    wu32(output, 36, pack.motor_identity);
    wu32(output, 40, pack.environment_identity);
    w32(output, 44, pack.launch_altitude.raw());
    w32(output, 48, pack.rail_length.raw());
    w32(output, 52, pack.main_deployment_altitude.raw());
    w32(output, 56, pack.drogue_inflation_time.raw());
    w32(output, 60, pack.main_inflation_time.raw());
    w32(output, 64, pack.max_mission_time.raw());
    w32(output, 68, pack.telemetry_period.raw());
    w32(output, 72, pack.minimum_rail_exit_velocity.raw());
    wu32(output, 76, pack.identity);
    seal_phase7_record(output)?;
    Ok(())
}

pub fn parse_mission_pack(input: &[u8]) -> Result<HobbyMissionPack, Phase7PackError> {
    let header = validate_phase7_record(input, Phase7RecordKind::MissionPack)?;
    if !zeros(input, MISSION_PAYLOAD_END, KMC7_LENGTH - 4) || ru32(input, 76) != header.identity {
        return Err(Phase7PackError::Reserved);
    }
    let pack = HobbyMissionPack {
        identity: header.identity,
        vehicle_identity: ru32(input, 32),
        motor_identity: ru32(input, 36),
        environment_identity: ru32(input, 40),
        launch_altitude: HobbyAltitude::from_raw(r32(input, 44)),
        rail_length: HobbyAltitude::from_raw(r32(input, 48)),
        main_deployment_altitude: HobbyAltitude::from_raw(r32(input, 52)),
        drogue_inflation_time: HobbyTime::from_raw(r32(input, 56)),
        main_inflation_time: HobbyTime::from_raw(r32(input, 60)),
        max_mission_time: HobbyTime::from_raw(r32(input, 64)),
        telemetry_period: HobbyTime::from_raw(r32(input, 68)),
        minimum_rail_exit_velocity: HobbyVelocity::from_raw(r32(input, 72)),
    };
    if !pack.is_valid() {
        return Err(Phase7PackError::Range);
    }
    Ok(pack)
}

pub const fn packs_are_compatible(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> bool {
    mission.vehicle_identity == vehicle.identity
        && mission.motor_identity == motor.identity
        && mission.environment_identity == HOBBY_ENVIRONMENT_ID
}
