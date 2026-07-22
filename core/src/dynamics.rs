//! Pure Phase 1 vertical-force evaluation. State integration is intentionally absent.

use crate::environment::EnvironmentSample;
use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::quantities::{Acceleration, Force, NetForce};
use crate::scenario::VehicleConfig;
use crate::vehicle::VerticalTruthState;

const MAX_NET_FORCE_Q12: i32 = 2_048_000;
const MAX_ACCELERATION_Q28: i32 = 26_843_546;

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
