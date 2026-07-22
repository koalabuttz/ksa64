//! Pure Phase 1 vertical-force evaluation and checked single-step integration.

use crate::environment::{EnvironmentSample, SimpleEarthEnvironment};
use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::quantities::{Acceleration, Altitude, Force, Mass, NetForce, Time, Velocity};
use crate::scenario::{Scenario, VehicleConfig};
use crate::vehicle::VerticalTruthState;

const MAX_NET_FORCE_Q12: i32 = 2_048_000;
const MAX_ACCELERATION_Q28: i32 = 26_843_546;
const MIN_ALTITUDE_Q12: i32 = -8_192;
const MAX_ALTITUDE_Q12: i32 = 8_192_000;
const MIN_VELOCITY_Q24: i32 = -134_217_728;
const MAX_VELOCITY_Q24: i32 = 134_217_728;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalForceSnapshot {
    engine_active: bool,
    thrust: Force,
    weight: Force,
    drag: Force,
    net_force: NetForce,
    acceleration: Acceleration,
}

impl VerticalForceSnapshot {
    pub const fn engine_active(self) -> bool {
        self.engine_active
    }

    pub const fn thrust(self) -> Force {
        self.thrust
    }

    /// Downward gravitational-force magnitude.
    pub const fn weight(self) -> Force {
        self.weight
    }

    /// Signed aerodynamic force along the positive-up vertical axis.
    pub const fn drag(self) -> Force {
        self.drag
    }

    pub const fn net_force(self) -> NetForce {
        self.net_force
    }

    pub const fn acceleration(self) -> Acceleration {
        self.acceleration
    }
}

#[inline]
fn velocity_magnitude(value: i32, status: &mut NumericStatus) -> i32 {
    if value == i32::MIN {
        status.record(NumericFault::InvalidInput);
        0
    } else if value < 0 {
        -value
    } else {
        value
    }
}

#[inline]
fn halve_nonnegative(value: i32, status: &mut NumericStatus) -> i32 {
    if value < 0 {
        status.record(NumericFault::InvalidInput);
        0
    } else {
        (value >> 1) + (value & 1)
    }
}

pub fn evaluate_vertical_forces(
    vehicle: &VehicleConfig,
    truth: &VerticalTruthState,
    environment: EnvironmentSample,
    status: &mut NumericStatus,
) -> VerticalForceSnapshot {
    let velocity = truth.velocity().raw();
    let speed = velocity_magnitude(velocity, status);
    let speed_squared = multiply_scaled(speed, speed, 28, status);
    let density_speed_squared =
        multiply_scaled(environment.density().raw(), speed_squared, 28, status);
    let twice_drag = multiply_scaled(density_speed_squared, vehicle.cda().raw(), 24, status);
    let drag_magnitude = halve_nonnegative(twice_drag, status);
    let drag = if velocity > 0 {
        subtract(0, drag_magnitude, status)
    } else if velocity < 0 {
        drag_magnitude
    } else {
        0
    };

    let weight = multiply_scaled(
        truth.total_mass().raw(),
        environment.gravity().raw(),
        28,
        status,
    );
    let engine_active = truth.propellant().raw() > 0 && truth.time() < vehicle.burn_duration();
    let thrust = if engine_active {
        vehicle.thrust().raw()
    } else {
        0
    };
    let thrust_minus_weight = subtract(thrust, weight, status);
    let net_force = add(thrust_minus_weight, drag, status);
    let acceleration = divide_scaled(net_force, truth.total_mass().raw(), 28, status);
    if !(-MAX_NET_FORCE_Q12..=MAX_NET_FORCE_Q12).contains(&net_force)
        || !(-MAX_ACCELERATION_Q28..=MAX_ACCELERATION_Q28).contains(&acceleration)
    {
        status.record(NumericFault::InvalidInput);
    }

    VerticalForceSnapshot {
        engine_active,
        thrust: Force::from_raw(thrust),
        weight: Force::from_raw(weight),
        drag: Force::from_raw(drag),
        net_force: NetForce::from_raw(net_force),
        acceleration: Acceleration::from_raw(acceleration),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalStepError {
    NumericFault,
    ScenarioComplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalStepResult {
    truth: VerticalTruthState,
    forces: VerticalForceSnapshot,
    propellant_consumed: Mass,
    engine_cutoff: bool,
}

impl VerticalStepResult {
    pub const fn truth(self) -> VerticalTruthState {
        self.truth
    }

    pub const fn forces(self) -> VerticalForceSnapshot {
        self.forces
    }

    pub const fn propellant_consumed(self) -> Mass {
        self.propellant_consumed
    }

    pub const fn engine_cutoff(self) -> bool {
        self.engine_cutoff
    }
}

pub fn advance_vertical_state(
    scenario: &Scenario,
    environment: SimpleEarthEnvironment,
    truth: &VerticalTruthState,
    status: &mut NumericStatus,
) -> Result<VerticalStepResult, VerticalStepError> {
    if !status.is_clear() {
        return Err(VerticalStepError::NumericFault);
    }
    if truth.step() >= scenario.steps() {
        return Err(VerticalStepError::ScenarioComplete);
    }

    let sample = environment.sample(truth.altitude(), status);
    let forces = evaluate_vertical_forces(scenario.vehicle(), truth, sample, status);
    if !status.is_clear() {
        return Err(VerticalStepError::NumericFault);
    }

    let timestep = scenario.timestep().raw();
    let delta_velocity = multiply_scaled(forces.acceleration().raw(), timestep, 20, status);
    let velocity = add(truth.velocity().raw(), delta_velocity, status);
    let delta_altitude = multiply_scaled(velocity, timestep, 28, status);
    let altitude = add(truth.altitude().raw(), delta_altitude, status);
    let time = add(truth.time().raw(), timestep, status);

    let requested_propellant = if forces.engine_active() {
        multiply_scaled(scenario.vehicle().mass_flow().raw(), timestep, 20, status)
    } else {
        0
    };
    let consumed = requested_propellant.min(truth.propellant().raw());
    let propellant = subtract(truth.propellant().raw(), consumed, status);
    let total_mass = subtract(truth.total_mass().raw(), consumed, status);

    if !(MIN_ALTITUDE_Q12..=MAX_ALTITUDE_Q12).contains(&altitude)
        || !(MIN_VELOCITY_Q24..=MAX_VELOCITY_Q24).contains(&velocity)
        || total_mass < scenario.vehicle().dry_mass().raw()
        || propellant < 0
        || propellant > total_mass
    {
        status.record(NumericFault::InvalidInput);
    }
    if !status.is_clear() {
        return Err(VerticalStepError::NumericFault);
    }

    let engine_cutoff = forces.engine_active()
        && (propellant == 0 || time >= scenario.vehicle().burn_duration().raw());
    let successor = VerticalTruthState::successor(
        truth.step() + 1,
        Time::from_raw(time),
        Altitude::from_raw(altitude),
        Velocity::from_raw(velocity),
        forces.acceleration(),
        Mass::from_raw(total_mass),
        Mass::from_raw(propellant),
    );
    Ok(VerticalStepResult {
        truth: successor,
        forces,
        propellant_consumed: Mass::from_raw(consumed),
        engine_cutoff,
    })
}
