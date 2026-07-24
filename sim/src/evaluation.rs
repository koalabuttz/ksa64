//! Phase 7 evaluation façade over accepted legacy mission executors.
//!
//! These adapters intentionally call the frozen Phase 2 and Phase 5 entry
//! points. They normalize only the result envelope; they do not alter mission
//! composition, arithmetic, operation order, or canonical artifacts.

use crate::phase5_closed_loop::Phase5ClosedLoopError;
use crate::phase5_mission::{
    run_phase5_mission, Phase5MissionCase, Phase5MissionOutcome, Phase5MissionSummary,
};
use ksa64_core::evaluation::{EvaluationOutcome, EvaluationSummary, MetricSlot, ModelProfileId};
use ksa64_core::phase2_mission::{
    execute_phase2_mission, Phase2MissionError, Phase2MissionOutcome, Phase2MissionResult,
};
use ksa64_core::phase2_numeric::{
    EARTH_RADIUS_Q12, PHASE2_ENVIRONMENT_ID, PHASE2_NUMERIC_CONTRACT_ID,
};
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::phase5_contract::{
    PHASE5_ENVIRONMENT_ID, PHASE5_NUMERIC_CONTRACT_ID, PHASE5_SCENARIO_ID,
};
use ksa64_core::phase7_mission::{
    execute_hobby_mission, HobbyMissionExecutionError, HobbyMissionOutcome, HobbyMissionResult,
};
use ksa64_core::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};
use ksa64_core::planar::OrbitClass;

pub enum EvaluationRequest<'a> {
    LegacyKsa2PlanarV1(&'a Phase2Scenario),
    LegacyKsa5SpatialV1(Phase5MissionCase),
    HobbyVerticalV1 {
        vehicle: VerticalVehiclePack,
        motor: &'a MotorPack,
        mission: HobbyMissionPack,
    },
}

impl EvaluationRequest<'_> {
    pub const fn profile(&self) -> ModelProfileId {
        match self {
            Self::LegacyKsa2PlanarV1(_) => ModelProfileId::LegacyKsa2PlanarV1,
            Self::LegacyKsa5SpatialV1(_) => ModelProfileId::LegacyKsa5SpatialV1,
            Self::HobbyVerticalV1 { .. } => ModelProfileId::HobbyVerticalV1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    LegacyKsa2(Phase2MissionError),
    LegacyKsa5(Phase5ClosedLoopError),
    HobbyConfiguration,
}

pub fn evaluate(request: EvaluationRequest<'_>) -> Result<EvaluationSummary, EvaluationError> {
    match request {
        EvaluationRequest::LegacyKsa2PlanarV1(scenario) => execute_phase2_mission(scenario)
            .map(|result| adapt_phase2(scenario, result))
            .map_err(EvaluationError::LegacyKsa2),
        EvaluationRequest::LegacyKsa5SpatialV1(case) => run_phase5_mission(case)
            .map(adapt_phase5)
            .map_err(EvaluationError::LegacyKsa5),
        EvaluationRequest::HobbyVerticalV1 {
            vehicle,
            motor,
            mission,
        } => execute_hobby_mission(vehicle, motor, mission)
            .map(|result| adapt_hobby(vehicle, motor, mission, result))
            .map_err(|error| match error {
                HobbyMissionExecutionError::Configuration => EvaluationError::HobbyConfiguration,
                HobbyMissionExecutionError::Observer(never) => match never {},
            }),
    }
}

fn adapt_phase2(scenario: &Phase2Scenario, result: Phase2MissionResult) -> EvaluationSummary {
    let truth = result.truth();
    let orbit = result.terminal_orbit();
    let outcome = match result.outcome() {
        Phase2MissionOutcome::Impact => EvaluationOutcome::GroundContact,
        Phase2MissionOutcome::DurationComplete => match orbit.map(|value| value.class()) {
            Some(OrbitClass::StableOrbit) => EvaluationOutcome::StableOrbit,
            Some(OrbitClass::Impact) => EvaluationOutcome::GroundContact,
            _ => EvaluationOutcome::CompleteNotOrbit,
        },
    };
    let mut summary = EvaluationSummary::empty(ModelProfileId::LegacyKsa2PlanarV1);
    summary.outcome = outcome;
    summary.steps = truth.step();
    summary.terminal_state_a = [truth.radius().raw(), truth.downrange().raw(), 0];
    summary.terminal_state_b = [
        truth.radial_velocity().raw(),
        truth.specific_angular_momentum().raw(),
        0,
    ];
    summary.set_metric(
        MetricSlot::MaxDynamicPressure,
        result.max_dynamic_pressure().raw(),
    );
    summary.set_metric(
        MetricSlot::MaxAcceleration,
        result.max_proper_acceleration().raw(),
    );
    summary.set_metric(MetricSlot::TerminalMass, truth.total_mass().raw());
    if let Some(solution) = orbit {
        summary.set_metric(
            MetricSlot::PerigeeAltitude,
            solution.perigee().raw() - EARTH_RADIUS_Q12,
        );
        summary.set_metric(
            MetricSlot::ApogeeAltitude,
            solution.apogee().raw() - EARTH_RADIUS_Q12,
        );
    }
    summary.events = result.event_history() as u32;
    summary.identities = [
        PHASE2_NUMERIC_CONTRACT_ID,
        PHASE2_ENVIRONMENT_ID,
        scenario.scenario_id(),
        0,
        0,
        0,
    ];
    summary.source_checksums[0] = result.state_checksum();
    summary
}

fn adapt_phase5(result: Phase5MissionSummary) -> EvaluationSummary {
    let outcome = match result.outcome {
        Phase5MissionOutcome::StableOrbit => EvaluationOutcome::StableOrbit,
        Phase5MissionOutcome::CompleteNotOrbit => EvaluationOutcome::CompleteNotOrbit,
        Phase5MissionOutcome::Aborted => EvaluationOutcome::Aborted,
        Phase5MissionOutcome::NumericFault => EvaluationOutcome::NumericFault,
        Phase5MissionOutcome::StepLimit => EvaluationOutcome::StepLimit,
    };
    let mut summary = EvaluationSummary::empty(ModelProfileId::LegacyKsa5SpatialV1);
    summary.outcome = outcome;
    summary.steps = result.steps;
    summary.terminal_state_a = result.terminal_position_q12;
    summary.terminal_state_b = result.terminal_velocity_q24;
    summary.set_metric(MetricSlot::PerigeeAltitude, result.perigee_altitude_q12);
    summary.set_metric(MetricSlot::ApogeeAltitude, result.apogee_altitude_q12);
    summary.set_metric(MetricSlot::Inclination, result.inclination_turn16 as i32);
    summary.set_metric(
        MetricSlot::MaxDynamicPressure,
        result.max_dynamic_pressure_q16,
    );
    summary.set_metric(
        MetricSlot::MaxNavigationError,
        result.max_nav_position_error_q12,
    );
    summary.events = result.events as u32;
    summary.identities = [
        PHASE5_NUMERIC_CONTRACT_ID,
        PHASE5_ENVIRONMENT_ID,
        PHASE5_SCENARIO_ID,
        result.case as u32,
        0,
        0,
    ];
    summary.source_checksums = [
        result.sensor_checksum,
        result.navigation_checksum,
        result.flight_checksum,
        result.summary_checksum,
        0,
    ];
    summary
}

fn adapt_hobby(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
    result: HobbyMissionResult,
) -> EvaluationSummary {
    let outcome = match result.outcome {
        HobbyMissionOutcome::Landed => EvaluationOutcome::GroundContact,
        HobbyMissionOutcome::NoLiftoff => EvaluationOutcome::NoLiftoff,
        HobbyMissionOutcome::RecoveryIncomplete => EvaluationOutcome::RecoveryIncomplete,
        HobbyMissionOutcome::NumericFault => EvaluationOutcome::NumericFault,
        HobbyMissionOutcome::StepLimit => EvaluationOutcome::StepLimit,
        HobbyMissionOutcome::ConfigurationFault => EvaluationOutcome::ConfigurationFault,
    };
    let mut summary = EvaluationSummary::empty(ModelProfileId::HobbyVerticalV1);
    summary.outcome = outcome;
    summary.numeric_faults = result.numeric_faults;
    summary.steps = result.terminal.step;
    summary.terminal_state_a = [
        result.terminal.altitude.raw(),
        result.terminal.velocity.raw(),
        result.terminal.mass.raw(),
    ];
    summary.terminal_state_b = [
        result.terminal.time.raw(),
        result.terminal.propellant.raw(),
        result.terminal.phase as i32,
    ];
    summary.set_metric(MetricSlot::ApogeeAltitude, result.max_altitude.raw());
    summary.set_metric(
        MetricSlot::MaxDynamicPressure,
        result.max_dynamic_pressure.raw(),
    );
    summary.set_metric(MetricSlot::MaxAcceleration, result.max_acceleration.raw());
    summary.set_metric(MetricSlot::MaxSpeed, result.max_speed.raw());
    summary.set_metric(MetricSlot::MaxMach, result.max_mach.raw());
    summary.set_metric(
        MetricSlot::MaxOpeningDeceleration,
        result.max_opening_deceleration.raw(),
    );
    summary.set_metric(MetricSlot::TerminalMass, result.terminal.mass.raw());
    for (slot, milestone, value) in [
        (
            MetricSlot::RailExitTime,
            result.rail_exit,
            result.rail_exit.time_raw,
        ),
        (
            MetricSlot::RailExitVelocity,
            result.rail_exit,
            result.rail_exit.velocity_raw,
        ),
        (
            MetricSlot::BurnoutTime,
            result.burnout,
            result.burnout.time_raw,
        ),
        (
            MetricSlot::BurnoutAltitude,
            result.burnout,
            result.burnout.altitude_raw,
        ),
        (
            MetricSlot::BurnoutVelocity,
            result.burnout,
            result.burnout.velocity_raw,
        ),
        (
            MetricSlot::ApogeeTime,
            result.apogee,
            result.apogee.time_raw,
        ),
        (
            MetricSlot::DrogueTime,
            result.drogue,
            result.drogue.time_raw,
        ),
        (
            MetricSlot::DrogueAltitude,
            result.drogue,
            result.drogue.altitude_raw,
        ),
        (
            MetricSlot::DrogueVelocity,
            result.drogue,
            result.drogue.velocity_raw,
        ),
        (MetricSlot::MainTime, result.main, result.main.time_raw),
        (
            MetricSlot::MainAltitude,
            result.main,
            result.main.altitude_raw,
        ),
        (
            MetricSlot::MainVelocity,
            result.main,
            result.main.velocity_raw,
        ),
        (
            MetricSlot::GroundContactTime,
            result.ground,
            result.ground.time_raw,
        ),
        (
            MetricSlot::ImpactVelocity,
            result.ground,
            result.ground.velocity_raw,
        ),
    ] {
        if milestone.valid {
            summary.set_metric(slot, value);
        }
    }
    summary.events = result.event_history;
    summary.identities = [
        ksa64_core::phase7_numeric::HOBBY_NUMERIC_CONTRACT_ID,
        ksa64_core::phase7_numeric::HOBBY_ENVIRONMENT_ID,
        vehicle.identity,
        motor.identity,
        mission.identity,
        0,
    ];
    summary.source_checksums[0] = result.state_checksum;
    summary
}
