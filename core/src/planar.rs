//! Phase 2 rotating-Earth planar vacuum dynamics and orbital classification.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::phase2_numeric::{
    sqrt_floor_u32, EARTH_MU_Q12, EARTH_RADIUS_Q12, EARTH_ROTATION_TURNS_Q32, INV_TWO_PI_Q30,
};
use crate::phase2_quantities::{
    DownrangeAngle, Eccentricity, PlanarAcceleration, PlanarVelocity, Radius,
    SpecificAngularMomentum, SpecificEnergy,
};
use crate::quantities::{Mass, Time};

const MIN_RADIUS_Q12: i32 = 26_116_096;
const MAX_RADIUS_Q12: i32 = 34_320_384;
const MAX_VELOCITY_Q24: i32 = 268_435_456;
const MAX_ACCELERATION_Q28: i32 = 53_687_091;
const ATMOSPHERE_TOP_Q12: i32 = 491_520;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StagePhase {
    CoastBeforeIgnition,
    Burning,
    CoastBeforeSeparation,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanarTruthState {
    step: u32,
    time: Time,
    radius: Radius,
    downrange: DownrangeAngle,
    radial_velocity: PlanarVelocity,
    specific_angular_momentum: SpecificAngularMomentum,
    radial_acceleration: PlanarAcceleration,
    tangential_acceleration: PlanarAcceleration,
    total_mass: Mass,
    active_propellant: Mass,
    active_stage: u8,
    stage_phase: StagePhase,
}

impl PlanarTruthState {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        step: u32,
        time: Time,
        radius: Radius,
        downrange: DownrangeAngle,
        radial_velocity: PlanarVelocity,
        specific_angular_momentum: SpecificAngularMomentum,
        total_mass: Mass,
        active_propellant: Mass,
        active_stage: u8,
        stage_phase: StagePhase,
    ) -> Self {
        Self {
            step,
            time,
            radius,
            downrange,
            radial_velocity,
            specific_angular_momentum,
            radial_acceleration: PlanarAcceleration::ZERO,
            tangential_acceleration: PlanarAcceleration::ZERO,
            total_mass,
            active_propellant,
            active_stage,
            stage_phase,
        }
    }

    pub const fn step(self) -> u32 {
        self.step
    }
    pub const fn time(self) -> Time {
        self.time
    }
    pub const fn radius(self) -> Radius {
        self.radius
    }
    pub const fn downrange(self) -> DownrangeAngle {
        self.downrange
    }
    pub const fn radial_velocity(self) -> PlanarVelocity {
        self.radial_velocity
    }
    pub const fn specific_angular_momentum(self) -> SpecificAngularMomentum {
        self.specific_angular_momentum
    }
    pub const fn radial_acceleration(self) -> PlanarAcceleration {
        self.radial_acceleration
    }
    pub const fn tangential_acceleration(self) -> PlanarAcceleration {
        self.tangential_acceleration
    }
    pub const fn total_mass(self) -> Mass {
        self.total_mass
    }
    pub const fn active_propellant(self) -> Mass {
        self.active_propellant
    }
    pub const fn active_stage(self) -> u8 {
        self.active_stage
    }
    pub const fn stage_phase(self) -> StagePhase {
        self.stage_phase
    }

    /// Replace only the discrete vehicle bookkeeping while preserving the
    /// continuous successor state and its acceleration snapshot.
    pub const fn with_vehicle_state(
        self,
        total_mass: Mass,
        active_propellant: Mass,
        active_stage: u8,
        stage_phase: StagePhase,
    ) -> Self {
        Self {
            total_mass,
            active_propellant,
            active_stage,
            stage_phase,
            ..self
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn successor(
        self,
        step: u32,
        time: Time,
        radius: Radius,
        downrange: DownrangeAngle,
        radial_velocity: PlanarVelocity,
        specific_angular_momentum: SpecificAngularMomentum,
        radial_acceleration: PlanarAcceleration,
        tangential_acceleration: PlanarAcceleration,
        total_mass: Mass,
        active_propellant: Mass,
        active_stage: u8,
        stage_phase: StagePhase,
    ) -> Self {
        Self {
            step,
            time,
            radius,
            downrange,
            radial_velocity,
            specific_angular_momentum,
            radial_acceleration,
            tangential_acceleration,
            total_mass,
            active_propellant,
            active_stage,
            stage_phase,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanarWorld {
    timestep: Time,
    radius: Radius,
    mu_q12: i32,
    rotation_turns_q32: u32,
}

impl PlanarWorld {
    pub const fn simple_earth(timestep: Time) -> Self {
        Self {
            timestep,
            radius: Radius::from_raw(EARTH_RADIUS_Q12),
            mu_q12: EARTH_MU_Q12,
            rotation_turns_q32: EARTH_ROTATION_TURNS_Q32,
        }
    }

    pub const fn timestep(self) -> Time {
        self.timestep
    }
    pub const fn radius(self) -> Radius {
        self.radius
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VacuumSnapshot {
    tangential_velocity: PlanarVelocity,
    gravity: PlanarAcceleration,
    centrifugal: PlanarAcceleration,
    radial_acceleration: PlanarAcceleration,
    earth_relative_turn_rate_q30: i32,
}

impl VacuumSnapshot {
    pub const fn tangential_velocity(self) -> PlanarVelocity {
        self.tangential_velocity
    }
    pub const fn gravity(self) -> PlanarAcceleration {
        self.gravity
    }
    pub const fn centrifugal(self) -> PlanarAcceleration {
        self.centrifugal
    }
    pub const fn radial_acceleration(self) -> PlanarAcceleration {
        self.radial_acceleration
    }
    pub const fn earth_relative_turn_rate_q30(self) -> i32 {
        self.earth_relative_turn_rate_q30
    }
}

pub fn evaluate_vacuum(
    world: PlanarWorld,
    truth: PlanarTruthState,
    status: &mut NumericStatus,
) -> VacuumSnapshot {
    let radius = truth.radius().raw();
    let tangential_velocity =
        divide_scaled(truth.specific_angular_momentum().raw(), radius, 22, status);
    let mu_over_r_q24 = divide_scaled(world.mu_q12, radius, 24, status);
    let gravity = divide_scaled(mu_over_r_q24, radius, 16, status);
    let tangential_squared_q20 =
        multiply_scaled(tangential_velocity, tangential_velocity, 28, status);
    let centrifugal = divide_scaled(tangential_squared_q20, radius, 20, status);
    let radial_acceleration = subtract(centrifugal, gravity, status);

    let inertial_rate_rad_q30 = divide_scaled(tangential_velocity, radius, 18, status);
    let inertial_rate_turns_q30 =
        multiply_scaled(inertial_rate_rad_q30, INV_TWO_PI_Q30, 30, status);
    let rotation_q30 = ((world.rotation_turns_q32 + 2) >> 2) as i32;
    let earth_relative_turn_rate_q30 = subtract(inertial_rate_turns_q30, rotation_q30, status);

    if !(MIN_RADIUS_Q12..=MAX_RADIUS_Q12).contains(&radius)
        || tangential_velocity.abs() > MAX_VELOCITY_Q24
        || radial_acceleration.abs() > MAX_ACCELERATION_Q28
    {
        status.record(NumericFault::InvalidInput);
    }

    VacuumSnapshot {
        tangential_velocity: PlanarVelocity::from_raw(tangential_velocity),
        gravity: PlanarAcceleration::from_raw(gravity),
        centrifugal: PlanarAcceleration::from_raw(centrifugal),
        radial_acceleration: PlanarAcceleration::from_raw(radial_acceleration),
        earth_relative_turn_rate_q30,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanarStepError {
    NumericFault,
}

fn update_downrange(
    current: DownrangeAngle,
    turn_rate_q30: i32,
    timestep: Time,
    status: &mut NumericStatus,
) -> DownrangeAngle {
    let delta_q32 = multiply_scaled(turn_rate_q30, timestep.raw(), 14, status);
    current.wrapping_add_raw(delta_q32)
}

pub fn advance_vacuum_semi_implicit(
    world: PlanarWorld,
    truth: PlanarTruthState,
    status: &mut NumericStatus,
) -> Result<PlanarTruthState, PlanarStepError> {
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    let forces = evaluate_vacuum(world, truth, status);
    let timestep = world.timestep().raw();
    let delta_velocity = multiply_scaled(forces.radial_acceleration().raw(), timestep, 20, status);
    let radial_velocity = add(truth.radial_velocity().raw(), delta_velocity, status);
    let delta_radius = multiply_scaled(radial_velocity, timestep, 28, status);
    let radius = add(truth.radius().raw(), delta_radius, status);
    let updated = PlanarTruthState::new(
        truth.step(),
        truth.time(),
        Radius::from_raw(radius),
        truth.downrange(),
        PlanarVelocity::from_raw(radial_velocity),
        truth.specific_angular_momentum(),
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    );
    let angular = evaluate_vacuum(world, updated, status);
    let downrange = update_downrange(
        truth.downrange(),
        angular.earth_relative_turn_rate_q30(),
        world.timestep(),
        status,
    );
    let time = add(truth.time().raw(), timestep, status);
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    Ok(truth.successor(
        truth.step() + 1,
        Time::from_raw(time),
        Radius::from_raw(radius),
        downrange,
        PlanarVelocity::from_raw(radial_velocity),
        truth.specific_angular_momentum(),
        forces.radial_acceleration(),
        PlanarAcceleration::ZERO,
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    ))
}

pub fn advance_vacuum_midpoint(
    world: PlanarWorld,
    truth: PlanarTruthState,
    status: &mut NumericStatus,
) -> Result<PlanarTruthState, PlanarStepError> {
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    let first = evaluate_vacuum(world, truth, status);
    let half_timestep = Time::from_raw(world.timestep().raw() >> 1);
    let mid_velocity = add(
        truth.radial_velocity().raw(),
        multiply_scaled(
            first.radial_acceleration().raw(),
            half_timestep.raw(),
            20,
            status,
        ),
        status,
    );
    let mid_radius = add(
        truth.radius().raw(),
        multiply_scaled(
            truth.radial_velocity().raw(),
            half_timestep.raw(),
            28,
            status,
        ),
        status,
    );
    let midpoint = PlanarTruthState::new(
        truth.step(),
        truth.time(),
        Radius::from_raw(mid_radius),
        truth.downrange(),
        PlanarVelocity::from_raw(mid_velocity),
        truth.specific_angular_momentum(),
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    );
    let middle = evaluate_vacuum(world, midpoint, status);
    let timestep = world.timestep().raw();
    let radial_velocity = add(
        truth.radial_velocity().raw(),
        multiply_scaled(middle.radial_acceleration().raw(), timestep, 20, status),
        status,
    );
    let radius = add(
        truth.radius().raw(),
        multiply_scaled(mid_velocity, timestep, 28, status),
        status,
    );
    let downrange = update_downrange(
        truth.downrange(),
        middle.earth_relative_turn_rate_q30(),
        world.timestep(),
        status,
    );
    let time = add(truth.time().raw(), timestep, status);
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    Ok(truth.successor(
        truth.step() + 1,
        Time::from_raw(time),
        Radius::from_raw(radius),
        downrange,
        PlanarVelocity::from_raw(radial_velocity),
        truth.specific_angular_momentum(),
        middle.radial_acceleration(),
        PlanarAcceleration::ZERO,
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OrbitClass {
    Impact,
    Suborbital,
    StableOrbit,
    Escape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrbitSolution {
    class: OrbitClass,
    specific_energy: SpecificEnergy,
    eccentricity: Eccentricity,
    perigee: Radius,
    apogee: Radius,
}

impl OrbitSolution {
    pub const fn class(self) -> OrbitClass {
        self.class
    }
    pub const fn specific_energy(self) -> SpecificEnergy {
        self.specific_energy
    }
    pub const fn eccentricity(self) -> Eccentricity {
        self.eccentricity
    }
    pub const fn perigee(self) -> Radius {
        self.perigee
    }
    pub const fn apogee(self) -> Radius {
        self.apogee
    }
}

pub fn classify_orbit(
    world: PlanarWorld,
    truth: PlanarTruthState,
    status: &mut NumericStatus,
) -> Option<OrbitSolution> {
    let vacuum = evaluate_vacuum(world, truth, status);
    let vr2_q20 = multiply_scaled(
        truth.radial_velocity().raw(),
        truth.radial_velocity().raw(),
        28,
        status,
    );
    let vt2_q20 = multiply_scaled(
        vacuum.tangential_velocity().raw(),
        vacuum.tangential_velocity().raw(),
        28,
        status,
    );
    let speed2_q20 = add(vr2_q20, vt2_q20, status);
    let kinetic_q24 = (speed2_q20 >> 1).checked_shl(4).unwrap_or(i32::MAX);
    let potential_q24 = divide_scaled(world.mu_q12, truth.radius().raw(), 24, status);
    let energy_q24 = subtract(kinetic_q24, potential_q24, status);
    if !status.is_clear() {
        return None;
    }
    if energy_q24 >= 0 {
        return Some(OrbitSolution {
            class: OrbitClass::Escape,
            specific_energy: SpecificEnergy::from_raw(energy_q24),
            eccentricity: Eccentricity::from_raw(1 << 16),
            perigee: truth.radius(),
            apogee: Radius::from_raw(i32::MAX),
        });
    }

    let vh_q12 = multiply_scaled(
        vacuum.tangential_velocity().raw(),
        truth.specific_angular_momentum().raw(),
        26,
        status,
    );
    let radial_h_q12 = multiply_scaled(
        truth.radial_velocity().raw(),
        truth.specific_angular_momentum().raw(),
        26,
        status,
    );
    let e_cos_q28 = subtract(
        divide_scaled(vh_q12, world.mu_q12, 28, status),
        1 << 28,
        status,
    );
    let e_sin_q28 = divide_scaled(radial_h_q12, world.mu_q12, 28, status);
    let e_cos2_q28 = multiply_scaled(e_cos_q28, e_cos_q28, 28, status);
    let e_sin2_q28 = multiply_scaled(e_sin_q28, e_sin_q28, 28, status);
    let e2_q16 = (add(e_cos2_q28, e_sin2_q28, status) + (1 << 11)) >> 12;
    let eccentricity_q16 = sqrt_floor_u32((e2_q16.max(0) as u32) << 16) as i32;
    let twice_energy = energy_q24.checked_mul(2).unwrap_or(i32::MIN);
    let semi_major_q12 = divide_scaled(-world.mu_q12, twice_energy, 24, status);
    let perigee_q12 = multiply_scaled(semi_major_q12, (1 << 16) - eccentricity_q16, 16, status);
    let apogee_q12 = multiply_scaled(semi_major_q12, (1 << 16) + eccentricity_q16, 16, status);
    if !status.is_clear() {
        return None;
    }
    let atmosphere_radius = add(world.radius.raw(), ATMOSPHERE_TOP_Q12, status);
    let class = if perigee_q12 <= world.radius.raw() {
        OrbitClass::Impact
    } else if perigee_q12 < atmosphere_radius {
        OrbitClass::Suborbital
    } else {
        OrbitClass::StableOrbit
    };
    Some(OrbitSolution {
        class,
        specific_energy: SpecificEnergy::from_raw(energy_q24),
        eccentricity: Eccentricity::from_raw(eccentricity_q16),
        perigee: Radius::from_raw(perigee_q12),
        apogee: Radius::from_raw(apogee_q12),
    })
}
