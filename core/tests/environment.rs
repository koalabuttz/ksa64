use core::mem::size_of;

use ksa64_core::environment::SimpleEarthEnvironment;
use ksa64_core::numeric::NumericStatus;
use ksa64_core::quantities::Altitude;
use ksa64_core::scenario::parse_scenario_image;
use ksa64_core::vehicle::VerticalTruthState;

const SCENARIO: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

#[test]
fn generated_environment_tables_validate_once() {
    assert!(SimpleEarthEnvironment::new().tables_are_valid());
}

#[test]
fn environment_matches_frozen_interpolation_points() {
    let environment = SimpleEarthEnvironment::new();
    let cases = [
        (-4_096, 328_833_434, 2_632_453),
        (0, 328_833_434, 2_632_453),
        (4_096, 299_505_518, 2_631_627),
        (14_336, 233_888_618, 2_629_563),
        (225_280, 212_321, 2_587_597),
        (491_520, 0, 2_536_019),
        (2_048_000, 0, 2_263_267),
        (10_240_000, 0, 1_524_829),
    ];
    let mut status = NumericStatus::CLEAR;
    for (altitude, density, gravity) in cases {
        let sample = environment.sample(Altitude::from_raw(altitude), &mut status);
        assert_eq!(sample.density().raw(), density);
        assert_eq!(sample.gravity().raw(), gravity);
    }
    assert!(status.is_clear());
}

#[test]
fn validated_scenario_initializes_immutable_truth_exactly() {
    let scenario = parse_scenario_image(SCENARIO).unwrap();
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let truth = VerticalTruthState::initial(&scenario);
    assert_eq!(truth.step(), 0);
    assert_eq!(truth.time().raw(), 0);
    assert_eq!(truth.altitude(), scenario.initial().altitude());
    assert_eq!(truth.velocity(), scenario.initial().velocity());
    assert_eq!(truth.acceleration().raw(), 0);
    assert_eq!(truth.total_mass(), scenario.initial().total_mass());
    assert_eq!(truth.propellant(), scenario.initial().propellant());
    assert_eq!(size_of::<VerticalTruthState>(), 28);

    let mut status = NumericStatus::CLEAR;
    let initial_environment = environment.sample(truth.altitude(), &mut status);
    assert_eq!(initial_environment.density().raw(), 328_833_434);
    assert_eq!(initial_environment.gravity().raw(), 2_632_453);
    assert!(status.is_clear());
}
