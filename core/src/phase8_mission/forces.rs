//! Phase 8 pre-deployment aerodynamics and recovery drag.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericStatus};
use crate::phase2_numeric::sin_cos_binary_q15;
use crate::phase8_aero::{
    enforce_spatial_aero_envelope, sample_spatial_aerodynamics, small_angle_of_attack_q28,
    SpatialAeroError,
};
use crate::phase8_numeric::{BodyTorque, EnuForce, EnuVelocity};
use crate::phase8_pack::{SpatialMissionPack, SpatialVehiclePack};
use crate::phase8_world::{HobbySpatialEnvironment, HobbySpatialState};
use crate::spatial_numeric::FixedVec3;

use super::propulsion::scale_ppm;
use super::{
    magnitude3_i32, HobbySpatialPhase, Phase85AppliedControl, Phase8MissionError, SpatialAeroState,
    SpatialMassProperties, SpatialMissionVariation,
};

/// Below this pressure the air-relative direction becomes numerically singular
/// while aerodynamic forces are no longer mission-significant. The attitude
/// state remains live, but the small-angle normal-force model is retired.
pub const MIN_DIRECTIONAL_AERO_Q13: i32 = 50 << 13;

#[derive(Clone, Copy, Debug)]
pub(super) struct ForceMoment {
    pub force_enu: EnuForce,
    pub torque_body: BodyTorque,
    pub aero: SpatialAeroState,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct ForceInput<'a> {
    pub vehicle: &'a SpatialVehiclePack,
    pub mass: SpatialMassProperties,
    pub state: HobbySpatialState,
    pub environment: HobbySpatialEnvironment,
    pub thrust_q13: i32,
    pub variation: SpatialMissionVariation,
    pub enforce_envelope: bool,
}

pub(super) fn dynamic_pressure_q13(
    environment: HobbySpatialEnvironment,
    density_ppm: i32,
    status: &mut NumericStatus,
) -> i32 {
    let speed2_q13 = multiply_scaled(
        environment.air_speed_q19,
        environment.air_speed_q19,
        25,
        status,
    );
    let density = scale_ppm(environment.density_q29, density_ppm, status);
    multiply_scaled(density, speed2_q13, 29, status) / 2
}

fn signed_opposing_force(
    magnitude_q13: i32,
    velocity_q19: EnuVelocity,
    speed_q19: i32,
    status: &mut NumericStatus,
) -> EnuForce {
    if speed_q19 <= 0 || magnitude_q13 == 0 {
        return EnuForce::ZERO;
    }
    let unit = FixedVec3::<30>::new(
        divide_scaled(velocity_q19.x(), speed_q19, 30, status),
        divide_scaled(velocity_q19.y(), speed_q19, 30, status),
        divide_scaled(velocity_q19.z(), speed_q19, 30, status),
    );
    EnuForce::new(
        -multiply_scaled(magnitude_q13, unit.x(), 30, status),
        -multiply_scaled(magnitude_q13, unit.y(), 30, status),
        -multiply_scaled(magnitude_q13, unit.z(), 30, status),
    )
}

pub(super) fn evaluate_forces(
    input: ForceInput<'_>,
    status: &mut NumericStatus,
) -> Result<ForceMoment, Phase8MissionError> {
    evaluate_forces_controlled(input, Phase85AppliedControl::NEUTRAL, status)
}

pub(super) fn evaluate_forces_controlled(
    input: ForceInput<'_>,
    control: Phase85AppliedControl,
    status: &mut NumericStatus,
) -> Result<ForceMoment, Phase8MissionError> {
    let ForceInput {
        vehicle,
        mass,
        state,
        environment,
        thrust_q13,
        variation,
        enforce_envelope,
    } = input;
    let mach_q24 = if environment.sound_speed_q19 > 0 {
        divide_scaled(
            environment.air_speed_q19,
            environment.sound_speed_q19,
            24,
            status,
        )
    } else {
        0
    };
    let sample = sample_spatial_aerodynamics(vehicle, mach_q24, mass.cg_from_nose.raw(), status)
        .map_err(|error| match error {
            SpatialAeroError::ModelEnvelopeExceeded => Phase8MissionError::ModelEnvelopeExceeded,
            _ => Phase8MissionError::Numeric,
        })?;
    let q_q13 = dynamic_pressure_q13(environment, variation.density_scale_ppm, status);
    let directional_aero_active = enforce_envelope && q_q13 >= MIN_DIRECTIONAL_AERO_Q13;
    let angle = if directional_aero_active {
        let value = small_angle_of_attack_q28(
            [
                environment.air_velocity_body.x(),
                environment.air_velocity_body.y(),
                environment.air_velocity_body.z(),
            ],
            status,
        )
        .map_err(|error| match error {
            SpatialAeroError::ModelEnvelopeExceeded => Phase8MissionError::ModelEnvelopeExceeded,
            _ => Phase8MissionError::Numeric,
        })?;
        enforce_spatial_aero_envelope(mach_q24, value)
            .map_err(|_| Phase8MissionError::ModelEnvelopeExceeded)?;
        value.raw()
    } else {
        0
    };
    let q_area_q13 = multiply_scaled(q_q13, vehicle.reference_area.raw(), 29, status);
    let cp = add(sample.cp_from_nose.raw(), variation.cp_offset_q28, status);
    let cp_aft_of_cg_q28 = subtract(cp, mass.cg_from_nose.raw(), status);
    let static_margin_q24 = divide_scaled(cp_aft_of_cg_q28, vehicle.diameter.raw(), 9, status);
    if enforce_envelope && static_margin_q24 <= 0 {
        return Err(Phase8MissionError::ModelEnvelopeExceeded);
    }
    let axial_cd = scale_ppm(
        sample.axial_cd.raw(),
        variation.axial_drag_scale_ppm,
        status,
    );
    let drag_magnitude = multiply_scaled(q_area_q13, axial_cd, 24, status);
    let drag = signed_opposing_force(
        drag_magnitude,
        environment.air_velocity_enu,
        environment.air_speed_q19,
        status,
    );
    let thrust_body = if control.gimbal_turn16 == [0; 2] {
        EnuForce::new(thrust_q13, 0, 0)
    } else {
        let (sin_pitch, cos_pitch) = sin_cos_binary_q15(control.gimbal_turn16[0] as u16);
        let (sin_yaw, cos_yaw) = sin_cos_binary_q15(control.gimbal_turn16[1] as u16);
        EnuForce::new(
            multiply_scaled(
                multiply_scaled(thrust_q13, i32::from(cos_pitch), 15, status),
                i32::from(cos_yaw),
                15,
                status,
            ),
            multiply_scaled(thrust_q13, i32::from(sin_yaw), 15, status),
            -multiply_scaled(thrust_q13, i32::from(sin_pitch), 15, status),
        )
    };
    let thrust = state.attitude.rotate(thrust_body, status);

    let body = environment.air_velocity_body;
    let normal_slope = scale_ppm(
        sample.normal_force_slope.raw(),
        variation.normal_force_scale_ppm,
        status,
    );
    let normal_per_alpha = if directional_aero_active {
        multiply_scaled(q_area_q13, normal_slope, 24, status)
    } else {
        0
    };
    // Evaluate lateral flow at CP, not only at CG. The omega cross r term
    // provides geometry-consistent pitch/yaw rate damping.
    let local_y_q19 = subtract(
        body.y(),
        multiply_scaled(cp_aft_of_cg_q28, state.angular_rate.z(), 28, status) >> 5,
        status,
    );
    let local_z_q19 = add(
        body.z(),
        multiply_scaled(cp_aft_of_cg_q28, state.angular_rate.y(), 28, status) >> 5,
        status,
    );
    let alpha_y_q24 = if directional_aero_active && body.x() > 0 {
        divide_scaled(local_y_q19, body.x(), 24, status)
    } else {
        0
    };
    let alpha_z_q24 = if directional_aero_active && body.x() > 0 {
        divide_scaled(local_z_q19, body.x(), 24, status)
    } else {
        0
    };
    let normal_body = EnuForce::new(
        0,
        -multiply_scaled(normal_per_alpha, alpha_y_q24, 24, status),
        -multiply_scaled(normal_per_alpha, alpha_z_q24, 24, status),
    );
    let normal = state.attitude.rotate(normal_body, status);
    let force = thrust.checked_add(drag, status).checked_add(normal, status);

    let cp = add(sample.cp_from_nose.raw(), variation.cp_offset_q28, status);
    let cp_aft_of_cg_q28 = subtract(cp, mass.cg_from_nose.raw(), status);
    let static_margin_q24 = divide_scaled(cp_aft_of_cg_q28, vehicle.diameter.raw(), 9, status);
    if enforce_envelope && static_margin_q24 <= 0 {
        return Err(Phase8MissionError::ModelEnvelopeExceeded);
    }
    let arm_body_x_q28 = -cp_aft_of_cg_q28;
    let aerodynamic_torque = BodyTorque::new(
        0,
        -multiply_scaled(arm_body_x_q28, normal_body.z(), 29, status),
        multiply_scaled(arm_body_x_q28, normal_body.y(), 29, status),
    );
    let q_area_length_q12 = multiply_scaled(q_area_q13, vehicle.length.raw(), 14, status);
    let damping = BodyTorque::new(
        -multiply_scaled(
            multiply_scaled(q_area_length_q12, vehicle.roll_damping.raw(), 24, status),
            state.angular_rate.x(),
            24,
            status,
        ),
        -multiply_scaled(
            multiply_scaled(q_area_length_q12, vehicle.pitch_damping.raw(), 24, status),
            state.angular_rate.y(),
            24,
            status,
        ),
        -multiply_scaled(
            multiply_scaled(q_area_length_q12, vehicle.yaw_damping.raw(), 24, status),
            state.angular_rate.z(),
            24,
            status,
        ),
    );
    let thrust_torque = if control.gimbal_turn16 == [0; 2] {
        BodyTorque::ZERO
    } else {
        let arm_q28 = subtract(control.pivot_from_nose_q28, mass.cg_from_nose.raw(), status);
        BodyTorque::new(
            0,
            -multiply_scaled(arm_q28, thrust_body.z(), 29, status),
            multiply_scaled(arm_q28, thrust_body.y(), 29, status),
        )
    };
    let torque = aerodynamic_torque
        .checked_add(damping, status)
        .checked_add(thrust_torque, status);
    if !status.is_clear() {
        return Err(Phase8MissionError::Numeric);
    }
    Ok(ForceMoment {
        force_enu: force,
        torque_body: torque,
        aero: SpatialAeroState {
            mach_q24,
            angle_of_attack_q28: angle,
            dynamic_pressure_q13: q_q13,
            axial_drag_q13: drag_magnitude,
            normal_force_q13: magnitude3_i32(normal_body, status),
            static_margin_q24,
        },
    })
}

pub(super) fn recovery_force(
    vehicle: &SpatialVehiclePack,
    environment: HobbySpatialEnvironment,
    phase: HobbySpatialPhase,
    deployment_elapsed: i32,
    mission: SpatialMissionPack,
    variation: SpatialMissionVariation,
    status: &mut NumericStatus,
) -> EnuForce {
    let (target_cda, inflation) = match phase {
        HobbySpatialPhase::DrogueRecovery => (
            vehicle.drogue_cda.raw(),
            mission.drogue_inflation_time.raw(),
        ),
        HobbySpatialPhase::MainRecovery => {
            (vehicle.main_cda.raw(), mission.main_inflation_time.raw())
        }
        _ => return EnuForce::ZERO,
    };
    let inflation = scale_ppm(inflation, variation.inflation_scale_ppm, status).max(1);
    let fraction_q16 =
        divide_scaled(deployment_elapsed.max(0), inflation, 16, status).clamp(0, 65_536);
    let cda = multiply_scaled(
        scale_ppm(target_cda, variation.recovery_cda_scale_ppm, status),
        fraction_q16,
        16,
        status,
    );
    let q_q13 = dynamic_pressure_q13(environment, variation.density_scale_ppm, status);
    let drag = multiply_scaled(q_q13, cda, 29, status);
    signed_opposing_force(
        drag,
        environment.air_velocity_enu,
        environment.air_speed_q19,
        status,
    )
}

pub(super) fn environment_with_wind_scale(
    mut environment: HobbySpatialEnvironment,
    state: HobbySpatialState,
    scale_ppm_value: i32,
    status: &mut NumericStatus,
) -> HobbySpatialEnvironment {
    let wind_q19 = EnuVelocity::new(
        scale_ppm(environment.wind.total.x() >> 3, scale_ppm_value, status),
        scale_ppm(environment.wind.total.y() >> 3, scale_ppm_value, status),
        scale_ppm(environment.wind.total.z() >> 3, scale_ppm_value, status),
    );
    environment.air_velocity_enu = state.velocity.checked_sub(wind_q19, status);
    environment.air_velocity_body = state
        .attitude
        .conjugate()
        .rotate(environment.air_velocity_enu, status);
    environment.air_speed_q19 = magnitude3_i32(environment.air_velocity_enu, status);
    environment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_format::KWP8_MAX_WIND_KNOTS;
    use crate::phase8_mission::Phase8MissionMachine;
    use crate::phase8_numeric::{SpatialPosition, SpatialTime, SpatialWind};
    use crate::phase8_pack::{
        parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack, WindKnot,
        WindProfilePack,
    };
    use crate::phase8_world::{
        acceleration_from_force, evaluate_hobby_spatial_environment, step_rail_constrained,
    };
    #[test]
    fn initial_cardinal_wind_force_path_is_symmetric() {
        let vehicle =
            parse_spatial_vehicle_pack(include_bytes!("../../../phase8/examples/firestorm54.kvp8"))
                .unwrap();
        let motor = parse_spatial_motor_pack(include_bytes!(
            "../../../phase8/examples/aerotech-i211w.kmp8"
        ))
        .unwrap();
        let base = parse_spatial_mission_pack(include_bytes!(
            "../../../phase8/examples/firestorm-i211.kmc8"
        ))
        .unwrap();
        for east in [2, -2] {
            let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
            knots[0] = WindKnot {
                altitude: SpatialPosition::ZERO,
                east: SpatialWind::from_raw(east << 22),
                north: SpatialWind::ZERO,
            };
            knots[1] = WindKnot {
                altitude: SpatialPosition::from_raw(1_000 << 13),
                east: SpatialWind::from_raw(east << 22),
                north: SpatialWind::ZERO,
            };
            let wind = WindProfilePack {
                identity: 0x8200_0000 + u32::from(east < 0),
                gust_seed: 0,
                gust_cadence: SpatialTime::from_raw(1 << 18),
                gust_amplitude_east: SpatialWind::ZERO,
                gust_amplitude_north: SpatialWind::ZERO,
                max_gust: SpatialWind::ZERO,
                knot_count: 2,
                knots,
            };
            let mission = SpatialMissionPack {
                wind_identity: wind.identity,
                ..base
            };
            let machine = Phase8MissionMachine::new(&vehicle, &motor, mission, &wind).unwrap();
            let raw = evaluate_hobby_spatial_environment(
                machine.snapshot.state,
                &wind,
                mission.case_seed,
            )
            .unwrap();
            let mut status = NumericStatus::CLEAR;
            let environment =
                environment_with_wind_scale(raw, machine.snapshot.state, 1_000_000, &mut status);
            assert!(status.is_clear(), "environment {east}: {:?}", status);
            let forces_result = evaluate_forces(
                ForceInput {
                    vehicle: &vehicle,
                    mass: machine.snapshot.mass,
                    state: machine.snapshot.state,
                    environment,
                    thrust_q13: 0,
                    variation: SpatialMissionVariation::NOMINAL,
                    enforce_envelope: false,
                },
                &mut status,
            );
            assert!(
                forces_result.is_ok(),
                "forces {east}: {status:?} {forces_result:?}"
            );
            let forces = forces_result.unwrap();
            assert!(status.is_clear(), "forces {east}: {:?}", status);
            let acceleration = acceleration_from_force(
                forces.force_enu,
                machine.snapshot.mass.mass,
                environment.gravity_q19,
                &mut status,
            );
            assert!(
                status.is_clear(),
                "acceleration {east}: {:?} {:?}",
                status,
                forces.force_enu
            );
            let axial =
                multiply_scaled(acceleration.z(), machine.rail_axis.z(), 30, &mut status).max(0);
            step_rail_constrained(machine.snapshot.state,machine.rail,machine.rail_axis,axial,SpatialTime::from_raw(2621)).unwrap_or_else(|error|panic!("rail {east}: {error:?}, status={status:?}, force={:?}, acceleration={acceleration:?}",forces.force_enu));
        }
    }
}
