use ksa64_core::dynamics::VerticalStepError;
use ksa64_core::mission::{hash_vertical_truth, run_vertical_mission, VERTICAL_CHECKSUM_OFFSET};
use ksa64_core::numeric::NumericFault;
use ksa64_core::scenario::{crc32_ieee, parse_scenario_image};
use ksa64_core::vehicle::VerticalTruthState;

const SCENARIO: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

mod expected {
    include!("../../phase1/generated/mission_v1.rs");
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn repair_crc(bytes: &mut [u8; 76]) {
    let checksum = crc32_ieee(&bytes[..72]);
    bytes[72..76].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn golden_mission_matches_independent_summary_and_checksum() {
    let scenario = parse_scenario_image(SCENARIO).unwrap();
    let initial = VerticalTruthState::initial(&scenario);
    assert_eq!(
        hash_vertical_truth(VERTICAL_CHECKSUM_OFFSET, &initial),
        expected::INITIAL_TRUTH_CHECKSUM
    );

    let summary = run_vertical_mission(&scenario).unwrap();
    let final_truth = summary.final_truth();
    assert_eq!(summary.completed_steps(), expected::FINAL_STEP);
    assert_eq!(final_truth.time().raw(), expected::FINAL_TIME_Q16);
    assert_eq!(final_truth.altitude().raw(), expected::FINAL_ALTITUDE_Q12);
    assert_eq!(final_truth.velocity().raw(), expected::FINAL_VELOCITY_Q24);
    assert_eq!(
        final_truth.acceleration().raw(),
        expected::FINAL_ACCELERATION_Q28
    );
    assert_eq!(final_truth.total_mass().raw(), expected::FINAL_MASS_Q12);
    assert_eq!(
        final_truth.propellant().raw(),
        expected::FINAL_PROPELLANT_Q12
    );
    assert_eq!(summary.checksum(), expected::FINAL_CHECKSUM);
    assert_eq!(summary.cutoff_events(), expected::CUTOFF_EVENTS);
}

#[test]
fn failed_mission_preserves_last_valid_truth_and_checksum() {
    let mut image = *SCENARIO;
    write_i32(&mut image, 36, 134_217_728);
    repair_crc(&mut image);
    let scenario = parse_scenario_image(&image).unwrap();
    let initial = VerticalTruthState::initial(&scenario);
    let failure = run_vertical_mission(&scenario).unwrap_err();

    assert_eq!(failure.last_truth(), initial);
    assert_eq!(failure.last_truth().step(), 0);
    assert_eq!(failure.checksum(), VERTICAL_CHECKSUM_OFFSET);
    assert_eq!(failure.cutoff_events(), 0);
    assert_eq!(failure.cause(), VerticalStepError::NumericFault);
    assert!(failure
        .numeric_status()
        .contains(NumericFault::InvalidInput));
}
