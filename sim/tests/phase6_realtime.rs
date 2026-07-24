use ksa64_sim::phase5_vehicle::Phase5StagePhase;
use ksa64_sim::phase6_realtime::{run_realtime_nominal, RealtimeRunEvidence};

const ACCEPTED: RealtimeRunEvidence = RealtimeRunEvidence {
    fast_epochs: 12_692,
    mission_steps: 3_173,
    terminal_phase: Phase5StagePhase::Complete,
    terminal_position_q12: [21_360_371, 4_030_786, 15_731_027],
    terminal_velocity_q24: [-69_442_203, 96_406_364, 65_655_653],
    navigation_position_q12: [21_360_000, 4_031_445, 15_731_484],
    navigation_velocity_q24: [-68_076_267, 95_786_604, 65_320_561],
    navigation_checksum: 2_195_755_368,
    flight_checksum: 2_901_449_607,
    status_checksum: 3_868_727_872,
    safe: false,
};

#[test]
fn realtime_nominal_matches_frozen_evidence() {
    assert_eq!(run_realtime_nominal().unwrap(), ACCEPTED);
}

#[test]
fn realtime_nominal_repeats_exactly() {
    assert_eq!(
        run_realtime_nominal().unwrap(),
        run_realtime_nominal().unwrap()
    );
}
