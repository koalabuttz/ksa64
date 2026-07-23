use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_mission::{EVENT_CUTOFF, EVENT_IGNITION, EVENT_SEPARATION};
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_interface::EngineAction;
use ksa64_sim::phase5_vehicle::{
    ksa5a_stage, GimbalCommandQ16, Phase5StagePhase, Phase5VehicleCommand, Phase5VehicleMachine,
    TwoAxisGimbalQ16, EVENT_GIMBAL_JAMMED, EVENT_RCS_DEPLETED, PHASE5_GIMBAL_LIMIT_Q16,
    PHASE5_GIMBAL_SLEW_PER_FAST_STEP_Q16, PHASE5_RCS_PROPELLANT_Q12,
};

const SCENARIO_IMAGE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const WRONG_SCENARIO_IMAGE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-early-cutoff.ksc2");

#[allow(dead_code)]
mod vectors {
    include!("../../phase5/generated/vehicle_v1.rs");
}

fn scenario() -> ksa64_core::phase2_scenario::Phase2Scenario {
    parse_phase2_scenario(SCENARIO_IMAGE).unwrap()
}

fn ignite() -> Phase5VehicleCommand {
    Phase5VehicleCommand {
        engine_action: EngineAction::Ignite,
        ..Phase5VehicleCommand::HOLD
    }
}

fn reach_upper_stage(machine: &mut Phase5VehicleMachine) {
    let first = machine.step(ignite()).unwrap();
    assert_ne!(first.events & EVENT_IGNITION, 0);
    let cutoff = machine
        .step(Phase5VehicleCommand {
            engine_action: EngineAction::Cutoff,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    assert_ne!(cutoff.events & EVENT_CUTOFF, 0);
    for _ in 0..7 {
        machine.step(Phase5VehicleCommand::HOLD).unwrap();
    }
    let separation = machine
        .step(Phase5VehicleCommand {
            separate: true,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    assert_ne!(separation.events & EVENT_SEPARATION, 0);
    assert_eq!(separation.truth.active_stage(), 1);
    assert_eq!(
        separation.truth.phase(),
        Phase5StagePhase::CoastBeforeIgnition
    );
}

#[test]
fn two_axis_gimbal_has_exact_lag_slew_clamp_and_jam_semantics() {
    let mut gimbal = TwoAxisGimbalQ16::new();
    let command = GimbalCommandQ16 {
        pitch: i32::MAX,
        yaw: i32::MIN,
    };
    let mut snapshot = gimbal.advance(command);
    assert_eq!(snapshot.requested.pitch, PHASE5_GIMBAL_LIMIT_Q16);
    assert_eq!(snapshot.requested.yaw, -PHASE5_GIMBAL_LIMIT_Q16);
    assert_eq!(
        snapshot.lagged.pitch,
        vectors::GIMBAL_POSITIVE_LAGGED_APPLIED_Q16[0][0]
    );
    assert_eq!(
        snapshot.lagged.yaw,
        -vectors::GIMBAL_POSITIVE_LAGGED_APPLIED_Q16[0][0]
    );
    assert_eq!(snapshot.applied.pitch, PHASE5_GIMBAL_SLEW_PER_FAST_STEP_Q16);
    assert_eq!(snapshot.applied.yaw, -PHASE5_GIMBAL_SLEW_PER_FAST_STEP_Q16);
    for _ in 1..8 {
        snapshot = gimbal.advance(command);
    }
    assert_eq!(
        snapshot.lagged,
        GimbalCommandQ16 {
            pitch: 6_175,
            yaw: -6_175
        }
    );
    assert_eq!(
        snapshot.applied,
        GimbalCommandQ16 {
            pitch: 2_288,
            yaw: -2_288
        }
    );
    gimbal.set_jammed(true, false);
    let frozen = gimbal.advance(GimbalCommandQ16 { pitch: 0, yaw: 0 });
    assert_eq!(frozen.applied.pitch, snapshot.applied.pitch);
    assert!(frozen.pitch_jammed);
    assert!(!frozen.yaw_jammed);
}

#[test]
fn inertia_schedule_matches_generated_dry_wet_and_midpoint_values() {
    let stage = ksa5a_stage(0).unwrap();
    let mut status = NumericStatus::CLEAR;
    let dry = stage.inertia.interpolate(0, &mut status);
    let wet = stage
        .inertia
        .interpolate(stage.propellant_mass_q12, &mut status);
    let half = stage
        .inertia
        .interpolate(stage.propellant_mass_q12 / 2, &mut status);
    assert_eq!(
        [dry.x(), dry.y(), dry.z()],
        vectors::STAGE_INERTIA_DRY_Q12[0]
    );
    assert_eq!(
        [wet.x(), wet.y(), wet.z()],
        vectors::STAGE_INERTIA_WET_Q12[0]
    );
    assert_eq!(
        [half.x(), half.y(), half.z()],
        vectors::STAGE_INERTIA_HALF_Q12[0]
    );
    assert!(status.is_clear());
}

#[test]
fn one_mission_command_executes_four_fast_steps_atomically() {
    let scenario = scenario();
    let mut machine = Phase5VehicleMachine::new_ksa5a_checked(&scenario).unwrap();
    assert_eq!(machine.truth().total_mass_q12(), 534 << 12);
    assert_eq!(machine.rcs_propellant_q12(), PHASE5_RCS_PROPELLANT_Q12);
    let initial_position = machine.truth().spatial().position();
    let snapshot = machine
        .step(Phase5VehicleCommand {
            gimbal: GimbalCommandQ16 {
                pitch: PHASE5_GIMBAL_LIMIT_Q16,
                yaw: -PHASE5_GIMBAL_LIMIT_Q16,
            },
            engine_action: EngineAction::Ignite,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    assert_eq!(snapshot.truth.step(), 1);
    assert_eq!(snapshot.truth.time_q16(), 8_192);
    assert_eq!(snapshot.truth.phase(), Phase5StagePhase::Burning);
    assert_ne!(snapshot.events & EVENT_IGNITION, 0);
    assert_eq!(snapshot.gimbal.applied.pitch, 1_144);
    assert_eq!(snapshot.gimbal.applied.yaw, -1_144);
    assert!(snapshot.truth.total_mass_q12() < 534 << 12);
    assert!(snapshot.truth.active_propellant_q12() < 400 << 12);
    assert_eq!(snapshot.rcs_propellant_q12, PHASE5_RCS_PROPELLANT_Q12);
    assert_ne!(snapshot.truth.spatial().position(), initial_position);
}

#[test]
fn cutoff_delay_separation_inertia_switch_and_rcs_consumption_are_bounded() {
    let scenario = scenario();
    let mut machine = Phase5VehicleMachine::new_ksa5a_checked(&scenario).unwrap();
    reach_upper_stage(&mut machine);
    assert_eq!(machine.truth().total_mass_q12(), 104 << 12);
    assert_eq!(machine.truth().active_propellant_q12(), 84 << 12);
    let before_rcs = machine.rcs_propellant_q12();
    let rcs = machine
        .step(Phase5VehicleCommand {
            rcs_q15: FixedVec3::new(32_767, 0, 0),
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    assert_eq!(
        before_rcs - rcs.rcs_propellant_q12,
        vectors::RCS_FULL_AXIS_ONE_MISSION_CONSUMED_Q12
    );
    assert!(rcs.truth.rigid().angular_rate().x() > 0);
    assert_eq!(rcs.inertia.x(), 677_880);
    for _ in 0..2 {
        machine.step(Phase5VehicleCommand::HOLD).unwrap();
    }
    let upper_ignition = machine.step(ignite()).unwrap();
    assert_ne!(upper_ignition.events & EVENT_IGNITION, 0);
    assert_eq!(upper_ignition.truth.phase(), Phase5StagePhase::Burning);
}

#[test]
fn jam_and_rcs_leak_faults_are_visible_and_depletion_is_fail_closed() {
    let scenario = scenario();
    let mut machine = Phase5VehicleMachine::new_ksa5a_checked(&scenario).unwrap();
    machine.jam_gimbal_at(700, -900);
    let jammed = machine
        .step(Phase5VehicleCommand {
            gimbal: GimbalCommandQ16 {
                pitch: -2_000,
                yaw: 2_000,
            },
            ..ignite()
        })
        .unwrap();
    assert_ne!(jammed.events & EVENT_GIMBAL_JAMMED, 0);
    assert_eq!(
        jammed.gimbal.applied,
        GimbalCommandQ16 {
            pitch: 700,
            yaw: -900
        }
    );
    machine.set_gimbal_jammed(false, false);
    machine
        .step(Phase5VehicleCommand {
            engine_action: EngineAction::Cutoff,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    for _ in 0..7 {
        machine.step(Phase5VehicleCommand::HOLD).unwrap();
    }
    machine
        .step(Phase5VehicleCommand {
            separate: true,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    machine.set_rcs_leak_q15(FixedVec3::new(32_767, 0, 0));
    let mut depleted_event = false;
    for _ in 0..100 {
        let snapshot = machine.step(Phase5VehicleCommand::HOLD).unwrap();
        depleted_event |= snapshot.events & EVENT_RCS_DEPLETED != 0;
    }
    assert!(depleted_event);
    assert_eq!(machine.rcs_propellant_q12(), 0);
    let rate = machine.truth().rigid().angular_rate();
    let after = machine.step(Phase5VehicleCommand::HOLD).unwrap();
    assert_eq!(after.rcs_propellant_q12, 0);
    assert!(after.truth.rigid().angular_rate().x() <= rate.x());
}

#[test]
fn portable_vehicle_signature_is_frozen() {
    assert_eq!(ksa64_sim::phase5_vehicle_signature(), 0x21e5_5663);
}

#[test]
fn host_compatibility_constructor_rejects_the_wrong_planar_base() {
    let nominal = scenario();
    assert!(Phase5VehicleMachine::new_ksa5a_checked(&nominal).is_ok());
    let wrong = parse_phase2_scenario(WRONG_SCENARIO_IMAGE).unwrap();
    assert!(Phase5VehicleMachine::new_ksa5a_checked(&wrong).is_err());
}
