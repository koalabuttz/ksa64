use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_mission::{
    execute_phase2_mission_observed, Phase2Observation, Phase2Observer, EVENT_CUTOFF,
    EVENT_IGNITION, PLANAR_CHECKSUM_OFFSET,
};
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::planar::StagePhase;
use ksa64_interface::EngineAction;
use ksa64_sim::actuator::{SteeringActuator, MAX_SLEW_PER_STEP};
use ksa64_sim::world::{WorldCommand, WorldMachine};

const SCENARIO_IMAGE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
struct NullObserver;
impl Phase2Observer for NullObserver {
    type Error = ();
    fn observe(&mut self, _: Phase2Observation) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn compatibility_stepper_reproduces_phase2_exactly() {
    let scenario = parse_phase2_scenario(SCENARIO_IMAGE).unwrap();
    let mut machine = WorldMachine::new_compatibility(&scenario).unwrap();
    while machine.truth().step() < scenario.steps() {
        let mut status = NumericStatus::CLEAR;
        let pitch = scenario
            .pitch_program()
            .pitch_at(machine.truth().time(), &mut status);
        assert!(status.is_clear());
        machine.step_compatibility(pitch).unwrap();
    }
    let legacy = execute_phase2_mission_observed(&scenario, &mut NullObserver).unwrap();
    assert_eq!(machine.truth(), legacy.truth());
    assert_eq!(machine.truth_checksum(), legacy.state_checksum());
    assert_eq!(machine.truth_checksum(), 0xcc57_612b);
    assert_ne!(machine.truth_checksum(), PLANAR_CHECKSUM_OFFSET);
}

#[test]
fn commanded_world_enforces_ignition_cutoff_and_separation_delay() {
    let scenario = parse_phase2_scenario(SCENARIO_IMAGE).unwrap();
    let mut machine = WorldMachine::new_commanded(&scenario).unwrap();
    let hold = WorldCommand {
        pitch: 0,
        engine_action: EngineAction::Hold,
        separate: false,
        abort_safeing: false,
    };
    let snapshot = machine.step_commanded(hold).unwrap();
    assert_eq!(
        snapshot.truth.stage_phase(),
        StagePhase::CoastBeforeIgnition
    );
    let ignition = machine
        .step_commanded(WorldCommand {
            engine_action: EngineAction::Ignite,
            ..hold
        })
        .unwrap();
    assert_eq!(ignition.truth.stage_phase(), StagePhase::Burning);
    assert_ne!(ignition.events & EVENT_IGNITION, 0);
    let cutoff = machine
        .step_commanded(WorldCommand {
            engine_action: EngineAction::Cutoff,
            ..hold
        })
        .unwrap();
    assert_eq!(
        cutoff.truth.stage_phase(),
        StagePhase::CoastBeforeSeparation
    );
    assert_ne!(cutoff.events & EVENT_CUTOFF, 0);
    let stage = cutoff.truth.active_stage();
    for _ in 0..6 {
        machine.step_commanded(hold).unwrap();
    }
    machine
        .step_commanded(WorldCommand {
            separate: true,
            ..hold
        })
        .unwrap();
    assert_eq!(machine.truth().active_stage(), stage);
    machine
        .step_commanded(WorldCommand {
            separate: true,
            ..hold
        })
        .unwrap();
    assert_eq!(machine.truth().active_stage(), stage + 1);
}

#[test]
fn actuator_has_lag_slew_limits_clamps_and_stuck_feedback() {
    let mut actuator = SteeringActuator::new(0);
    let first = actuator.advance(u16::MAX);
    assert_eq!(first.requested, 20_025);
    assert_eq!(first.applied, MAX_SLEW_PER_STEP as u16);
    assert!(first.lagged_target > first.applied);
    actuator.set_stuck(true);
    let frozen = actuator.advance(0);
    assert_eq!(frozen.applied, first.applied);
    assert!(frozen.stuck);
    actuator.jam_at(14_564);
    let jammed = actuator.advance(20_000);
    assert_eq!(jammed.applied, 14_564);
    assert!(jammed.stuck);
}
