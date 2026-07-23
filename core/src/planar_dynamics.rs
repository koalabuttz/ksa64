//! Pure Phase 2 force resolution and one-step planar integration.

use crate::aerodynamics::AeroSnapshot;
use crate::numeric::{add, divide_scaled, multiply_scaled, NumericStatus};
use crate::phase2_numeric::sin_cos_pitch_q15;
use crate::phase2_quantities::{
    PitchAngle, PlanarAcceleration, PlanarVelocity, Radius, SpecificAngularMomentum,
};
use crate::planar::{evaluate_vacuum, PlanarStepError, PlanarTruthState, PlanarWorld};
use crate::quantities::{Force, Time};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanarForceSnapshot {
    pitch: PitchAngle,
    thrust: Force,
    radial_thrust: Force,
    tangential_thrust: Force,
    radial_drag: Force,
    tangential_drag: Force,
    radial_acceleration: PlanarAcceleration,
    tangential_acceleration: PlanarAcceleration,
}

impl PlanarForceSnapshot {
    pub const fn pitch(self) -> PitchAngle {
        self.pitch
    }
    pub const fn thrust(self) -> Force {
        self.thrust
    }
    pub const fn radial_thrust(self) -> Force {
        self.radial_thrust
    }
    pub const fn tangential_thrust(self) -> Force {
        self.tangential_thrust
    }
    pub const fn radial_drag(self) -> Force {
        self.radial_drag
    }
    pub const fn tangential_drag(self) -> Force {
        self.tangential_drag
    }
    pub const fn radial_acceleration(self) -> PlanarAcceleration {
        self.radial_acceleration
    }
    pub const fn tangential_acceleration(self) -> PlanarAcceleration {
        self.tangential_acceleration
    }
}

pub fn evaluate_planar_forces(
    world: PlanarWorld,
    truth: PlanarTruthState,
    thrust: Force,
    pitch: PitchAngle,
    aero: AeroSnapshot,
    status: &mut NumericStatus,
) -> Option<PlanarForceSnapshot> {
    let (sine, cosine) = sin_cos_pitch_q15(pitch)?;
    let radial_thrust = multiply_scaled(thrust.raw(), cosine as i32, 15, status);
    let tangential_thrust = multiply_scaled(thrust.raw(), sine as i32, 15, status);
    let radial_force = add(radial_thrust, aero.radial_drag().raw(), status);
    let tangential_force = add(tangential_thrust, aero.tangential_drag().raw(), status);
    let radial_non_gravity = divide_scaled(radial_force, truth.total_mass().raw(), 28, status);
    let tangential_acceleration =
        divide_scaled(tangential_force, truth.total_mass().raw(), 28, status);
    let vacuum = evaluate_vacuum(world, truth, status);
    let radial_acceleration = add(
        vacuum.radial_acceleration().raw(),
        radial_non_gravity,
        status,
    );
    Some(PlanarForceSnapshot {
        pitch,
        thrust,
        radial_thrust: Force::from_raw(radial_thrust),
        tangential_thrust: Force::from_raw(tangential_thrust),
        radial_drag: aero.radial_drag(),
        tangential_drag: aero.tangential_drag(),
        radial_acceleration: PlanarAcceleration::from_raw(radial_acceleration),
        tangential_acceleration: PlanarAcceleration::from_raw(tangential_acceleration),
    })
}

pub fn advance_planar_state(
    world: PlanarWorld,
    truth: PlanarTruthState,
    forces: PlanarForceSnapshot,
    status: &mut NumericStatus,
) -> Result<PlanarTruthState, PlanarStepError> {
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    let timestep = world.timestep().raw();
    let delta_velocity = multiply_scaled(forces.radial_acceleration().raw(), timestep, 20, status);
    let radial_velocity = add(truth.radial_velocity().raw(), delta_velocity, status);
    let delta_radius = multiply_scaled(radial_velocity, timestep, 28, status);
    let radius = add(truth.radius().raw(), delta_radius, status);

    let angular_momentum_rate = multiply_scaled(
        truth.radius().raw(),
        forces.tangential_acceleration().raw(),
        26,
        status,
    );
    let delta_angular_momentum = multiply_scaled(angular_momentum_rate, timestep, 16, status);
    let angular_momentum = add(
        truth.specific_angular_momentum().raw(),
        delta_angular_momentum,
        status,
    );

    let angular_state = PlanarTruthState::new(
        truth.step(),
        truth.time(),
        Radius::from_raw(radius),
        truth.downrange(),
        PlanarVelocity::from_raw(radial_velocity),
        SpecificAngularMomentum::from_raw(angular_momentum),
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    );
    let angular = evaluate_vacuum(world, angular_state, status);
    let delta_downrange =
        multiply_scaled(angular.earth_relative_turn_rate_q30(), timestep, 14, status);
    let time = add(truth.time().raw(), timestep, status);
    if !status.is_clear() {
        return Err(PlanarStepError::NumericFault);
    }
    Ok(truth.successor(
        truth.step() + 1,
        Time::from_raw(time),
        Radius::from_raw(radius),
        truth.downrange().wrapping_add_raw(delta_downrange),
        PlanarVelocity::from_raw(radial_velocity),
        SpecificAngularMomentum::from_raw(angular_momentum),
        forces.radial_acceleration(),
        forces.tangential_acceleration(),
        truth.total_mass(),
        truth.active_propellant(),
        truth.active_stage(),
        truth.stage_phase(),
    ))
}
