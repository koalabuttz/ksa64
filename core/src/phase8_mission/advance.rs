//! One frozen-order Phase 8 mission advance.

use crate::numeric::{add, multiply_scaled, subtract, NumericStatus};
use crate::phase8_numeric::{
    BodyAngularRate, SpatialMass, SpatialTime, SPATIAL_COAST_ATTITUDE_STEP,
};
use crate::phase8_world::{
    acceleration_from_force, evaluate_hobby_spatial_environment, step_free_translation,
    step_hobby_attitude, step_rail_constrained, HobbySpatialEnvironment, HobbySpatialState,
    Phase8WorldError,
};

use super::forces::{
    dynamic_pressure_q13, environment_with_wind_scale, evaluate_forces, evaluate_forces_controlled,
    recovery_force, ForceInput,
};
use super::machine::Phase8MissionMachine;
use super::propulsion::{burn_propellant, derive_mass_properties, sample_motor_thrust};
use super::{
    HobbySpatialPhase, Phase85AppliedControl, Phase8MissionError, SpatialAeroState,
    SpatialMassProperties,
};

pub(super) struct AdvanceOutput {
    pub state: HobbySpatialState,
    pub mass: SpatialMassProperties,
    pub thrust_q13: i32,
    pub aero: SpatialAeroState,
    pub environment: HobbySpatialEnvironment,
}

pub(super) fn advance(
    machine: &mut Phase8MissionMachine<'_>,
    timestep: SpatialTime,
) -> Result<AdvanceOutput, Phase8MissionError> {
    advance_controlled(machine, timestep, Phase85AppliedControl::NEUTRAL, false)
}

pub(super) fn advance_controlled(
    machine: &mut Phase8MissionMachine<'_>,
    timestep: SpatialTime,
    control: Phase85AppliedControl,
    exact_attitude_substeps: bool,
) -> Result<AdvanceOutput, Phase8MissionError> {
    let phase = machine.snapshot.phase;
    let mut status = NumericStatus::CLEAR;

    // Frozen order: environment/wind, propulsion, mass properties,
    // aerodynamics, force/torque, translation, attitude, then caller events.
    let raw_environment = evaluate_hobby_spatial_environment(
        machine.snapshot.state,
        machine.wind,
        machine.mission.case_seed,
    )
    .map_err(|error| match error {
        Phase8WorldError::ModelEnvelopeExceeded => Phase8MissionError::ModelEnvelopeExceeded,
        _ => Phase8MissionError::Numeric,
    })?;
    let environment = environment_with_wind_scale(
        raw_environment,
        machine.snapshot.state,
        machine.variation.wind_scale_ppm,
        &mut status,
    );
    let thrust = if matches!(
        phase,
        HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
    ) {
        sample_motor_thrust(
            machine.motor,
            machine.snapshot.state.time,
            machine.variation.thrust_scale_ppm,
            &mut status,
        )
    } else {
        0
    };
    let remaining = if machine.snapshot.state.time.raw() >= machine.motor.burn_time.raw() {
        SpatialMass::ZERO
    } else {
        burn_propellant(
            machine.motor,
            machine.snapshot.mass.propellant_remaining,
            thrust,
            timestep,
            &mut status,
        )
    };
    let mass = derive_mass_properties(
        machine.vehicle,
        machine.motor,
        remaining,
        machine.variation.mass_scale_ppm,
        &mut status,
    );

    let (state, aero) = match phase {
        HobbySpatialPhase::ConstrainedPowered => {
            let forces = evaluate_forces(
                ForceInput {
                    vehicle: machine.vehicle,
                    mass,
                    state: machine.snapshot.state,
                    environment,
                    thrust_q13: thrust,
                    variation: machine.variation,
                    enforce_envelope: false,
                },
                &mut status,
            )?;
            let acceleration = acceleration_from_force(
                forces.force_enu,
                mass.mass,
                environment.gravity_q19,
                &mut status,
            );
            let axial = add(
                add(
                    multiply_scaled(acceleration.x(), machine.rail_axis.x(), 30, &mut status),
                    multiply_scaled(acceleration.y(), machine.rail_axis.y(), 30, &mut status),
                    &mut status,
                ),
                multiply_scaled(acceleration.z(), machine.rail_axis.z(), 30, &mut status),
                &mut status,
            )
            .max(0);
            let (state, rail) = step_rail_constrained(
                machine.snapshot.state,
                machine.rail,
                machine.rail_axis,
                axial,
                timestep,
            )
            .map_err(|_| Phase8MissionError::Numeric)?;
            machine.rail = rail;
            (state, forces.aero)
        }
        HobbySpatialPhase::PoweredFlight | HobbySpatialPhase::Coast => {
            let input = ForceInput {
                vehicle: machine.vehicle,
                mass,
                state: machine.snapshot.state,
                environment,
                thrust_q13: thrust,
                variation: machine.variation,
                enforce_envelope: true,
            };
            let applied = if phase == HobbySpatialPhase::PoweredFlight {
                control
            } else {
                Phase85AppliedControl::NEUTRAL
            };
            let forces = evaluate_forces_controlled(input, applied, &mut status)?;
            let acceleration = acceleration_from_force(
                forces.force_enu,
                mass.mass,
                environment.gravity_q19,
                &mut status,
            );
            let translated = step_free_translation(machine.snapshot.state, acceleration, timestep)
                .map_err(|_| Phase8MissionError::Numeric)?;
            let mut rotated = translated;
            if exact_attitude_substeps && phase == HobbySpatialPhase::Coast {
                let mut remaining = timestep.raw();
                while remaining > 0 {
                    let raw = remaining.min(SPATIAL_COAST_ATTITUDE_STEP.raw());
                    rotated = step_hobby_attitude(
                        rotated,
                        mass.inertia,
                        forces.torque_body,
                        SpatialTime::from_raw(raw),
                    )
                    .map_err(|_| Phase8MissionError::Numeric)?;
                    remaining -= raw;
                }
            } else {
                let attitude_steps = if phase == HobbySpatialPhase::Coast {
                    2
                } else {
                    1
                };
                let attitude_dt = if phase == HobbySpatialPhase::Coast {
                    SPATIAL_COAST_ATTITUDE_STEP
                } else {
                    timestep
                };
                let mut index = 0;
                while index < attitude_steps {
                    rotated =
                        step_hobby_attitude(rotated, mass.inertia, forces.torque_body, attitude_dt)
                            .map_err(|_| Phase8MissionError::Numeric)?;
                    index += 1;
                }
            }
            (rotated, forces.aero)
        }
        HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery => {
            let drag = recovery_force(
                machine.vehicle,
                environment,
                phase,
                subtract(
                    machine.snapshot.state.time.raw(),
                    machine.deployment_started_raw,
                    &mut status,
                ),
                machine.mission,
                machine.variation,
                &mut status,
            );
            let acceleration =
                acceleration_from_force(drag, mass.mass, environment.gravity_q19, &mut status);
            let mut recovered =
                step_free_translation(machine.snapshot.state, acceleration, timestep)
                    .map_err(|_| Phase8MissionError::Numeric)?;
            recovered.angular_rate = BodyAngularRate::ZERO;
            (
                recovered,
                SpatialAeroState {
                    dynamic_pressure_q13: dynamic_pressure_q13(
                        environment,
                        machine.variation.density_scale_ppm,
                        &mut status,
                    ),
                    ..SpatialAeroState::ZERO
                },
            )
        }
        HobbySpatialPhase::Complete | HobbySpatialPhase::Failed => {
            return Err(Phase8MissionError::Complete)
        }
    };
    if !status.is_clear() {
        return Err(Phase8MissionError::Numeric);
    }
    Ok(AdvanceOutput {
        state,
        mass,
        thrust_q13: thrust,
        aero,
        environment,
    })
}
