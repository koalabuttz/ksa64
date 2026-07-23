//! Parser and validator for the fixed-capacity 884-byte Phase 2 scenario image.

use crate::aerodynamics::AeroTable;
use crate::guidance::{PitchKnot, PitchProgram};
use crate::numeric::{add, multiply_scaled, NumericStatus};
use crate::phase2_numeric::{
    EARTH_RADIUS_Q12, EARTH_ROTATION_RAD_Q30, PHASE2_ENVIRONMENT_ID, PHASE2_NUMERIC_CONTRACT_ID,
};
use crate::phase2_quantities::{
    DownrangeAngle, PitchAngle, PlanarVelocity, Radius, ReferenceArea, SpecificAngularMomentum,
};
use crate::planar::{PlanarTruthState, StagePhase};
use crate::quantities::{Force, Mass, MassFlow, Time};
use crate::scenario::{crc32_ieee, fnv1a_32};

pub const PHASE2_SCENARIO_IMAGE_LENGTH: usize = 884;
pub const PHASE2_SCENARIO_VERSION: u16 = 2;
pub const MAX_STAGES: usize = 4;
pub const MAX_PITCH_KNOTS: usize = 16;
pub const MAX_AERO_TABLES: usize = 4;
pub const MAX_AERO_KNOTS: usize = 16;

const MAGIC: [u8; 4] = *b"KSC2";
const HEADER_LENGTH: usize = 64;
const STAGE_RECORD_LENGTH: usize = 40;
const PITCH_RECORD_LENGTH: usize = 8;
const AERO_RECORD_LENGTH: usize = 132;
const PITCH_BASE: usize = HEADER_LENGTH + MAX_STAGES * STAGE_RECORD_LENGTH;
const AERO_BASE: usize = PITCH_BASE + MAX_PITCH_KNOTS * PITCH_RECORD_LENGTH;
const KNOWN_FLAGS: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2ScenarioError {
    Length,
    Magic,
    Version,
    RecordLength,
    Checksum,
    NumericContract,
    Environment,
    Reserved,
    Count,
    Flags,
    Timestep,
    Duration,
    InitialState,
    Stage,
    PitchProgram,
    AeroTable,
    MassInvariant,
    NumericFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageConfig {
    dry_mass: Mass,
    propellant_mass: Mass,
    thrust: Force,
    mass_flow: MassFlow,
    burn_steps: u32,
    separation_delay_steps: u16,
    ignition_delay_steps: u16,
    reference_area: ReferenceArea,
    aero_table_index: u8,
    separate: bool,
}

impl StageConfig {
    const EMPTY: Self = Self {
        dry_mass: Mass::ZERO,
        propellant_mass: Mass::ZERO,
        thrust: Force::ZERO,
        mass_flow: MassFlow::ZERO,
        burn_steps: 0,
        separation_delay_steps: 0,
        ignition_delay_steps: 0,
        reference_area: ReferenceArea::ZERO,
        aero_table_index: 0,
        separate: false,
    };

    pub const fn dry_mass(self) -> Mass {
        self.dry_mass
    }
    pub const fn propellant_mass(self) -> Mass {
        self.propellant_mass
    }
    pub const fn thrust(self) -> Force {
        self.thrust
    }
    pub const fn mass_flow(self) -> MassFlow {
        self.mass_flow
    }
    pub const fn burn_steps(self) -> u32 {
        self.burn_steps
    }
    pub const fn separation_delay_steps(self) -> u16 {
        self.separation_delay_steps
    }
    pub const fn ignition_delay_steps(self) -> u16 {
        self.ignition_delay_steps
    }
    pub const fn reference_area(self) -> ReferenceArea {
        self.reference_area
    }
    pub const fn aero_table_index(self) -> u8 {
        self.aero_table_index
    }
    pub const fn separate(self) -> bool {
        self.separate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2AeroTable {
    count: u8,
    mach_q16: [i32; MAX_AERO_KNOTS],
    cd_q14: [i32; MAX_AERO_KNOTS],
}

impl Phase2AeroTable {
    const EMPTY: Self = Self {
        count: 0,
        mach_q16: [0; MAX_AERO_KNOTS],
        cd_q14: [0; MAX_AERO_KNOTS],
    };
    pub fn table(&self) -> AeroTable<'_> {
        AeroTable::new(
            &self.mach_q16[..self.count as usize],
            &self.cd_q14[..self.count as usize],
        )
    }
    pub const fn count(&self) -> u8 {
        self.count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2Scenario {
    scenario_id: u32,
    timestep: Time,
    steps: u32,
    telemetry_stride: u16,
    flags: u8,
    payload_mass: Mass,
    initial_altitude_q12: i32,
    initial_radial_velocity: PlanarVelocity,
    initial_surface_relative_velocity: PlanarVelocity,
    stage_count: u8,
    stages: [StageConfig; MAX_STAGES],
    pitch_count: u8,
    pitch_knots: [PitchKnot; MAX_PITCH_KNOTS],
    aero_table_count: u8,
    aero_tables: [Phase2AeroTable; MAX_AERO_TABLES],
}

impl Phase2Scenario {
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
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    pub const fn payload_mass(&self) -> Mass {
        self.payload_mass
    }
    pub const fn stage_count(&self) -> u8 {
        self.stage_count
    }
    pub fn stage(&self, index: u8) -> Option<StageConfig> {
        if index < self.stage_count {
            Some(self.stages[index as usize])
        } else {
            None
        }
    }
    pub fn pitch_program(&self) -> PitchProgram<'_> {
        PitchProgram::new(&self.pitch_knots[..self.pitch_count as usize])
    }
    pub fn aero_table(&self, index: u8) -> Option<AeroTable<'_>> {
        if index < self.aero_table_count {
            Some(self.aero_tables[index as usize].table())
        } else {
            None
        }
    }

    pub fn initial_truth(&self, status: &mut NumericStatus) -> Option<PlanarTruthState> {
        let radius_raw = add(EARTH_RADIUS_Q12, self.initial_altitude_q12, status);
        let atmosphere_velocity = multiply_scaled(EARTH_ROTATION_RAD_Q30, radius_raw, 18, status);
        let tangential_velocity = add(
            atmosphere_velocity,
            self.initial_surface_relative_velocity.raw(),
            status,
        );
        let angular_momentum = multiply_scaled(tangential_velocity, radius_raw, 22, status);
        let mut total_mass = self.payload_mass.raw();
        let mut index = 0;
        while index < self.stage_count as usize {
            total_mass = add(total_mass, self.stages[index].dry_mass.raw(), status);
            total_mass = add(total_mass, self.stages[index].propellant_mass.raw(), status);
            index += 1;
        }
        if !status.is_clear() {
            return None;
        }
        let first = self.stages[0];
        Some(PlanarTruthState::new(
            0,
            Time::ZERO,
            Radius::from_raw(radius_raw),
            DownrangeAngle::ZERO,
            self.initial_radial_velocity,
            SpecificAngularMomentum::from_raw(angular_momentum),
            Mass::from_raw(total_mass),
            first.propellant_mass,
            0,
            if first.ignition_delay_steps == 0 {
                StagePhase::Burning
            } else {
                StagePhase::CoastBeforeIgnition
            },
        ))
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

pub fn parse_phase2_scenario(bytes: &[u8]) -> Result<Phase2Scenario, Phase2ScenarioError> {
    if bytes.len() != PHASE2_SCENARIO_IMAGE_LENGTH {
        return Err(Phase2ScenarioError::Length);
    }
    if bytes[..4] != MAGIC {
        return Err(Phase2ScenarioError::Magic);
    }
    if read_u16(bytes, 4) != PHASE2_SCENARIO_VERSION {
        return Err(Phase2ScenarioError::Version);
    }
    if read_u16(bytes, 6) as usize != PHASE2_SCENARIO_IMAGE_LENGTH {
        return Err(Phase2ScenarioError::RecordLength);
    }
    if crc32_ieee(&bytes[..PHASE2_SCENARIO_IMAGE_LENGTH - 4])
        != read_u32(bytes, PHASE2_SCENARIO_IMAGE_LENGTH - 4)
    {
        return Err(Phase2ScenarioError::Checksum);
    }
    if read_u32(bytes, 8) != PHASE2_NUMERIC_CONTRACT_ID {
        return Err(Phase2ScenarioError::NumericContract);
    }
    if read_u32(bytes, 16) != PHASE2_ENVIRONMENT_ID {
        return Err(Phase2ScenarioError::Environment);
    }
    if bytes[33] & !KNOWN_FLAGS != 0 {
        return Err(Phase2ScenarioError::Flags);
    }
    if read_u16(bytes, 34) != 0 || bytes[52..64].iter().any(|byte| *byte != 0) {
        return Err(Phase2ScenarioError::Reserved);
    }

    let timestep = Time::from_raw(read_i32(bytes, 20));
    let steps = read_u32(bytes, 24);
    let telemetry_stride = read_u16(bytes, 28);
    let stage_count = bytes[30];
    let pitch_count = bytes[31];
    let aero_table_count = bytes[32];
    if timestep.raw() <= 0 || timestep.raw() > 8_192 {
        return Err(Phase2ScenarioError::Timestep);
    }
    if steps == 0 || steps > 32_768 || telemetry_stride == 0 || telemetry_stride as u32 > steps {
        return Err(Phase2ScenarioError::Duration);
    }
    if !(1..=MAX_STAGES as u8).contains(&stage_count)
        || !(2..=MAX_PITCH_KNOTS as u8).contains(&pitch_count)
        || !(1..=MAX_AERO_TABLES as u8).contains(&aero_table_count)
    {
        return Err(Phase2ScenarioError::Count);
    }
    let payload_mass = Mass::from_raw(read_i32(bytes, 36));
    let initial_altitude_q12 = read_i32(bytes, 40);
    let initial_radial_velocity = PlanarVelocity::from_raw(read_i32(bytes, 44));
    let initial_surface_relative_velocity = PlanarVelocity::from_raw(read_i32(bytes, 48));
    if payload_mass.raw() <= 0
        || payload_mass.raw() > 20_480_000
        || !(-8_192..=8_192_000).contains(&initial_altitude_q12)
        || !(-268_435_456..=268_435_456).contains(&initial_radial_velocity.raw())
        || !(-268_435_456..=268_435_456).contains(&initial_surface_relative_velocity.raw())
    {
        return Err(Phase2ScenarioError::InitialState);
    }

    let mut status = NumericStatus::CLEAR;
    let mut stages = [StageConfig::EMPTY; MAX_STAGES];
    let mut index = 0usize;
    while index < MAX_STAGES {
        let offset = HEADER_LENGTH + index * STAGE_RECORD_LENGTH;
        if index < stage_count as usize {
            let config = StageConfig {
                dry_mass: Mass::from_raw(read_i32(bytes, offset)),
                propellant_mass: Mass::from_raw(read_i32(bytes, offset + 4)),
                thrust: Force::from_raw(read_i32(bytes, offset + 8)),
                mass_flow: MassFlow::from_raw(read_i32(bytes, offset + 12)),
                burn_steps: read_u32(bytes, offset + 16),
                separation_delay_steps: read_u16(bytes, offset + 20),
                ignition_delay_steps: read_u16(bytes, offset + 22),
                reference_area: ReferenceArea::from_raw(read_i32(bytes, offset + 24)),
                aero_table_index: bytes[offset + 28],
                separate: bytes[offset + 29] == 1,
            };
            if bytes[offset + 29] > 1
                || bytes[offset + 30..offset + STAGE_RECORD_LENGTH]
                    .iter()
                    .any(|byte| *byte != 0)
                || config.dry_mass.raw() <= 0
                || config.dry_mass.raw() > 20_480_000
                || config.propellant_mass.raw() < 0
                || config.propellant_mass.raw() > 20_480_000
                || config.thrust.raw() <= 0
                || config.thrust.raw() > 4_096_000
                || config.mass_flow.raw() <= 0
                || config.mass_flow.raw() > 6_553_600
                || config.burn_steps == 0
                || config.burn_steps > steps
                || config.separation_delay_steps as u32 > steps
                || config.ignition_delay_steps as u32 > steps
                || config.reference_area.raw() <= 0
                || config.reference_area.raw() > 131_072_000
                || config.aero_table_index >= aero_table_count
                || (index + 1 < stage_count as usize && !config.separate)
                || (index + 1 == stage_count as usize && config.separate)
            {
                return Err(Phase2ScenarioError::Stage);
            }
            let per_step = multiply_scaled(config.mass_flow.raw(), timestep.raw(), 20, &mut status);
            let planned = multiply_scaled(per_step, config.burn_steps as i32, 0, &mut status);
            if planned > add(config.propellant_mass.raw(), per_step, &mut status) {
                return Err(Phase2ScenarioError::MassInvariant);
            }
            stages[index] = config;
        } else if bytes[offset..offset + STAGE_RECORD_LENGTH]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Phase2ScenarioError::Reserved);
        }
        index += 1;
    }

    let mut pitch_knots = [PitchKnot::new(Time::ZERO, PitchAngle::RADIAL); MAX_PITCH_KNOTS];
    index = 0;
    while index < MAX_PITCH_KNOTS {
        let offset = PITCH_BASE + index * PITCH_RECORD_LENGTH;
        if index < pitch_count as usize {
            if read_u16(bytes, offset + 6) != 0 {
                return Err(Phase2ScenarioError::Reserved);
            }
            pitch_knots[index] = PitchKnot::new(
                Time::from_raw(read_i32(bytes, offset)),
                PitchAngle::from_raw(read_u16(bytes, offset + 4)),
            );
        } else if bytes[offset..offset + PITCH_RECORD_LENGTH]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Phase2ScenarioError::Reserved);
        }
        index += 1;
    }
    if !PitchProgram::new(&pitch_knots[..pitch_count as usize]).is_valid(timestep) {
        return Err(Phase2ScenarioError::PitchProgram);
    }

    let mut aero_tables = [Phase2AeroTable::EMPTY; MAX_AERO_TABLES];
    index = 0;
    while index < MAX_AERO_TABLES {
        let offset = AERO_BASE + index * AERO_RECORD_LENGTH;
        if index < aero_table_count as usize {
            let count = bytes[offset];
            if !(2..=MAX_AERO_KNOTS as u8).contains(&count)
                || bytes[offset + 1..offset + 4].iter().any(|byte| *byte != 0)
            {
                return Err(Phase2ScenarioError::AeroTable);
            }
            let mut table = Phase2AeroTable::EMPTY;
            table.count = count;
            let mut knot = 0usize;
            while knot < MAX_AERO_KNOTS {
                let knot_offset = offset + 4 + knot * 8;
                let mach = read_i32(bytes, knot_offset);
                let cd = read_i32(bytes, knot_offset + 4);
                if knot < count as usize {
                    table.mach_q16[knot] = mach;
                    table.cd_q14[knot] = cd;
                } else if mach != 0 || cd != 0 {
                    return Err(Phase2ScenarioError::Reserved);
                }
                knot += 1;
            }
            if !table.table().is_valid() {
                return Err(Phase2ScenarioError::AeroTable);
            }
            aero_tables[index] = table;
        } else if bytes[offset..offset + AERO_RECORD_LENGTH]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Phase2ScenarioError::Reserved);
        }
        index += 1;
    }
    if !status.is_clear() {
        return Err(Phase2ScenarioError::NumericFault);
    }

    let scenario = Phase2Scenario {
        scenario_id: read_u32(bytes, 12),
        timestep,
        steps,
        telemetry_stride,
        flags: bytes[33],
        payload_mass,
        initial_altitude_q12,
        initial_radial_velocity,
        initial_surface_relative_velocity,
        stage_count,
        stages,
        pitch_count,
        pitch_knots,
        aero_table_count,
        aero_tables,
    };
    let mut initial_status = NumericStatus::CLEAR;
    if scenario.initial_truth(&mut initial_status).is_none() || !initial_status.is_clear() {
        return Err(Phase2ScenarioError::MassInvariant);
    }
    Ok(scenario)
}

pub const KSA2A_NOMINAL_SCENARIO_ID: u32 = fnv1a_32(b"ksa64.phase2.ksa2a-200km.v1");
pub const KSA2A_EARLY_CUTOFF_SCENARIO_ID: u32 = fnv1a_32(b"ksa64.phase2.ksa2a-early-cutoff.v1");
#[cfg(feature = "fixtures")]
mod fixture_data {
    include!("../../phase2/generated/mission_v1.rs");
}

#[cfg(feature = "fixtures")]
const fn fixture_stage(index: usize, burn_steps: u32) -> StageConfig {
    StageConfig {
        dry_mass: Mass::from_raw(fixture_data::STAGE_DRY_Q12[index]),
        propellant_mass: Mass::from_raw(fixture_data::STAGE_PROPELLANT_Q12[index]),
        thrust: Force::from_raw(fixture_data::STAGE_THRUST_Q12[index]),
        mass_flow: MassFlow::from_raw(fixture_data::STAGE_MASS_FLOW_Q16[index]),
        burn_steps,
        separation_delay_steps: fixture_data::SEPARATION_STEPS[index],
        ignition_delay_steps: fixture_data::IGNITION_STEPS[index],
        reference_area: ReferenceArea::from_raw(fixture_data::STAGE_AREA_Q16[index]),
        aero_table_index: fixture_data::AERO_INDEX[index],
        separate: fixture_data::SEPARATE[index],
    }
}

#[cfg(feature = "fixtures")]
const fn fixture_scenario(
    scenario_id: u32,
    flags: u8,
    burn_steps: [u32; 2],
    mission_steps: u32,
) -> Phase2Scenario {
    Phase2Scenario {
        scenario_id,
        timestep: Time::from_raw(fixture_data::TIMESTEP_Q16),
        steps: mission_steps,
        telemetry_stride: fixture_data::TELEMETRY_STRIDE,
        flags,
        payload_mass: Mass::from_raw(fixture_data::PAYLOAD_Q12),
        initial_altitude_q12: 0,
        initial_radial_velocity: PlanarVelocity::ZERO,
        initial_surface_relative_velocity: PlanarVelocity::ZERO,
        stage_count: 2,
        stages: [
            fixture_stage(0, burn_steps[0]),
            fixture_stage(1, burn_steps[1]),
            StageConfig::EMPTY,
            StageConfig::EMPTY,
        ],
        pitch_count: 8,
        pitch_knots: [
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[0]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[0]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[1]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[1]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[2]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[2]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[3]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[3]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[4]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[4]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[5]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[5]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[6]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[6]),
            ),
            PitchKnot::new(
                Time::from_raw(fixture_data::PITCH_TIME_Q16[7]),
                PitchAngle::from_raw(fixture_data::PITCH_ANGLE[7]),
            ),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
            PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
        ],
        aero_table_count: 2,
        aero_tables: [
            Phase2AeroTable {
                count: fixture_data::AERO_COUNT[0],
                mach_q16: fixture_data::AERO_0_MACH_Q16,
                cd_q14: fixture_data::AERO_0_CD_Q14,
            },
            Phase2AeroTable {
                count: fixture_data::AERO_COUNT[1],
                mach_q16: fixture_data::AERO_1_MACH_Q16,
                cd_q14: fixture_data::AERO_1_CD_Q14,
            },
            Phase2AeroTable::EMPTY,
            Phase2AeroTable::EMPTY,
        ],
    }
}

#[cfg(feature = "fixtures")]
static KSA2A_NOMINAL_FIXTURE: Phase2Scenario = fixture_scenario(
    fixture_data::NOMINAL_SCENARIO_ID,
    0,
    fixture_data::NOMINAL_BURN_STEPS,
    fixture_data::MISSION_STEPS,
);

#[cfg(feature = "fixtures")]
static KSA2A_FAILURE_FIXTURE: Phase2Scenario = fixture_scenario(
    fixture_data::FAILURE_SCENARIO_ID,
    1,
    fixture_data::FAILURE_BURN_STEPS,
    3_076,
);

#[cfg(feature = "fixtures")]
pub fn ksa2a_fixture(failure: bool) -> &'static Phase2Scenario {
    if failure {
        &KSA2A_FAILURE_FIXTURE
    } else {
        &KSA2A_NOMINAL_FIXTURE
    }
}
