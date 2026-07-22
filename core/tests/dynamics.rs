use ksa64_core::dynamics::evaluate_vertical_forces;
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
