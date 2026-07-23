//! Six-step closed-loop Phase 3 mission composition.

use crate::actuator::{SteeringActuator, SteeringSnapshot};
use crate::sensors::{SensorFaults, SensorSuite, StepWindow};
use crate::world::{WorldCommand, WorldError, WorldMachine, WorldSnapshot};
use ksa64_core::numeric::{add, multiply_scaled, NumericStatus};
use ksa64_core::phase2_numeric::{sqrt_floor_u32, EARTH_RADIUS_Q12};
use ksa64_core::phase2_quantities::{DynamicPressure, PitchAngle, PlanarAcceleration};
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::planar::{
    classify_orbit, evaluate_vacuum, OrbitSolution, PlanarTruthState, PlanarWorld,
};
use ksa64_flight::gnc::{FlightComputer, FlightStatus};
use ksa64_flight::navigation::NavigationState;
use ksa64_interface::{EngineAction, FlightOutput, SensorFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionCase {
    Nominal,
    AltimeterDropout,
    GpsOutage,
    SteeringStuck,
}
impl MissionCase {
    pub const fn sensor_faults(self) -> SensorFaults {
        match self {
            Self::AltimeterDropout => SensorFaults {
                altimeter_dropout: Some(StepWindow {
                    start: 360,
                    end: 480,
                }),
                gps_outage: None,
            },
            Self::GpsOutage => SensorFaults {
                altimeter_dropout: None,
                gps_outage: Some(StepWindow {
                    start: 2080,
                    end: 2560,
                }),
            },
            _ => SensorFaults {
                altimeter_dropout: None,
                gps_outage: None,
            },
        }
    }
    pub const fn steering_stuck_step(self) -> Option<u32> {
        if matches!(self, Self::SteeringStuck) {
            Some(2080)
        } else {
            None
        }
    }
    pub const fn seed(self) -> u32 {
        match self {
            Self::Nominal => 0x4b53_4133,
            Self::AltimeterDropout => 0x4b53_4134,
            Self::GpsOutage => 0x4b53_4135,
            Self::SteeringStuck => 0x4b53_4136,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionOutcome {
    DurationComplete,
    Impact,
    Abort,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionError {
    World { step: u32, error: WorldError },
    Numeric,
}
#[derive(Clone, Copy, Debug)]
pub struct MissionResult {
    pub case: MissionCase,
    pub outcome: MissionOutcome,
    pub truth: PlanarTruthState,
    pub orbit: Option<OrbitSolution>,
    pub max_dynamic_pressure: DynamicPressure,
    pub max_proper_acceleration: PlanarAcceleration,
    pub abort_step: u32,
    pub cutoff_step: u32,
    pub cutoff_truth: PlanarTruthState,
    pub cutoff_navigation: NavigationState,
    pub recovery_requested: bool,
    pub truth_checksum: u32,
    pub sensor_checksum: u32,
    pub nav_checksum: u32,
    pub flight_checksum: u32,
    pub flight_status: FlightStatus,
}

pub trait MissionObserver {
    type Error;
    fn observe(&mut self, record: MissionRecord) -> Result<(), Self::Error>;
}
#[derive(Clone, Copy, Debug)]
pub struct MissionRecord {
    pub world: WorldSnapshot,
    pub steering: SteeringSnapshot,
    pub sensors: SensorFrame,
    pub flight: FlightOutput,
    pub sensor_checksum: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionRunError<E> {
    Mission(MissionError),
    Observer(E),
}
struct NullObserver;
impl MissionObserver for NullObserver {
    type Error = core::convert::Infallible;
    fn observe(&mut self, _: MissionRecord) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn run_phase3_mission(
    scenario: &Phase2Scenario,
    case: MissionCase,
) -> Result<MissionResult, MissionError> {
    let mut observer = NullObserver;
    match run_phase3_mission_observed(scenario, case, &mut observer) {
        Ok(result) => Ok(result),
        Err(MissionRunError::Mission(error)) => Err(error),
        Err(MissionRunError::Observer(never)) => match never {},
    }
}

pub fn run_phase3_mission_observed<O: MissionObserver>(
    scenario: &Phase2Scenario,
    case: MissionCase,
    observer: &mut O,
) -> Result<MissionResult, MissionRunError<O::Error>> {
    let mut world = WorldMachine::new_commanded(scenario)
        .map_err(|e| MissionRunError::Mission(MissionError::World { step: 0, error: e }))?;
    let mut actuator = SteeringActuator::new(0);
    let mut sensors = SensorSuite::new(case.seed(), case.sensor_faults());
    let mut flight = FlightComputer::new();
    let bootstrap_world = WorldSnapshot {
        truth: world.truth(),
        pitch: PitchAngle::RADIAL,
        mach: ksa64_core::phase2_quantities::Mach::ZERO,
        dynamic_pressure: DynamicPressure::ZERO,
        events: 0,
        truth_checksum: world.truth_checksum(),
    };
    let bootstrap_steering = actuator.snapshot();
    let bootstrap_sensor = sensors.sample(bootstrap_world, bootstrap_steering);
    let mut output = flight.step(&bootstrap_sensor);
    observer
        .observe(MissionRecord {
            world: bootstrap_world,
            steering: bootstrap_steering,
            sensors: bootstrap_sensor,
            flight: output,
            sensor_checksum: sensors.checksum(),
        })
        .map_err(MissionRunError::Observer)?;
    let mut max_q = 0;
    let mut max_proper = 0;
    let mut abort_step = 0;
    let mut cutoff_step = 0;
    let mut recovery_requested = false;
    let mut cutoff_truth = world.truth();
    let mut cutoff_navigation = flight.navigation();
    let mut outcome = MissionOutcome::DurationComplete;
    while world.truth().step() < scenario.steps() {
        if case
            .steering_stuck_step()
            .map(|s| world.truth().step() == s)
            .unwrap_or(false)
        {
            actuator.jam_at(14_564); // 80-degree off-nominal jam.
        }
        let steering = actuator.advance(output.command.desired_pitch);
        let command = WorldCommand {
            pitch: steering.applied,
            engine_action: output.command.engine_action,
            separate: output.command.separate,
            abort_safeing: output.command.abort_safeing,
        };
        let snapshot = world.step_commanded(command).map_err(|e| {
            MissionRunError::Mission(MissionError::World {
                step: world.truth().step(),
                error: e,
            })
        })?;
        max_q = max_q.max(snapshot.dynamic_pressure.raw());
        max_proper = max_proper.max(proper_acceleration(snapshot.truth, scenario));
        let sensor = sensors.sample(snapshot, steering);
        output = flight.step(&sensor);
        if output.command.engine_action == EngineAction::Cutoff
            && snapshot.truth.active_stage() == 1
            && cutoff_step == 0
        {
            cutoff_step = snapshot.truth.step().saturating_add(1);
            cutoff_truth = snapshot.truth;
            cutoff_navigation = flight.navigation();
        }
        if output.command.abort_safeing && abort_step == 0 {
            abort_step = snapshot.truth.step();
            outcome = MissionOutcome::Abort
        }
        recovery_requested |= output.command.recovery_requested;
        observer
            .observe(MissionRecord {
                world: snapshot,
                steering,
                sensors: sensor,
                flight: output,
                sensor_checksum: sensors.checksum(),
            })
            .map_err(MissionRunError::Observer)?;
        if snapshot.truth.radius().raw() < EARTH_RADIUS_Q12 {
            outcome = if outcome == MissionOutcome::Abort {
                MissionOutcome::Abort
            } else {
                MissionOutcome::Impact
            };
            break;
        }
    }
    let mut status = NumericStatus::CLEAR;
    let orbit = classify_orbit(
        PlanarWorld::simple_earth(scenario.timestep()),
        world.truth(),
        &mut status,
    );
    if !status.is_clear() {
        return Err(MissionRunError::Mission(MissionError::Numeric));
    }
    Ok(MissionResult {
        case,
        outcome,
        truth: world.truth(),
        orbit,
        max_dynamic_pressure: DynamicPressure::from_raw(max_q),
        max_proper_acceleration: PlanarAcceleration::from_raw(max_proper),
        abort_step,
        cutoff_step,
        cutoff_truth,
        cutoff_navigation,
        recovery_requested,
        truth_checksum: world.truth_checksum(),
        sensor_checksum: sensors.checksum(),
        nav_checksum: output.nav_checksum,
        flight_checksum: output.flight_checksum,
        flight_status: flight.status(),
    })
}

fn proper_acceleration(truth: PlanarTruthState, scenario: &Phase2Scenario) -> i32 {
    let mut status = NumericStatus::CLEAR;
    let vacuum = evaluate_vacuum(
        PlanarWorld::simple_earth(scenario.timestep()),
        truth,
        &mut status,
    );
    let radial = truth.radial_acceleration().raw() - vacuum.radial_acceleration().raw();
    let radial2 = multiply_scaled(radial, radial, 28, &mut status);
    let tangential2 = multiply_scaled(
        truth.tangential_acceleration().raw(),
        truth.tangential_acceleration().raw(),
        28,
        &mut status,
    );
    let total = add(radial2, tangential2, &mut status).max(0) as u32;
    if !status.is_clear() {
        return i32::MAX;
    }
    sqrt_floor_u32(total << 4) as i32 * (1 << 12)
}
