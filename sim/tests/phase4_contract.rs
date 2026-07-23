use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_interface::crc32_ieee;
use ksa64_sim::mission::{run_phase3_mission, MissionCase, MissionOutcome};
use ksa64_sim::phase4::contracts::*;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const NOMINAL_KST3: &[u8] = include_bytes!("../../phase3/examples/ksa3-nominal.kst3");

#[test]
fn phase4_contract_sizes_and_reference_identity_are_frozen() {
    assert_eq!(CAMPAIGN_CONFIG_LENGTH, 512);
    assert_eq!(MAX_DISTRIBUTIONS, 16);
    assert_eq!(RUN_SUMMARY_LENGTH, 128);
    assert_eq!(PLOT_POINT_LENGTH, 8);
    assert_eq!(DETAIL_FRAME_LENGTH, 160);
    assert_eq!(SMOKE_RUNS, 64);
    assert_eq!(REFERENCE_RUNS, 1_024);
    assert_eq!(REFERENCE_MASTER_SEED, 0x4b53_4134);
    assert_eq!(crc32_ieee(NOMINAL_KST3), 0xaf79_b36e);
}

#[test]
fn additive_phase4_boundary_preserves_phase3_nominal_exactly() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let result = run_phase3_mission(&scenario, MissionCase::Nominal).unwrap();
    assert_eq!(result.outcome, MissionOutcome::DurationComplete);
    assert_eq!(result.truth.step(), 7_200);
    assert_eq!(result.truth_checksum, 0xc860_45a0);
    assert_eq!(result.sensor_checksum, 0x47d1_1fb0);
    assert_eq!(result.nav_checksum, 0xc6f9_da7b);
    assert_eq!(result.flight_checksum, 0x02ce_28ef);
}
