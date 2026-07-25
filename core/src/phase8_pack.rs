//! Bounded Phase 8 spatial vehicle, motor, mission, and wind packs.

use crate::evaluation::ModelProfileId;
use crate::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordError,
    Phase8RecordKind, KMC8_LENGTH, KMP8_LENGTH, KMP8_MAX_KNOTS, KVP8_LENGTH, KVP8_MAX_AERO_KNOTS,
    KWP8_LENGTH, KWP8_MAX_WIND_KNOTS,
};
use crate::phase8_numeric::{
    SpatialAngle, SpatialArea, SpatialCoefficient, SpatialInertia, SpatialMass, SpatialMomentArm,
    SpatialPosition, SpatialTime, SpatialVelocity, SpatialWind, HOBBY_SPATIAL_ENVIRONMENT_ID,
    SPATIAL_MAX_ANGLE_RAW, SPATIAL_MAX_AREA_RAW, SPATIAL_MAX_COEFFICIENT_RAW,
    SPATIAL_MAX_INERTIA_RAW, SPATIAL_MAX_MASS_RAW, SPATIAL_MAX_MOMENT_ARM_RAW,
    SPATIAL_MAX_POSITION_RAW, SPATIAL_MAX_TIME_RAW, SPATIAL_MAX_VELOCITY_RAW, SPATIAL_MAX_WIND_RAW,
};

const VEHICLE_AERO_BASE: usize = 128;
const VEHICLE_AERO_LENGTH: usize = 16;
const MOTOR_KNOT_BASE: usize = 128;
const MOTOR_KNOT_LENGTH: usize = 8;
const MISSION_PAYLOAD_END: usize = 96;
const WIND_KNOT_BASE: usize = 64;
const WIND_KNOT_LENGTH: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase8PackError {
    Record(Phase8RecordError),
    Profile,
    Identity,
    Reference,
    Range,
    Count,
    Ordering,
    Reserved,
}

impl From<Phase8RecordError> for Phase8PackError {
    fn from(value: Phase8RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AeroKnot {
    pub mach: SpatialCoefficient,
    pub axial_cd: SpatialCoefficient,
    pub cp_from_nose: SpatialMomentArm,
    pub normal_force_slope: SpatialCoefficient,
}

impl AeroKnot {
    pub const ZERO: Self = Self {
        mach: SpatialCoefficient::ZERO,
        axial_cd: SpatialCoefficient::ZERO,
        cp_from_nose: SpatialMomentArm::ZERO,
        normal_force_slope: SpatialCoefficient::ZERO,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialVehiclePack {
    pub identity: u32,
    pub dry_mass: SpatialMass,
    pub length: SpatialPosition,
    pub diameter: SpatialPosition,
    pub reference_area: SpatialArea,
    pub dry_cg_from_nose: SpatialMomentArm,
    pub dry_inertia: [SpatialInertia; 3],
    pub motor_aft_from_tail: SpatialPosition,
    pub aft_rail_guide_from_tail: SpatialPosition,
    pub forward_rail_guide_from_tail: SpatialPosition,
    pub drogue_cda: SpatialArea,
    pub main_cda: SpatialArea,
    pub pitch_damping: SpatialCoefficient,
    pub yaw_damping: SpatialCoefficient,
    pub roll_damping: SpatialCoefficient,
    pub source_manifest_identity: u32,
    pub aero_knot_count: u8,
    pub aero_knots: [AeroKnot; KVP8_MAX_AERO_KNOTS],
}

impl SpatialVehiclePack {
    pub fn is_valid(&self) -> bool {
        if self.identity == 0
            || self.source_manifest_identity == 0
            || !positive_bounded(self.dry_mass.raw(), SPATIAL_MAX_MASS_RAW)
            || !positive_bounded(self.length.raw(), SPATIAL_MAX_POSITION_RAW)
            || !positive_bounded(self.diameter.raw(), self.length.raw())
            || !positive_bounded(self.reference_area.raw(), SPATIAL_MAX_AREA_RAW)
            || self.dry_cg_from_nose.raw() <= 0
            || self.dry_cg_from_nose.raw() as i64 > (self.length.raw() as i64) << 15
            || self
                .dry_inertia
                .iter()
                .any(|v| !positive_bounded(v.raw(), SPATIAL_MAX_INERTIA_RAW))
            || !bounded_nonnegative(self.motor_aft_from_tail.raw(), self.length.raw())
            || !bounded_nonnegative(self.aft_rail_guide_from_tail.raw(), self.length.raw())
            || !bounded_nonnegative(self.forward_rail_guide_from_tail.raw(), self.length.raw())
            || self.forward_rail_guide_from_tail.raw() <= self.aft_rail_guide_from_tail.raw()
            || !positive_bounded(self.drogue_cda.raw(), SPATIAL_MAX_AREA_RAW)
            || !positive_bounded(self.main_cda.raw(), SPATIAL_MAX_AREA_RAW)
            || self.aero_knot_count < 2
            || self.aero_knot_count as usize > KVP8_MAX_AERO_KNOTS
        {
            return false;
        }
        let mut previous_mach = -1;
        let mut index = 0usize;
        while index < self.aero_knot_count as usize {
            let knot = self.aero_knots[index];
            if knot.mach.raw() <= previous_mach
                || !positive_bounded(knot.axial_cd.raw(), SPATIAL_MAX_COEFFICIENT_RAW)
                || !positive_bounded(knot.cp_from_nose.raw(), SPATIAL_MAX_MOMENT_ARM_RAW)
                || !positive_bounded(knot.normal_force_slope.raw(), SPATIAL_MAX_COEFFICIENT_RAW)
            {
                return false;
            }
            previous_mach = knot.mach.raw();
            index += 1;
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialMotorKnot {
    pub time: SpatialTime,
    pub thrust_raw_q13: i32,
}

impl SpatialMotorKnot {
    pub const ZERO: Self = Self {
        time: SpatialTime::ZERO,
        thrust_raw_q13: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialMotorPack {
    pub identity: u32,
    pub loaded_mass: SpatialMass,
    pub propellant_mass: SpatialMass,
    pub length: SpatialPosition,
    pub diameter: SpatialPosition,
    pub loaded_cg_from_aft: SpatialMomentArm,
    pub dry_cg_from_aft: SpatialMomentArm,
    pub loaded_axial_inertia: SpatialInertia,
    pub loaded_transverse_inertia: SpatialInertia,
    pub dry_axial_inertia: SpatialInertia,
    pub dry_transverse_inertia: SpatialInertia,
    pub total_impulse_raw_q16: i32,
    pub burn_time: SpatialTime,
    pub knot_count: u8,
    pub knots: [SpatialMotorKnot; KMP8_MAX_KNOTS],
}

impl SpatialMotorPack {
    pub fn is_valid(&self) -> bool {
        if self.identity == 0
            || !positive_bounded(self.loaded_mass.raw(), SPATIAL_MAX_MASS_RAW)
            || !positive_bounded(self.propellant_mass.raw(), self.loaded_mass.raw() - 1)
            || !positive_bounded(self.length.raw(), SPATIAL_MAX_POSITION_RAW)
            || !positive_bounded(self.diameter.raw(), self.length.raw())
            || !positive_bounded(self.loaded_cg_from_aft.raw(), SPATIAL_MAX_MOMENT_ARM_RAW)
            || !positive_bounded(self.dry_cg_from_aft.raw(), SPATIAL_MAX_MOMENT_ARM_RAW)
            || !positive_bounded(self.loaded_axial_inertia.raw(), SPATIAL_MAX_INERTIA_RAW)
            || !positive_bounded(
                self.loaded_transverse_inertia.raw(),
                SPATIAL_MAX_INERTIA_RAW,
            )
            || !positive_bounded(self.dry_axial_inertia.raw(), SPATIAL_MAX_INERTIA_RAW)
            || !positive_bounded(self.dry_transverse_inertia.raw(), SPATIAL_MAX_INERTIA_RAW)
            || self.total_impulse_raw_q16 <= 0
            || !positive_bounded(self.burn_time.raw(), SPATIAL_MAX_TIME_RAW)
            || self.knot_count < 2
            || self.knot_count as usize > KMP8_MAX_KNOTS
        {
            return false;
        }
        let mut previous_time = -1;
        let mut index = 0usize;
        while index < self.knot_count as usize {
            let knot = self.knots[index];
            if knot.time.raw() <= previous_time || knot.thrust_raw_q13 < 0 {
                return false;
            }
            previous_time = knot.time.raw();
            index += 1;
        }
        let last = self.knots[self.knot_count as usize - 1];
        last.time == self.burn_time && last.thrust_raw_q13 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialMissionPack {
    pub identity: u32,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub wind_identity: u32,
    pub environment_identity: u32,
    pub launch_altitude: SpatialPosition,
    pub rail_length: SpatialPosition,
    pub launch_azimuth: SpatialAngle,
    pub launch_elevation: SpatialAngle,
    pub main_deployment_altitude: SpatialPosition,
    pub drogue_inflation_time: SpatialTime,
    pub main_inflation_time: SpatialTime,
    pub max_mission_time: SpatialTime,
    pub telemetry_period: SpatialTime,
    pub minimum_rail_exit_velocity: SpatialVelocity,
    pub case_seed: u32,
}

impl SpatialMissionPack {
    pub const fn is_valid(self) -> bool {
        self.identity != 0
            && self.vehicle_identity != 0
            && self.motor_identity != 0
            && self.wind_identity != 0
            && self.environment_identity == HOBBY_SPATIAL_ENVIRONMENT_ID
            && bounded_nonnegative(self.launch_altitude.raw(), SPATIAL_MAX_POSITION_RAW)
            && positive_bounded(self.rail_length.raw(), SPATIAL_MAX_POSITION_RAW)
            && self.launch_azimuth.raw().unsigned_abs() <= SPATIAL_MAX_ANGLE_RAW as u32
            && self.launch_elevation.raw() > 0
            && self.launch_elevation.raw() <= SPATIAL_MAX_ANGLE_RAW
            && positive_bounded(
                self.main_deployment_altitude.raw(),
                SPATIAL_MAX_POSITION_RAW,
            )
            && positive_bounded(self.drogue_inflation_time.raw(), SPATIAL_MAX_TIME_RAW)
            && positive_bounded(self.main_inflation_time.raw(), SPATIAL_MAX_TIME_RAW)
            && positive_bounded(self.max_mission_time.raw(), SPATIAL_MAX_TIME_RAW)
            && positive_bounded(self.telemetry_period.raw(), SPATIAL_MAX_TIME_RAW)
            && positive_bounded(
                self.minimum_rail_exit_velocity.raw(),
                SPATIAL_MAX_VELOCITY_RAW,
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindKnot {
    pub altitude: SpatialPosition,
    pub east: SpatialWind,
    pub north: SpatialWind,
}

impl WindKnot {
    pub const ZERO: Self = Self {
        altitude: SpatialPosition::ZERO,
        east: SpatialWind::ZERO,
        north: SpatialWind::ZERO,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindProfilePack {
    pub identity: u32,
    pub gust_seed: u32,
    pub gust_cadence: SpatialTime,
    pub gust_amplitude_east: SpatialWind,
    pub gust_amplitude_north: SpatialWind,
    pub max_gust: SpatialWind,
    pub knot_count: u8,
    pub knots: [WindKnot; KWP8_MAX_WIND_KNOTS],
}

impl WindProfilePack {
    pub fn is_valid(&self) -> bool {
        if self.identity == 0
            || !positive_bounded(self.gust_cadence.raw(), SPATIAL_MAX_TIME_RAW)
            || self.gust_amplitude_east.raw().unsigned_abs() > SPATIAL_MAX_WIND_RAW as u32
            || self.gust_amplitude_north.raw().unsigned_abs() > SPATIAL_MAX_WIND_RAW as u32
            || !bounded_nonnegative(self.max_gust.raw(), SPATIAL_MAX_WIND_RAW)
            || self.knot_count == 0
            || self.knot_count as usize > KWP8_MAX_WIND_KNOTS
        {
            return false;
        }
        let mut previous_altitude = -1;
        let mut index = 0usize;
        while index < self.knot_count as usize {
            let knot = self.knots[index];
            if knot.altitude.raw() <= previous_altitude
                || knot.east.raw().unsigned_abs() > SPATIAL_MAX_WIND_RAW as u32
                || knot.north.raw().unsigned_abs() > SPATIAL_MAX_WIND_RAW as u32
            {
                return false;
            }
            previous_altitude = knot.altitude.raw();
            index += 1;
        }
        self.knots[0].altitude.raw() == 0
    }
}

pub fn packs_are_compatible(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
) -> bool {
    vehicle.is_valid()
        && motor.is_valid()
        && mission.is_valid()
        && wind.is_valid()
        && mission.vehicle_identity == vehicle.identity
        && mission.motor_identity == motor.identity
        && mission.wind_identity == wind.identity
}

const fn positive_bounded(value: i32, maximum: i32) -> bool {
    value > 0 && value <= maximum
}

const fn bounded_nonnegative(value: i32, maximum: i32) -> bool {
    value >= 0 && value <= maximum
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

pub fn encode_spatial_vehicle_pack(
    pack: &SpatialVehiclePack,
    output: &mut [u8; KVP8_LENGTH],
) -> Result<(), Phase8PackError> {
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    write_phase8_header(output, Phase8RecordKind::VehiclePack, pack.identity)?;
    output[32] = ModelProfileId::HobbySpatialV1 as u8;
    output[33] = pack.aero_knot_count;
    for (offset, value) in [
        (36, pack.dry_mass.raw()),
        (40, pack.length.raw()),
        (44, pack.diameter.raw()),
        (48, pack.reference_area.raw()),
        (52, pack.dry_cg_from_nose.raw()),
        (56, pack.dry_inertia[0].raw()),
        (60, pack.dry_inertia[1].raw()),
        (64, pack.dry_inertia[2].raw()),
        (68, pack.motor_aft_from_tail.raw()),
        (72, pack.aft_rail_guide_from_tail.raw()),
        (76, pack.forward_rail_guide_from_tail.raw()),
        (80, pack.drogue_cda.raw()),
        (84, pack.main_cda.raw()),
        (88, pack.pitch_damping.raw()),
        (92, pack.yaw_damping.raw()),
        (96, pack.roll_damping.raw()),
    ] {
        w32(output, offset, value);
    }
    wu32(output, 100, pack.source_manifest_identity);
    let mut index = 0usize;
    while index < pack.aero_knot_count as usize {
        let offset = VEHICLE_AERO_BASE + index * VEHICLE_AERO_LENGTH;
        let knot = pack.aero_knots[index];
        w32(output, offset, knot.mach.raw());
        w32(output, offset + 4, knot.axial_cd.raw());
        w32(output, offset + 8, knot.cp_from_nose.raw());
        w32(output, offset + 12, knot.normal_force_slope.raw());
        index += 1;
    }
    seal_phase8_record(output)?;
    Ok(())
}

pub fn parse_spatial_vehicle_pack(input: &[u8]) -> Result<SpatialVehiclePack, Phase8PackError> {
    let header = validate_phase8_record(input, Phase8RecordKind::VehiclePack)?;
    if input[32] != ModelProfileId::HobbySpatialV1 as u8 {
        return Err(Phase8PackError::Profile);
    }
    let count = input[33] as usize;
    if !(2..=KVP8_MAX_AERO_KNOTS).contains(&count) {
        return Err(Phase8PackError::Count);
    }
    let used_end = VEHICLE_AERO_BASE + count * VEHICLE_AERO_LENGTH;
    if !zeros(input, 34, 36)
        || !zeros(input, 104, VEHICLE_AERO_BASE)
        || !zeros(input, used_end, KVP8_LENGTH - 4)
    {
        return Err(Phase8PackError::Reserved);
    }
    let mut knots = [AeroKnot::ZERO; KVP8_MAX_AERO_KNOTS];
    let mut index = 0usize;
    while index < count {
        let offset = VEHICLE_AERO_BASE + index * VEHICLE_AERO_LENGTH;
        knots[index] = AeroKnot {
            mach: SpatialCoefficient::from_raw(r32(input, offset)),
            axial_cd: SpatialCoefficient::from_raw(r32(input, offset + 4)),
            cp_from_nose: SpatialMomentArm::from_raw(r32(input, offset + 8)),
            normal_force_slope: SpatialCoefficient::from_raw(r32(input, offset + 12)),
        };
        index += 1;
    }
    let pack = SpatialVehiclePack {
        identity: header.identity,
        dry_mass: SpatialMass::from_raw(r32(input, 36)),
        length: SpatialPosition::from_raw(r32(input, 40)),
        diameter: SpatialPosition::from_raw(r32(input, 44)),
        reference_area: SpatialArea::from_raw(r32(input, 48)),
        dry_cg_from_nose: SpatialMomentArm::from_raw(r32(input, 52)),
        dry_inertia: [
            SpatialInertia::from_raw(r32(input, 56)),
            SpatialInertia::from_raw(r32(input, 60)),
            SpatialInertia::from_raw(r32(input, 64)),
        ],
        motor_aft_from_tail: SpatialPosition::from_raw(r32(input, 68)),
        aft_rail_guide_from_tail: SpatialPosition::from_raw(r32(input, 72)),
        forward_rail_guide_from_tail: SpatialPosition::from_raw(r32(input, 76)),
        drogue_cda: SpatialArea::from_raw(r32(input, 80)),
        main_cda: SpatialArea::from_raw(r32(input, 84)),
        pitch_damping: SpatialCoefficient::from_raw(r32(input, 88)),
        yaw_damping: SpatialCoefficient::from_raw(r32(input, 92)),
        roll_damping: SpatialCoefficient::from_raw(r32(input, 96)),
        source_manifest_identity: ru32(input, 100),
        aero_knot_count: count as u8,
        aero_knots: knots,
    };
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    Ok(pack)
}

pub fn encode_spatial_motor_pack(
    pack: &SpatialMotorPack,
    output: &mut [u8; KMP8_LENGTH],
) -> Result<(), Phase8PackError> {
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    write_phase8_header(output, Phase8RecordKind::MotorPack, pack.identity)?;
    output[32] = ModelProfileId::HobbySpatialV1 as u8;
    output[33] = pack.knot_count;
    for (offset, value) in [
        (36, pack.loaded_mass.raw()),
        (40, pack.propellant_mass.raw()),
        (44, pack.length.raw()),
        (48, pack.diameter.raw()),
        (52, pack.loaded_cg_from_aft.raw()),
        (56, pack.dry_cg_from_aft.raw()),
        (60, pack.loaded_axial_inertia.raw()),
        (64, pack.loaded_transverse_inertia.raw()),
        (68, pack.dry_axial_inertia.raw()),
        (72, pack.dry_transverse_inertia.raw()),
        (76, pack.total_impulse_raw_q16),
        (80, pack.burn_time.raw()),
    ] {
        w32(output, offset, value);
    }
    let mut index = 0usize;
    while index < pack.knot_count as usize {
        let offset = MOTOR_KNOT_BASE + index * MOTOR_KNOT_LENGTH;
        w32(output, offset, pack.knots[index].time.raw());
        w32(output, offset + 4, pack.knots[index].thrust_raw_q13);
        index += 1;
    }
    seal_phase8_record(output)?;
    Ok(())
}

pub fn parse_spatial_motor_pack(input: &[u8]) -> Result<SpatialMotorPack, Phase8PackError> {
    let header = validate_phase8_record(input, Phase8RecordKind::MotorPack)?;
    if input[32] != ModelProfileId::HobbySpatialV1 as u8 {
        return Err(Phase8PackError::Profile);
    }
    let count = input[33] as usize;
    if !(2..=KMP8_MAX_KNOTS).contains(&count) {
        return Err(Phase8PackError::Count);
    }
    let used_end = MOTOR_KNOT_BASE + count * MOTOR_KNOT_LENGTH;
    if !zeros(input, 34, 36)
        || !zeros(input, 84, MOTOR_KNOT_BASE)
        || !zeros(input, used_end, KMP8_LENGTH - 4)
    {
        return Err(Phase8PackError::Reserved);
    }
    let mut knots = [SpatialMotorKnot::ZERO; KMP8_MAX_KNOTS];
    let mut index = 0usize;
    while index < count {
        let offset = MOTOR_KNOT_BASE + index * MOTOR_KNOT_LENGTH;
        knots[index] = SpatialMotorKnot {
            time: SpatialTime::from_raw(r32(input, offset)),
            thrust_raw_q13: r32(input, offset + 4),
        };
        index += 1;
    }
    let pack = SpatialMotorPack {
        identity: header.identity,
        loaded_mass: SpatialMass::from_raw(r32(input, 36)),
        propellant_mass: SpatialMass::from_raw(r32(input, 40)),
        length: SpatialPosition::from_raw(r32(input, 44)),
        diameter: SpatialPosition::from_raw(r32(input, 48)),
        loaded_cg_from_aft: SpatialMomentArm::from_raw(r32(input, 52)),
        dry_cg_from_aft: SpatialMomentArm::from_raw(r32(input, 56)),
        loaded_axial_inertia: SpatialInertia::from_raw(r32(input, 60)),
        loaded_transverse_inertia: SpatialInertia::from_raw(r32(input, 64)),
        dry_axial_inertia: SpatialInertia::from_raw(r32(input, 68)),
        dry_transverse_inertia: SpatialInertia::from_raw(r32(input, 72)),
        total_impulse_raw_q16: r32(input, 76),
        burn_time: SpatialTime::from_raw(r32(input, 80)),
        knot_count: count as u8,
        knots,
    };
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    Ok(pack)
}

pub fn encode_spatial_mission_pack(
    pack: SpatialMissionPack,
    output: &mut [u8; KMC8_LENGTH],
) -> Result<(), Phase8PackError> {
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    write_phase8_header(output, Phase8RecordKind::MissionPack, pack.identity)?;
    output[32] = ModelProfileId::HobbySpatialV1 as u8;
    for (offset, value) in [
        (36, pack.vehicle_identity),
        (40, pack.motor_identity),
        (44, pack.wind_identity),
        (48, pack.environment_identity),
    ] {
        wu32(output, offset, value);
    }
    for (offset, value) in [
        (52, pack.launch_altitude.raw()),
        (56, pack.rail_length.raw()),
        (60, pack.launch_azimuth.raw()),
        (64, pack.launch_elevation.raw()),
        (68, pack.main_deployment_altitude.raw()),
        (72, pack.drogue_inflation_time.raw()),
        (76, pack.main_inflation_time.raw()),
        (80, pack.max_mission_time.raw()),
        (84, pack.telemetry_period.raw()),
        (88, pack.minimum_rail_exit_velocity.raw()),
    ] {
        w32(output, offset, value);
    }
    wu32(output, 92, pack.case_seed);
    seal_phase8_record(output)?;
    Ok(())
}

pub fn parse_spatial_mission_pack(input: &[u8]) -> Result<SpatialMissionPack, Phase8PackError> {
    let header = validate_phase8_record(input, Phase8RecordKind::MissionPack)?;
    if input[32] != ModelProfileId::HobbySpatialV1 as u8 {
        return Err(Phase8PackError::Profile);
    }
    if !zeros(input, 33, 36) || !zeros(input, MISSION_PAYLOAD_END, KMC8_LENGTH - 4) {
        return Err(Phase8PackError::Reserved);
    }
    let pack = SpatialMissionPack {
        identity: header.identity,
        vehicle_identity: ru32(input, 36),
        motor_identity: ru32(input, 40),
        wind_identity: ru32(input, 44),
        environment_identity: ru32(input, 48),
        launch_altitude: SpatialPosition::from_raw(r32(input, 52)),
        rail_length: SpatialPosition::from_raw(r32(input, 56)),
        launch_azimuth: SpatialAngle::from_raw(r32(input, 60)),
        launch_elevation: SpatialAngle::from_raw(r32(input, 64)),
        main_deployment_altitude: SpatialPosition::from_raw(r32(input, 68)),
        drogue_inflation_time: SpatialTime::from_raw(r32(input, 72)),
        main_inflation_time: SpatialTime::from_raw(r32(input, 76)),
        max_mission_time: SpatialTime::from_raw(r32(input, 80)),
        telemetry_period: SpatialTime::from_raw(r32(input, 84)),
        minimum_rail_exit_velocity: SpatialVelocity::from_raw(r32(input, 88)),
        case_seed: ru32(input, 92),
    };
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    Ok(pack)
}

pub fn encode_wind_profile_pack(
    pack: &WindProfilePack,
    output: &mut [u8; KWP8_LENGTH],
) -> Result<(), Phase8PackError> {
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    write_phase8_header(output, Phase8RecordKind::WindPack, pack.identity)?;
    output[32] = ModelProfileId::HobbySpatialV1 as u8;
    output[33] = pack.knot_count;
    wu32(output, 36, pack.gust_seed);
    w32(output, 40, pack.gust_cadence.raw());
    w32(output, 44, pack.gust_amplitude_east.raw());
    w32(output, 48, pack.gust_amplitude_north.raw());
    w32(output, 52, pack.max_gust.raw());
    let mut index = 0usize;
    while index < pack.knot_count as usize {
        let offset = WIND_KNOT_BASE + index * WIND_KNOT_LENGTH;
        let knot = pack.knots[index];
        w32(output, offset, knot.altitude.raw());
        w32(output, offset + 4, knot.east.raw());
        w32(output, offset + 8, knot.north.raw());
        index += 1;
    }
    seal_phase8_record(output)?;
    Ok(())
}

pub fn parse_wind_profile_pack(input: &[u8]) -> Result<WindProfilePack, Phase8PackError> {
    let header = validate_phase8_record(input, Phase8RecordKind::WindPack)?;
    if input[32] != ModelProfileId::HobbySpatialV1 as u8 {
        return Err(Phase8PackError::Profile);
    }
    let count = input[33] as usize;
    if !(1..=KWP8_MAX_WIND_KNOTS).contains(&count) {
        return Err(Phase8PackError::Count);
    }
    let used_end = WIND_KNOT_BASE + count * WIND_KNOT_LENGTH;
    if !zeros(input, 34, 36)
        || !zeros(input, 56, WIND_KNOT_BASE)
        || !zeros(input, used_end, KWP8_LENGTH - 4)
    {
        return Err(Phase8PackError::Reserved);
    }
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    let mut index = 0usize;
    while index < count {
        let offset = WIND_KNOT_BASE + index * WIND_KNOT_LENGTH;
        knots[index] = WindKnot {
            altitude: SpatialPosition::from_raw(r32(input, offset)),
            east: SpatialWind::from_raw(r32(input, offset + 4)),
            north: SpatialWind::from_raw(r32(input, offset + 8)),
        };
        index += 1;
    }
    let pack = WindProfilePack {
        identity: header.identity,
        gust_seed: ru32(input, 36),
        gust_cadence: SpatialTime::from_raw(r32(input, 40)),
        gust_amplitude_east: SpatialWind::from_raw(r32(input, 44)),
        gust_amplitude_north: SpatialWind::from_raw(r32(input, 48)),
        max_gust: SpatialWind::from_raw(r32(input, 52)),
        knot_count: count as u8,
        knots,
    };
    if !pack.is_valid() {
        return Err(Phase8PackError::Range);
    }
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vehicle() -> SpatialVehiclePack {
        let mut aero = [AeroKnot::ZERO; KVP8_MAX_AERO_KNOTS];
        aero[0] = AeroKnot {
            mach: SpatialCoefficient::from_raw(1),
            axial_cd: SpatialCoefficient::from_raw(8_000_000),
            cp_from_nose: SpatialMomentArm::from_raw(300_000_000),
            normal_force_slope: SpatialCoefficient::from_raw(80_000_000),
        };
        aero[1] = AeroKnot {
            mach: SpatialCoefficient::from_raw(10_000_000),
            ..aero[0]
        };
        SpatialVehiclePack {
            identity: 11,
            dry_mass: SpatialMass::from_raw(4_000_000),
            length: SpatialPosition::from_raw(16_000),
            diameter: SpatialPosition::from_raw(500),
            reference_area: SpatialArea::from_raw(700_000),
            dry_cg_from_nose: SpatialMomentArm::from_raw(300_000_000),
            dry_inertia: [SpatialInertia::from_raw(50_000); 3],
            motor_aft_from_tail: SpatialPosition::ZERO,
            aft_rail_guide_from_tail: SpatialPosition::from_raw(100),
            forward_rail_guide_from_tail: SpatialPosition::from_raw(1_000),
            drogue_cda: SpatialArea::from_raw(1_000_000),
            main_cda: SpatialArea::from_raw(2_000_000),
            pitch_damping: SpatialCoefficient::from_raw(1),
            yaw_damping: SpatialCoefficient::from_raw(1),
            roll_damping: SpatialCoefficient::from_raw(1),
            source_manifest_identity: 12,
            aero_knot_count: 2,
            aero_knots: aero,
        }
    }

    fn sample_motor() -> SpatialMotorPack {
        let mut knots = [SpatialMotorKnot::ZERO; KMP8_MAX_KNOTS];
        knots[0] = SpatialMotorKnot {
            time: SpatialTime::from_raw(1),
            thrust_raw_q13: 1_000,
        };
        knots[1] = SpatialMotorKnot {
            time: SpatialTime::from_raw(200),
            thrust_raw_q13: 0,
        };
        SpatialMotorPack {
            identity: 21,
            loaded_mass: SpatialMass::from_raw(2_000_000),
            propellant_mass: SpatialMass::from_raw(1_000_000),
            length: SpatialPosition::from_raw(1_000),
            diameter: SpatialPosition::from_raw(100),
            loaded_cg_from_aft: SpatialMomentArm::from_raw(10_000),
            dry_cg_from_aft: SpatialMomentArm::from_raw(10_000),
            loaded_axial_inertia: SpatialInertia::from_raw(1),
            loaded_transverse_inertia: SpatialInertia::from_raw(2),
            dry_axial_inertia: SpatialInertia::from_raw(1),
            dry_transverse_inertia: SpatialInertia::from_raw(2),
            total_impulse_raw_q16: 1,
            burn_time: SpatialTime::from_raw(200),
            knot_count: 2,
            knots,
        }
    }

    fn sample_mission() -> SpatialMissionPack {
        SpatialMissionPack {
            identity: 31,
            vehicle_identity: 11,
            motor_identity: 21,
            wind_identity: 41,
            environment_identity: HOBBY_SPATIAL_ENVIRONMENT_ID,
            launch_altitude: SpatialPosition::ZERO,
            rail_length: SpatialPosition::from_raw(10),
            launch_azimuth: SpatialAngle::ZERO,
            launch_elevation: SpatialAngle::from_raw(100),
            main_deployment_altitude: SpatialPosition::from_raw(100),
            drogue_inflation_time: SpatialTime::from_raw(10),
            main_inflation_time: SpatialTime::from_raw(10),
            max_mission_time: SpatialTime::from_raw(100),
            telemetry_period: SpatialTime::from_raw(1),
            minimum_rail_exit_velocity: SpatialVelocity::from_raw(1),
            case_seed: 42,
        }
    }

    fn sample_wind() -> WindProfilePack {
        let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
        knots[0] = WindKnot {
            altitude: SpatialPosition::ZERO,
            east: SpatialWind::from_raw(1),
            north: SpatialWind::from_raw(2),
        };
        WindProfilePack {
            identity: 41,
            gust_seed: 7,
            gust_cadence: SpatialTime::from_raw(10),
            gust_amplitude_east: SpatialWind::from_raw(1),
            gust_amplitude_north: SpatialWind::from_raw(1),
            max_gust: SpatialWind::from_raw(2),
            knot_count: 1,
            knots,
        }
    }

    #[test]
    fn all_packs_round_trip_and_bind() {
        let vehicle = sample_vehicle();
        let motor = sample_motor();
        let mission = sample_mission();
        let wind = sample_wind();
        let mut vb = [0; KVP8_LENGTH];
        let mut mb = [0; KMP8_LENGTH];
        let mut cb = [0; KMC8_LENGTH];
        let mut wb = [0; KWP8_LENGTH];
        encode_spatial_vehicle_pack(&vehicle, &mut vb).unwrap();
        encode_spatial_motor_pack(&motor, &mut mb).unwrap();
        encode_spatial_mission_pack(mission, &mut cb).unwrap();
        encode_wind_profile_pack(&wind, &mut wb).unwrap();
        let v = parse_spatial_vehicle_pack(&vb).unwrap();
        let m = parse_spatial_motor_pack(&mb).unwrap();
        let c = parse_spatial_mission_pack(&cb).unwrap();
        let w = parse_wind_profile_pack(&wb).unwrap();
        assert!(packs_are_compatible(&v, &m, c, &w));
    }

    #[test]
    fn payload_reserved_bytes_fail_closed() {
        let mut bytes = [0; KMC8_LENGTH];
        encode_spatial_mission_pack(sample_mission(), &mut bytes).unwrap();
        bytes[100] = 1;
        seal_phase8_record(&mut bytes).unwrap();
        assert_eq!(
            parse_spatial_mission_pack(&bytes),
            Err(Phase8PackError::Reserved)
        );
    }
}
