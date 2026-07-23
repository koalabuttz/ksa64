//! Finite, allocation-free Phase 3 target probes.

use crate::actuator::SteeringActuator;
use crate::sensors::{SensorFaults, SensorSuite};
use crate::world::{WorldCommand, WorldMachine, WorldSnapshot};
use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::{EARTH_MU_Q12, EARTH_RADIUS_Q12};
use ksa64_core::phase2_quantities::{
    DownrangeAngle, DynamicPressure, Mach, PitchAngle, PlanarVelocity, Radius,
    SpecificAngularMomentum,
};
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::planar::{
    advance_vacuum_semi_implicit, PlanarTruthState, PlanarWorld, StagePhase as TruthStagePhase,
};
use ksa64_core::quantities::{Mass, Time};
use ksa64_flight::gnc::FlightComputer;
use ksa64_interface::{
    FlightMode, SensorFrame, StagePhase, SENSOR_VALID_ACCEL, SENSOR_VALID_CLOCK, SENSOR_VALID_GPS,
    SENSOR_VALID_GYRO, SENSOR_VALID_STEERING,
};

pub const PROBE_STEPS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposedProbeResult {
    pub step: u32,
    pub radius_q12: i32,
    pub truth_checksum: u32,
    pub sensor_checksum: u32,
    pub nav_checksum: u32,
    pub flight_checksum: u32,
}

pub fn run_composed_probe(scenario: &Phase2Scenario) -> Option<ComposedProbeResult> {
    let mut world = WorldMachine::new_commanded(scenario).ok()?;
    let mut actuator = SteeringActuator::new(0);
    let mut sensors = SensorSuite::new(0x4b53_4133, SensorFaults::default());
    let mut flight = FlightComputer::new();
    let bootstrap = WorldSnapshot {
        truth: world.truth(),
        pitch: PitchAngle::RADIAL,
        mach: Mach::ZERO,
        dynamic_pressure: DynamicPressure::ZERO,
        events: 0,
        truth_checksum: world.truth_checksum(),
    };
    let mut output = flight.step(&sensors.sample(bootstrap, actuator.snapshot()));
    while world.truth().step() < PROBE_STEPS {
        let steering = actuator.advance(output.command.desired_pitch);
        let snapshot = world
            .step_commanded(WorldCommand {
                pitch: steering.applied,
                engine_action: output.command.engine_action,
                separate: output.command.separate,
                abort_safeing: output.command.abort_safeing,
            })
            .ok()?;
        output = flight.step(&sensors.sample(snapshot, steering));
    }
    Some(ComposedProbeResult {
        step: world.truth().step(),
        radius_q12: world.truth().radius().raw(),
        truth_checksum: world.truth_checksum(),
        sensor_checksum: sensors.checksum(),
        nav_checksum: output.nav_checksum,
        flight_checksum: output.flight_checksum,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuidanceProbeResult {
    pub sequence: u32,
    pub mode: FlightMode,
    pub alarms: u16,
    pub desired_pitch: u16,
    pub nav_checksum: u32,
    pub flight_checksum: u32,
}

pub fn run_guidance_probe(stuck_steering: bool) -> GuidanceProbeResult {
    let mut flight = FlightComputer::new();
    let mut steering = 0u16;
    let mut output = ksa64_interface::FlightOutput {
        sequence: 0,
        nav_time_q16: 0,
        nav_radius_q12: EARTH_RADIUS_Q12,
        nav_downrange_q32: 0,
        nav_radial_velocity_q24: 0,
        nav_tangential_velocity_q24: 7_803_689,
        nav_pitch: 0,
        mode: FlightMode::Boot,
        alarms: 0,
        command: ksa64_interface::ActuatorCommand::SAFE,
        nav_checksum: 0,
        flight_checksum: 0,
    };
    let mut sequence = 0u32;
    while sequence < PROBE_STEPS {
        let frame = SensorFrame {
            sequence,
            onboard_time_q16: (sequence as i32) * 8_192,
            accel_radial_q28: 0,
            accel_tangential_q28: 0,
            gyro_rate_q24: 0,
            steering_pitch: if stuck_steering { 14_564 } else { steering },
            validity: SENSOR_VALID_ACCEL
                | SENSOR_VALID_GYRO
                | SENSOR_VALID_STEERING
                | SENSOR_VALID_CLOCK
                | SENSOR_VALID_GPS,
            altitude_q12: 0,
            gps_radius_q12: 26_900_000,
            gps_downrange_q32: sequence as i32 * 90_000,
            gps_radial_velocity_q24: 0,
            gps_tangential_velocity_q24: 130_700_000,
            events: 0,
            active_stage: 1,
            stage_phase: StagePhase::Burning,
            engine_on: true,
        };
        output = flight.step(&frame);
        steering = output.command.desired_pitch;
        sequence += 1;
    }
    GuidanceProbeResult {
        sequence: output.sequence,
        mode: output.mode,
        alarms: output.alarms,
        desired_pitch: output.command.desired_pitch,
        nav_checksum: output.nav_checksum,
        flight_checksum: output.flight_checksum,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoastProbeResult {
    pub step: u32,
    pub radius_q12: i32,
    pub radial_velocity_q24: i32,
    pub angular_momentum_q14: i32,
}

pub fn run_coast_probe() -> Option<CoastProbeResult> {
    let world = PlanarWorld::simple_earth(Time::from_raw(8_192));
    let mut truth = PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(EARTH_RADIUS_Q12 + 200 * 4_096),
        DownrangeAngle::ZERO,
        PlanarVelocity::ZERO,
        SpecificAngularMomentum::from_raw(838_958_251),
        Mass::from_raw(23 * 4_096),
        Mass::ZERO,
        1,
        TruthStagePhase::Complete,
    );
    let mut status = NumericStatus::CLEAR;
    let mut step = 0;
    while step < PROBE_STEPS {
        truth = advance_vacuum_semi_implicit(world, truth, &mut status).ok()?;
        step += 1;
    }
    if !status.is_clear() || EARTH_MU_Q12 == 0 {
        return None;
    }
    Some(CoastProbeResult {
        step: truth.step(),
        radius_q12: truth.radius().raw(),
        radial_velocity_q24: truth.radial_velocity().raw(),
        angular_momentum_q14: truth.specific_angular_momentum().raw(),
    })
}

pub fn run_actuator_probe() -> u32 {
    let mut actuator = SteeringActuator::new(0);
    let mut hash = 2_166_136_261u32;
    let mut step = 0;
    while step < PROBE_STEPS {
        let requested = if step < 32 { 16_384 } else { 8_192 };
        let snapshot = actuator.advance(requested);
        hash ^= snapshot.applied as u32 | ((snapshot.lagged_target as u32) << 16);
        hash = hash.wrapping_mul(16_777_619);
        step += 1;
    }
    hash
}

pub fn command_is_safe_after_fault(result: GuidanceProbeResult) -> bool {
    result.mode == FlightMode::Abort && result.alarms != 0
}
