//! Parameter application boundary for Phase 4 campaigns.

use crate::actuator::{ActuatorParameters, DEFAULT_LAG_STEPS, MAX_SLEW_PER_STEP};
use crate::mission::{
    run_parameterized_mission, MissionCase, MissionError, MissionParameters, MissionResult,
};
use crate::sensors::SensorParameters;
use crate::world::WorldParameters;
use ksa64_core::phase2_scenario::Phase2Scenario;

use super::campaign::{ParameterId, RunSpec};

fn scale_ppm(value: i32, delta_ppm: i32) -> Option<i32> {
    let scaled = (value as i64 * (1_000_000i64 + delta_ppm as i64)) / 1_000_000i64;
    if scaled < i32::MIN as i64 || scaled > i32::MAX as i64 {
        None
    } else {
        Some(scaled as i32)
    }
}

pub fn mission_parameters(run: RunSpec) -> Option<MissionParameters> {
    let variation = run.variation;
    let lag = DEFAULT_LAG_STEPS as i32 + variation.value(ParameterId::ActuatorLagSteps);
    if !(1..=16).contains(&lag) {
        return None;
    }
    let parameters = MissionParameters {
        world: WorldParameters {
            payload_mass_ppm: variation.value(ParameterId::PayloadMassPpm),
            stage_thrust_ppm: [
                variation.value(ParameterId::Stage1ThrustPpm),
                variation.value(ParameterId::Stage2ThrustPpm),
            ],
            atmosphere_density_ppm: variation.value(ParameterId::AtmosphereDensityPpm),
            drag_ppm: variation.value(ParameterId::DragPpm),
        },
        sensors: SensorParameters {
            accelerometer_bias_q28: variation.value(ParameterId::AccelerometerBiasQ28),
            gyro_bias_q24: variation.value(ParameterId::GyroBiasQ24),
            altimeter_bias_q12: variation.value(ParameterId::AltimeterBiasQ12),
            gps_radial_position_bias_q12: variation.value(ParameterId::GpsRadialPositionQ12),
            gps_downrange_bias_q32: variation.value(ParameterId::GpsDownrangeQ32),
            gps_radial_velocity_bias_q24: variation.value(ParameterId::GpsRadialVelocityQ24),
            gps_tangential_velocity_bias_q24: variation
                .value(ParameterId::GpsTangentialVelocityQ24),
            noise_scale_ppm: variation.value(ParameterId::SensorNoisePpm),
        },
        actuator: ActuatorParameters {
            lag_steps: lag as u8,
            max_slew_per_step: scale_ppm(
                MAX_SLEW_PER_STEP,
                variation.value(ParameterId::ActuatorSlewPpm),
            )?,
        },
        sensor_seed: run.sensor_seed,
    };
    parameters.is_valid().then_some(parameters)
}

pub fn run_phase4_mission(
    scenario: &Phase2Scenario,
    run: RunSpec,
) -> Result<MissionResult, MissionError> {
    let parameters = mission_parameters(run).ok_or(MissionError::Numeric)?;
    run_parameterized_mission(scenario, MissionCase::Nominal, parameters)
}
