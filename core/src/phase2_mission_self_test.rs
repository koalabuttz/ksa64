//! Target-executable checks for the generated Phase 2 mission fixtures.

use crate::phase2_mission::{execute_phase2_mission, Phase2MissionError, Phase2MissionOutcome};
use crate::phase2_scenario::{ksa2a_fixture, ksa2a_smoke_fixture};
use crate::planar::{OrbitClass, StagePhase};

pub fn run_phase2_mission_smoke_self_tests() -> u8 {
    let result = match execute_phase2_mission(ksa2a_smoke_fixture()) {
        Ok(value) => value,
        Err(Phase2MissionError::InitialState) => return 11,
        Err(Phase2MissionError::Configuration) => return 12,
        Err(Phase2MissionError::NumericFault) => return 13,
    };
    if result.outcome() != Phase2MissionOutcome::DurationComplete
        || result.truth().step() != 1
        || result.truth().stage_phase() != StagePhase::Burning
        || result.truth().active_stage() != 0
        || result.cutoff_step() != 0
        || result.cutoff_orbit().is_some()
    {
        2
    } else {
        0
    }
}

pub fn run_phase2_nominal_mission_self_tests() -> u8 {
    let result = match execute_phase2_mission(ksa2a_fixture(false)) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    let orbit = match result.cutoff_orbit() {
        Some(value) => value,
        None => return 2,
    };
    if result.outcome() != Phase2MissionOutcome::DurationComplete
        || result.truth().step() != 7_200
        || result.cutoff_step() != 3_172
        || result.truth().radius().raw() != 26_934_487
        || result.truth().radial_velocity().raw() != -139_912
        || result.truth().specific_angular_momentum().raw() != 838_201_963
        || result.max_dynamic_pressure().raw() != 2_672_500
        || result.max_proper_acceleration().raw() != 14_839_808
        || orbit.class() != OrbitClass::StableOrbit
        || orbit.perigee().raw() != 26_895_588
        || orbit.apogee().raw() != 26_895_588
    {
        3
    } else {
        0
    }
}

pub fn run_phase2_failure_mission_self_tests() -> u8 {
    let result = match execute_phase2_mission(ksa2a_fixture(true)) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    let orbit = match result.cutoff_orbit() {
        Some(value) => value,
        None => return 2,
    };
    if result.outcome() != Phase2MissionOutcome::DurationComplete
        || result.truth().step() != 3_076
        || result.cutoff_step() != 3_076
        || result.truth().radius().raw() != 26_943_877
        || result.truth().radial_velocity().raw() != 113_139
        || result.truth().specific_angular_momentum().raw() != 781_641_910
        || orbit.class() == OrbitClass::StableOrbit
    {
        3
    } else {
        0
    }
}
