use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_scenario::{
    parse_phase2_scenario, Phase2ScenarioError, KSA2A_EARLY_CUTOFF_SCENARIO_ID,
    KSA2A_NOMINAL_SCENARIO_ID, PHASE2_SCENARIO_IMAGE_LENGTH,
};
use ksa64_core::planar::StagePhase;
use ksa64_core::scenario::crc32_ieee;

const NOMINAL: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const FAILURE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-early-cutoff.ksc2");

fn repaired(mut image: [u8; PHASE2_SCENARIO_IMAGE_LENGTH]) -> [u8; PHASE2_SCENARIO_IMAGE_LENGTH] {
    let crc = crc32_ieee(&image[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]).to_le_bytes();
    image[PHASE2_SCENARIO_IMAGE_LENGTH - 4..].copy_from_slice(&crc);
    image
}

#[test]
fn nominal_and_failure_images_parse_to_bounded_configurations() {
    let nominal = parse_phase2_scenario(NOMINAL).unwrap();
    let failure = parse_phase2_scenario(FAILURE).unwrap();
    assert_eq!(nominal.scenario_id(), KSA2A_NOMINAL_SCENARIO_ID);
    assert_eq!(failure.scenario_id(), KSA2A_EARLY_CUTOFF_SCENARIO_ID);
    assert_eq!(nominal.stage_count(), 2);
    assert_eq!(nominal.steps(), 7_200);
    assert_eq!(nominal.stage(0).unwrap().burn_steps(), 1_240);
    assert_eq!(nominal.stage(1).unwrap().burn_steps(), 1_920);
    assert_eq!(failure.stage(1).unwrap().burn_steps(), 1_824);
    assert_eq!(failure.flags(), 1);
    assert!(nominal.pitch_program().is_valid(nominal.timestep()));
    assert!(nominal.aero_table(0).unwrap().is_valid());
    assert!(nominal.aero_table(1).unwrap().is_valid());
}

#[test]
fn initial_truth_contains_all_vehicle_mass_and_surface_corotation() {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let mut status = NumericStatus::CLEAR;
    let truth = scenario.initial_truth(&mut status).unwrap();
    assert!(status.is_clear());
    assert_eq!(truth.total_mass().raw(), 537 * 4_096);
    assert_eq!(truth.active_propellant().raw(), 400 * 4_096);
    assert_eq!(truth.stage_phase(), StagePhase::Burning);
    assert_eq!(truth.active_stage(), 0);
}

#[test]
fn framing_checksum_and_reserved_fields_fail_closed() {
    assert_eq!(
        parse_phase2_scenario(&NOMINAL[..883]),
        Err(Phase2ScenarioError::Length)
    );
    let mut bad = *NOMINAL;
    bad[0] ^= 1;
    assert_eq!(parse_phase2_scenario(&bad), Err(Phase2ScenarioError::Magic));
    let mut bad = *NOMINAL;
    bad[52] = 1;
    let bad = repaired(bad);
    assert_eq!(
        parse_phase2_scenario(&bad),
        Err(Phase2ScenarioError::Reserved)
    );
}

#[test]
fn invalid_stage_and_pitch_records_fail_closed() {
    let mut bad = *NOMINAL;
    bad[64 + 29] = 0;
    let bad = repaired(bad);
    assert_eq!(parse_phase2_scenario(&bad), Err(Phase2ScenarioError::Stage));

    let mut bad = *NOMINAL;
    let pitch_base = 64 + 4 * 40;
    bad[pitch_base + 8..pitch_base + 12].copy_from_slice(&0i32.to_le_bytes());
    let bad = repaired(bad);
    assert_eq!(
        parse_phase2_scenario(&bad),
        Err(Phase2ScenarioError::PitchProgram)
    );
}
