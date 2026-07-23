//! Gate 8 integrated KSA-5A reference and reviewed failure missions.

use crate::phase5_closed_loop::{Phase5ClosedLoop, Phase5ClosedLoopError, Phase5ClosedLoopStep};
use crate::phase5_sensors::{Phase5SensorFaults, Phase5SensorParameters};
use crate::phase5_vehicle::{Phase5StagePhase, EVENT_RCS_DEPLETED};
use crate::sensors::StepWindow;
use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::planar::OrbitClass;
use ksa64_core::spatial_numeric::ForceVec;
use ksa64_core::spatial_world::classify_spatial_orbit;
use ksa64_flight::phase5_gnc::AttitudeControllerGains;
use ksa64_flight::phase5_guidance::reference_guidance_target_scaled;
use ksa64_interface::FlightMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase5MissionCase {
    Nominal,
    GustAndSlosh,
    StarOutageAndGyroBias,
    GimbalJamAbort,
    DampingLossAbort,
    RcsLeakAndDepletion,
}
impl Phase5MissionCase {
    pub const ALL: [Self; 6] = [
        Self::Nominal,
        Self::GustAndSlosh,
        Self::StarOutageAndGyroBias,
        Self::GimbalJamAbort,
        Self::DampingLossAbort,
        Self::RcsLeakAndDepletion,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase5MissionOutcome {
    StableOrbit,
    CompleteNotOrbit,
    Aborted,
    NumericFault,
    StepLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Phase5MissionSummary {
    pub case: Phase5MissionCase,
    pub outcome: Phase5MissionOutcome,
    pub steps: u32,
    pub terminal_position_q12: [i32; 3],
    pub terminal_velocity_q24: [i32; 3],
    pub perigee_altitude_q12: i32,
    pub apogee_altitude_q12: i32,
    pub inclination_turn16: u16,
    pub max_dynamic_pressure_q16: i32,
    pub max_aoa_sine_q16: i32,
    pub max_flexible_state_q24: i32,
    pub max_nav_position_error_q12: i32,
    pub events: u16,
    pub sensor_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub summary_checksum: u32,
}

pub trait Phase5MissionObserver {
    type Error;

    fn observe_initial(
        &mut self,
        case: Phase5MissionCase,
        seed: u32,
        snapshot: crate::phase5_vehicle::Phase5VehicleSnapshot,
    ) -> Result<(), Self::Error>;

    fn observe_step(
        &mut self,
        case: Phase5MissionCase,
        step: Phase5ClosedLoopStep,
        terminal: bool,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5ObservedMissionError<E> {
    Mission(Phase5ClosedLoopError),
    Observer(E),
}

struct NullObserver;

impl Phase5MissionObserver for NullObserver {
    type Error = core::convert::Infallible;

    fn observe_initial(
        &mut self,
        _case: Phase5MissionCase,
        _seed: u32,
        _snapshot: crate::phase5_vehicle::Phase5VehicleSnapshot,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn observe_step(
        &mut self,
        _case: Phase5MissionCase,
        _step: Phase5ClosedLoopStep,
        _terminal: bool,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn run_phase5_mission(
    case: Phase5MissionCase,
) -> Result<Phase5MissionSummary, Phase5ClosedLoopError> {
    match run_phase5_mission_observed(case, &mut NullObserver) {
        Ok(summary) => Ok(summary),
        Err(Phase5ObservedMissionError::Mission(error)) => Err(error),
        Err(Phase5ObservedMissionError::Observer(never)) => match never {},
    }
}

pub fn run_phase5_mission_observed<O: Phase5MissionObserver>(
    case: Phase5MissionCase,
    observer: &mut O,
) -> Result<Phase5MissionSummary, Phase5ObservedMissionError<O::Error>> {
    let (faults, parameters) = sensor_configuration(case);
    let seed = 0x5a00_0000u32 | case as u32;
    let mut loopback = Phase5ClosedLoop::new_parameterized(seed, faults, parameters)
        .map_err(Phase5ObservedMissionError::Mission)?;
    observer
        .observe_initial(case, seed, loopback.latest())
        .map_err(Phase5ObservedMissionError::Observer)?;
    if case == Phase5MissionCase::RcsLeakAndDepletion {
        loopback
            .vehicle_mut()
            .set_rcs_leak_q15(ksa64_core::spatial_numeric::FixedVec3::new(
                32_767, 32_767, 32_767,
            ));
    }
    let mut summary = Phase5MissionSummary {
        case,
        outcome: Phase5MissionOutcome::StepLimit,
        steps: 0,
        terminal_position_q12: [0; 3],
        terminal_velocity_q24: [0; 3],
        perigee_altitude_q12: i32::MIN,
        apogee_altitude_q12: i32::MIN,
        inclination_turn16: 0,
        max_dynamic_pressure_q16: 0,
        max_aoa_sine_q16: 0,
        max_flexible_state_q24: 0,
        max_nav_position_error_q12: 0,
        events: 0,
        sensor_checksum: 0,
        navigation_checksum: 0,
        flight_checksum: 0,
        summary_checksum: 0,
    };
    let mut step = 0u32;
    while step < ksa64_core::phase5_contract::PHASE5_MISSION_STEPS {
        apply_case_dynamics(case, step, &mut loopback);
        let gains = if loopback.latest().truth.active_stage() == 0 {
            AttitudeControllerGains::REFERENCE_STAGE1
        } else {
            AttitudeControllerGains::REFERENCE_STAGE2
        };
        let result = match loopback
            .step_with_gains(reference_guidance_target_scaled(step, 100, 100), gains)
        {
            Ok(value) => value,
            Err(Phase5ClosedLoopError::Vehicle(_)) => {
                summary.outcome = Phase5MissionOutcome::NumericFault;
                break;
            }
            Err(error) => return Err(Phase5ObservedMissionError::Mission(error)),
        };
        summary.steps = result.vehicle.truth.step();
        summary.events |= result.vehicle.events;
        summary.max_dynamic_pressure_q16 = summary
            .max_dynamic_pressure_q16
            .max(result.vehicle.dynamic_pressure_q16);
        summary.max_aoa_sine_q16 = summary
            .max_aoa_sine_q16
            .max(result.vehicle.angle_of_attack_sine_q16.abs());
        summary.max_flexible_state_q24 = summary
            .max_flexible_state_q24
            .max(max_flexible(result.vehicle.truth.flexible()));
        let truth_position = result.vehicle.truth.spatial().position();
        let nav = result.flight.navigation;
        let nav_error = [
            truth_position.x().saturating_sub(nav.position_q12[0]).abs(),
            truth_position.y().saturating_sub(nav.position_q12[1]).abs(),
            truth_position.z().saturating_sub(nav.position_q12[2]).abs(),
        ];
        summary.max_nav_position_error_q12 = summary
            .max_nav_position_error_q12
            .max(nav_error[0].max(nav_error[1]).max(nav_error[2]));
        summary.sensor_checksum = result.sensor_checksum;
        summary.navigation_checksum = nav.checksum;
        summary.flight_checksum = result.flight.flight_checksum;
        if result.flight.mode == FlightMode::Abort {
            summary.outcome = Phase5MissionOutcome::Aborted;
        } else if result.vehicle.truth.phase() == Phase5StagePhase::Complete {
            summary.outcome = classify_terminal(result.vehicle.truth.spatial(), &mut summary);
        }
        let terminal = summary.outcome != Phase5MissionOutcome::StepLimit;
        observer
            .observe_step(case, result, terminal)
            .map_err(Phase5ObservedMissionError::Observer)?;
        if terminal {
            break;
        }
        step += 1;
    }
    let terminal = loopback.latest().truth.spatial();
    summary.terminal_position_q12 = [
        terminal.position().x(),
        terminal.position().y(),
        terminal.position().z(),
    ];
    summary.terminal_velocity_q24 = [
        terminal.velocity().x(),
        terminal.velocity().y(),
        terminal.velocity().z(),
    ];
    if summary.outcome == Phase5MissionOutcome::StepLimit {
        summary.outcome = classify_terminal(terminal, &mut summary);
    }
    summary.summary_checksum = hash_summary(summary);
    Ok(summary)
}

fn sensor_configuration(case: Phase5MissionCase) -> (Phase5SensorFaults, Phase5SensorParameters) {
    if case == Phase5MissionCase::StarOutageAndGyroBias {
        (
            Phase5SensorFaults {
                star_tracker_outage: Some(StepWindow {
                    start: 400,
                    end: 1_600,
                }),
                ..Phase5SensorFaults::default()
            },
            Phase5SensorParameters {
                gyro_bias_q24: [400, 1_464, -900],
                ..Phase5SensorParameters::DEFAULT
            },
        )
    } else {
        (
            Phase5SensorFaults::default(),
            Phase5SensorParameters::DEFAULT,
        )
    }
}

fn apply_case_dynamics(case: Phase5MissionCase, step: u32, loopback: &mut Phase5ClosedLoop) {
    match case {
        Phase5MissionCase::GustAndSlosh => {
            let force = if (600..632).contains(&step) {
                ForceVec::new(0, 328, -164)
            } else {
                ForceVec::ZERO
            };
            loopback.vehicle_mut().set_disturbance_body_q12(force);
        }
        Phase5MissionCase::GimbalJamAbort if step == 1_000 => {
            loopback.vehicle_mut().set_gimbal_jammed(true, true);
        }
        Phase5MissionCase::DampingLossAbort => {
            if step == 500 {
                loopback.vehicle_mut().set_damping_scale_q16(0);
            }
            let force = if (520..600).contains(&step) {
                ForceVec::new(0, 410, 246)
            } else {
                ForceVec::ZERO
            };
            loopback.vehicle_mut().set_disturbance_body_q12(force);
        }
        _ => {}
    }
}

fn classify_terminal(
    spatial: ksa64_core::spatial_world::SpatialState,
    summary: &mut Phase5MissionSummary,
) -> Phase5MissionOutcome {
    let mut status = NumericStatus::CLEAR;
    let Some(orbit) = classify_spatial_orbit(spatial, &mut status) else {
        return Phase5MissionOutcome::NumericFault;
    };
    summary.perigee_altitude_q12 = orbit.perigee().raw().saturating_sub(EARTH_RADIUS_Q12);
    summary.apogee_altitude_q12 = orbit.apogee().raw().saturating_sub(EARTH_RADIUS_Q12);
    summary.inclination_turn16 = orbit.inclination_turn16();
    if status.is_clear() && orbit.class() == OrbitClass::StableOrbit {
        Phase5MissionOutcome::StableOrbit
    } else {
        Phase5MissionOutcome::CompleteNotOrbit
    }
}

fn max_flexible(state: ksa64_core::flexible::FlexibleStateQ24) -> i32 {
    let mut maximum = 0;
    for value in [
        state.y().bending().displacement(),
        state.y().bending().rate(),
        state.y().slosh().displacement(),
        state.y().slosh().rate(),
        state.z().bending().displacement(),
        state.z().bending().rate(),
        state.z().slosh().displacement(),
        state.z().slosh().rate(),
    ] {
        maximum = maximum.max(value.saturating_abs());
    }
    maximum
}

fn hash_word(mut hash: u32, word: u32) -> u32 {
    for byte in word.to_le_bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}
fn hash_summary(hash: Phase5MissionSummary) -> u32 {
    let mut value = 2_166_136_261u32;
    value = hash_word(value, hash.case as u32);
    value = hash_word(value, hash.outcome as u32);
    value = hash_word(value, hash.steps);
    for word in hash.terminal_position_q12 {
        value = hash_word(value, word as u32);
    }
    for word in hash.terminal_velocity_q24 {
        value = hash_word(value, word as u32);
    }
    for word in [
        hash.perigee_altitude_q12,
        hash.apogee_altitude_q12,
        hash.max_dynamic_pressure_q16,
        hash.max_aoa_sine_q16,
        hash.max_flexible_state_q24,
        hash.max_nav_position_error_q12,
    ] {
        value = hash_word(value, word as u32);
    }
    value = hash_word(value, hash.inclination_turn16 as u32);
    value = hash_word(value, hash.events as u32);
    value = hash_word(value, hash.sensor_checksum);
    value = hash_word(value, hash.navigation_checksum);
    value = hash_word(value, hash.flight_checksum);
    value
}

pub const fn rcs_depletion_event_seen(summary: Phase5MissionSummary) -> bool {
    summary.events & EVENT_RCS_DEPLETED != 0
}
