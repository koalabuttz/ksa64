//! Target-executable checks for Phase 2 packed scenario ingestion.

use crate::phase2_scenario::{
    parse_phase2_scenario, KSA2A_EARLY_CUTOFF_SCENARIO_ID, KSA2A_NOMINAL_SCENARIO_ID,
    PHASE2_SCENARIO_IMAGE_LENGTH,
};

const NOMINAL: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const FAILURE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-early-cutoff.ksc2");

pub fn run_phase2_scenario_self_tests() -> u8 {
    let nominal = match parse_phase2_scenario(NOMINAL) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    let failure = match parse_phase2_scenario(FAILURE) {
        Ok(value) => value,
        Err(_) => return 2,
    };
    if nominal.scenario_id() != KSA2A_NOMINAL_SCENARIO_ID
        || failure.scenario_id() != KSA2A_EARLY_CUTOFF_SCENARIO_ID
        || nominal.stage_count() != 2
        || nominal.stage(1).map(|stage| stage.burn_steps()) != Some(1_920)
        || failure.stage(1).map(|stage| stage.burn_steps()) != Some(1_824)
        || !nominal.pitch_program().is_valid(nominal.timestep())
    {
        3
    } else {
        0
    }
}
