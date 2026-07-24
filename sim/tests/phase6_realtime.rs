use ksa64_sim::phase5_vehicle::Phase5StagePhase;
use ksa64_sim::phase6_realtime::{run_realtime_nominal, RealtimeRunEvidence};

const ACCEPTED: RealtimeRunEvidence = RealtimeRunEvidence {
    fast_epochs: 12_692,
    mission_steps: 3_173,
    terminal_phase: Phase5StagePhase::Complete,
    terminal_position_q12: [21_357_272, 4_045_155, 15_714_392],
    terminal_velocity_q24: [-69_495_488, 96_886_117, 65_043_473],
    navigation_position_q12: [21_356_900, 4_045_817, 15_714_845],
    navigation_velocity_q24: [-68_129_578, 96_267_553, 64_703_018],
    navigation_checksum: 2_707_470_065,
    flight_checksum: 3_942_459_298,
    status_checksum: 1_258_209_725,
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
