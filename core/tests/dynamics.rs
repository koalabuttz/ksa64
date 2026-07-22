use ksa64_core::dynamics::{advance_vertical_state, evaluate_vertical_forces, VerticalStepError};
use ksa64_core::environment::SimpleEarthEnvironment;
use ksa64_core::numeric::NumericStatus;
use ksa64_core::scenario::parse_scenario_image;
use ksa64_core::vehicle::VerticalTruthState;

const SCENARIO: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

#[test]
fn initial_force_snapshot_is_pure_and_exact() {
    let scenario = parse_scenario_image(SCENARIO).unwrap();
    let truth = VerticalTruthState::initial(&scenario);
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let mut status = NumericStatus::CLEAR;
    let sample = environment.sample(truth.altitude(), &mut status);
    let before = truth;
    let forces = evaluate_vertical_forces(scenario.vehicle(), &truth, sample, &mut status);

    assert_eq!(truth, before);
    assert!(forces.engine_active());
    assert_eq!(forces.thrust().raw(), 31_130);
    assert_eq!(forces.weight().raw(), 20_084);
    assert_eq!(forces.drag().raw(), 0);
    assert_eq!(forces.net_force().raw(), 11_046);
    assert_eq!(forces.acceleration().raw(), 1_447_821);
    assert!(status.is_clear());
}

#[test]
fn initial_transition_matches_exact_semi_implicit_euler_case() {
    let scenario = parse_scenario_image(SCENARIO).unwrap();
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let truth = VerticalTruthState::initial(&scenario);
    let before = truth;
    let mut status = NumericStatus::CLEAR;
    let result = advance_vertical_state(&scenario, environment, &truth, &mut status).unwrap();
    let next = result.truth();

    assert_eq!(truth, before);
    assert_eq!(next.step(), 1);
    assert_eq!(next.time().raw(), 8_192);
    assert_eq!(next.altitude().raw(), 0);
    assert_eq!(next.velocity().raw(), 11_311);
    assert_eq!(next.acceleration().raw(), 1_447_821);
    assert_eq!(next.total_mass().raw(), 2_046_720);
    assert_eq!(next.propellant().raw(), 1_555_200);
    assert_eq!(result.propellant_consumed().raw(), 1_280);
    assert!(!result.engine_cutoff());
    assert!(status.is_clear());
}

#[test]
fn transition_refuses_to_advance_with_a_preexisting_fault() {
    let scenario = parse_scenario_image(SCENARIO).unwrap();
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let truth = VerticalTruthState::initial(&scenario);
    let before = truth;
    let mut status = NumericStatus::from_bits(0x08);

    assert_eq!(
        advance_vertical_state(&scenario, environment, &truth, &mut status),
        Err(VerticalStepError::NumericFault)
    );
    assert_eq!(truth, before);
    assert_eq!(status.bits(), 0x08);
}
