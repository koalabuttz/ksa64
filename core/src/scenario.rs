//! Parser and validator for the fixed 76-byte Phase 1 scenario image.

use crate::numeric::{add, divide_scaled, NumericStatus};
use crate::quantities::{Altitude, Cda, Force, Mass, MassFlow, Time, Velocity};

pub const SCENARIO_IMAGE_LENGTH: usize = 76;
pub const SCENARIO_VERSION: u16 = 1;
pub const NUMERIC_CONTRACT_ID: u32 = fnv1a_32(b"ksa64.numeric.phase1-v1");
pub const SIMPLE_EARTH_ENVIRONMENT_ID: u32 = fnv1a_32(b"earth.simple-atmosphere.v1");

pub const fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut value = 2_166_136_261u32;
    let mut index = 0usize;
    while index < bytes.len() {
        value ^= bytes[index] as u32;
        value = value.wrapping_mul(16_777_619);
        index += 1;
    }
    value
}

const MAGIC: [u8; 4] = *b"KSC1";
const MAX_TIMESTEP_Q16: i32 = 8_192;
const MAX_DURATION_Q16: u32 = 268_435_456;
const MAX_STEPS: u32 = 32_768;
const MIN_ALTITUDE_Q12: i32 = -8_192;
const MAX_ALTITUDE_Q12: i32 = 8_192_000;
const MIN_VELOCITY_Q24: i32 = -134_217_728;
const MAX_VELOCITY_Q24: i32 = 134_217_728;
const MAX_MASS_Q12: i32 = 20_480_000;
const MAX_THRUST_Q12: i32 = 409_600_000;
const MAX_MASS_FLOW_Q16: i32 = 6_553_600;
const MAX_CDA_Q16: i32 = 131_072_000;
const MAX_ACCELERATION_Q28: i32 = 26_843_546;
const CONSERVATIVE_GRAVITY_Q28: i32 = 3_221_225;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioField {
    Timestep,
    Steps,
    TelemetryStride,
    Altitude,
    Velocity,
    TotalMass,
    Propellant,
    DryMass,
    Thrust,
    MassFlow,
    BurnDuration,
    Cda,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioError {
    Length,
    Magic,
    Version,
    RecordLength,
    Checksum,
    NumericContract,
    Environment,
    FieldRange(ScenarioField),
    MassInvariant,
    Duration,
    AccelerationEnvelope,
    NumericFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitialConditions {
    altitude: Altitude,
    velocity: Velocity,
    total_mass: Mass,
    propellant: Mass,
}

impl InitialConditions {
    pub const fn altitude(&self) -> Altitude {
        self.altitude
    }
    pub const fn velocity(&self) -> Velocity {
        self.velocity
    }
    pub const fn total_mass(&self) -> Mass {
        self.total_mass
    }
    pub const fn propellant(&self) -> Mass {
        self.propellant
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleConfig {
    dry_mass: Mass,
    thrust: Force,
    mass_flow: MassFlow,
    burn_duration: Time,
    cda: Cda,
}

impl VehicleConfig {
    pub const fn dry_mass(&self) -> Mass {
        self.dry_mass
    }
    pub const fn thrust(&self) -> Force {
        self.thrust
    }
    pub const fn mass_flow(&self) -> MassFlow {
        self.mass_flow
    }
    pub const fn burn_duration(&self) -> Time {
        self.burn_duration
    }
    pub const fn cda(&self) -> Cda {
        self.cda
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scenario {
    scenario_id: u32,
    timestep: Time,
    steps: u32,
    telemetry_stride: u16,
    flags: u16,
    seed: u32,
    initial: InitialConditions,
    vehicle: VehicleConfig,
    environment_id: u32,
}

impl Scenario {
    pub const fn scenario_id(&self) -> u32 {
        self.scenario_id
    }
    pub const fn timestep(&self) -> Time {
        self.timestep
    }
    pub const fn steps(&self) -> u32 {
        self.steps
    }
    pub const fn telemetry_stride(&self) -> u16 {
        self.telemetry_stride
    }
    pub const fn flags(&self) -> u16 {
        self.flags
    }
    pub const fn seed(&self) -> u32 {
        self.seed
    }
    pub const fn initial(&self) -> &InitialConditions {
        &self.initial
    }
    pub const fn vehicle(&self) -> &VehicleConfig {
        &self.vehicle
    }
    pub const fn environment_id(&self) -> u32 {
        self.environment_id
    }
}

#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    read_u32(bytes, offset) as i32
}

pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    let mut index = 0usize;
    while index < bytes.len() {
        crc ^= bytes[index] as u32;
        let mut bit = 0u8;
        while bit < 8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            bit += 1;
        }
        index += 1;
    }
    !crc
}

#[inline]
fn in_range(
    value: i32,
    minimum: i32,
    maximum: i32,
    field: ScenarioField,
) -> Result<(), ScenarioError> {
    if value < minimum || value > maximum {
        Err(ScenarioError::FieldRange(field))
    } else {
        Ok(())
    }
}

pub fn parse_scenario_image(bytes: &[u8]) -> Result<Scenario, ScenarioError> {
    if bytes.len() != SCENARIO_IMAGE_LENGTH {
        return Err(ScenarioError::Length);
    }
    if bytes[0..4] != MAGIC {
        return Err(ScenarioError::Magic);
    }
    if read_u16(bytes, 4) != SCENARIO_VERSION {
        return Err(ScenarioError::Version);
    }
    if read_u16(bytes, 6) as usize != SCENARIO_IMAGE_LENGTH {
        return Err(ScenarioError::RecordLength);
    }
    if crc32_ieee(&bytes[..72]) != read_u32(bytes, 72) {
        return Err(ScenarioError::Checksum);
    }
    if read_u32(bytes, 8) != NUMERIC_CONTRACT_ID {
        return Err(ScenarioError::NumericContract);
    }
    let environment_id = read_u32(bytes, 68);
    if environment_id != SIMPLE_EARTH_ENVIRONMENT_ID {
        return Err(ScenarioError::Environment);
    }

    let timestep = read_i32(bytes, 16);
    let steps = read_u32(bytes, 20);
    let telemetry_stride = read_u16(bytes, 24);
    let altitude = read_i32(bytes, 32);
    let velocity = read_i32(bytes, 36);
    let total_mass = read_i32(bytes, 40);
    let propellant = read_i32(bytes, 44);
    let dry_mass = read_i32(bytes, 48);
    let thrust = read_i32(bytes, 52);
    let mass_flow = read_i32(bytes, 56);
    let burn_duration = read_i32(bytes, 60);
    let cda = read_i32(bytes, 64);

    in_range(timestep, 1, MAX_TIMESTEP_Q16, ScenarioField::Timestep)?;
    if steps == 0 || steps > MAX_STEPS {
        return Err(ScenarioError::FieldRange(ScenarioField::Steps));
    }
    if telemetry_stride == 0 || telemetry_stride as u32 > steps {
        return Err(ScenarioError::FieldRange(ScenarioField::TelemetryStride));
    }
    in_range(
        altitude,
        MIN_ALTITUDE_Q12,
        MAX_ALTITUDE_Q12,
        ScenarioField::Altitude,
    )?;
    in_range(
        velocity,
        MIN_VELOCITY_Q24,
        MAX_VELOCITY_Q24,
        ScenarioField::Velocity,
    )?;
    in_range(total_mass, 1, MAX_MASS_Q12, ScenarioField::TotalMass)?;
    in_range(propellant, 0, MAX_MASS_Q12, ScenarioField::Propellant)?;
    in_range(dry_mass, 1, MAX_MASS_Q12, ScenarioField::DryMass)?;
    in_range(thrust, 0, MAX_THRUST_Q12, ScenarioField::Thrust)?;
    in_range(mass_flow, 0, MAX_MASS_FLOW_Q16, ScenarioField::MassFlow)?;
    in_range(
        burn_duration,
        0,
        MAX_DURATION_Q16 as i32,
        ScenarioField::BurnDuration,
    )?;
    in_range(cda, 0, MAX_CDA_Q16, ScenarioField::Cda)?;

    if total_mass < dry_mass || propellant > total_mass {
        return Err(ScenarioError::MassInvariant);
    }

    let duration = (timestep as u32)
        .checked_mul(steps)
        .ok_or(ScenarioError::Duration)?;
    if duration > MAX_DURATION_Q16 || burn_duration as u32 > duration {
        return Err(ScenarioError::Duration);
    }

    let mut numeric_status = NumericStatus::CLEAR;
    let thrust_acceleration = divide_scaled(thrust, dry_mass, 28, &mut numeric_status);
    let conservative_acceleration = add(
        thrust_acceleration,
        CONSERVATIVE_GRAVITY_Q28,
        &mut numeric_status,
    );
    if !numeric_status.is_clear() {
        return Err(ScenarioError::NumericFault);
    }
    if conservative_acceleration > MAX_ACCELERATION_Q28 {
        return Err(ScenarioError::AccelerationEnvelope);
    }

    Ok(Scenario {
        scenario_id: read_u32(bytes, 12),
        timestep: Time::from_raw(timestep),
        steps,
        telemetry_stride,
        flags: read_u16(bytes, 26),
        seed: read_u32(bytes, 28),
        initial: InitialConditions {
            altitude: Altitude::from_raw(altitude),
            velocity: Velocity::from_raw(velocity),
            total_mass: Mass::from_raw(total_mass),
            propellant: Mass::from_raw(propellant),
        },
        vehicle: VehicleConfig {
            dry_mass: Mass::from_raw(dry_mass),
            thrust: Force::from_raw(thrust),
            mass_flow: MassFlow::from_raw(mass_flow),
            burn_duration: Time::from_raw(burn_duration),
            cda: Cda::from_raw(cda),
        },
        environment_id,
    })
}
