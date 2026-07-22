use ksa64_phase0_rust::{run_vertical_manual, vertical_vectors};

#[test]
fn vertical_workload_matches_every_checkpoint_and_checksum() {
    let run = run_vertical_manual();
    assert_eq!(run.checkpoint_failures, 0);
    assert_eq!(run.checksum, vertical_vectors::VERTICAL_FINAL_FNV1A32);

    let expected = vertical_vectors::VERTICAL_CHECKPOINTS.last().unwrap();
    assert_eq!(run.state.time_q12, expected.time_q12);
    assert_eq!(run.state.altitude_q12, expected.altitude_q12);
    assert_eq!(run.state.velocity_q24, expected.velocity_q24);
    assert_eq!(run.state.acceleration_q28, expected.acceleration_q28);
    assert_eq!(run.state.mass_q12, expected.mass_q12);
    assert_eq!(run.state.propellant_q12, expected.propellant_q12);
    assert_eq!(run.state.cutoff_events, expected.cutoff_events);
}
