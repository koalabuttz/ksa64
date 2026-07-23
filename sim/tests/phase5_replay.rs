use ksa64_sim::phase5_history::{validate_kph5, KPH5_HEADER_LENGTH, KPH5_POINT_LENGTH};
use ksa64_sim::phase5_replay::{phase5_plot_coordinate, replay_phase5_history, Phase5ReplayError};
const TAPE: &[u8; 1664] = include_bytes!("../../phase5/examples/ksa5-baseline.kph5");
#[test]
fn frozen_phase5_replay_is_strict_and_bounded() {
    let summary = replay_phase5_history(TAPE, 0x4b534135, 0).unwrap();

    assert_eq!(summary.points, 99);
    assert_eq!(summary.final_step, 3133);
    assert_eq!(summary.observed_events, 7);
    assert_eq!(summary.observed_alarms, 0);
    assert_eq!(summary.cue_counts, [2, 2, 1, 0, 0]);
    assert_eq!(summary.cue_hash, 0x3b2f_b64b);
    let h = validate_kph5(TAPE).unwrap();
    let at = KPH5_HEADER_LENGTH + (h.point_count as usize - 1) * KPH5_POINT_LENGTH;
    let p = ksa64_sim::phase5_history::parse_kph5_point(&TAPE[at..at + KPH5_POINT_LENGTH]).unwrap();
    assert_eq!(phase5_plot_coordinate(p), (33, 7));
}
#[test]
fn replay_rejects_corruption_and_identity() {
    assert!(matches!(
        replay_phase5_history(TAPE, 1, 0),
        Err(Phase5ReplayError::Identity)
    ));
    let mut bad = *TAPE;
    bad[KPH5_HEADER_LENGTH + 4] ^= 1;
    assert!(matches!(
        replay_phase5_history(&bad, 0x4b534135, 0),
        Err(Phase5ReplayError::History(_))
    ));
}
