use ksa64_sim::replay::*;
const NOMINAL: &[u8] = include_bytes!("../../phase3/examples/ksa3-nominal.krp3");
const SCENARIO: u32 = 0x95bc_9413;
const CONFIG: u32 = 0x2815_ea66;

#[test]
fn frozen_nominal_replay_is_strict_and_deterministic() {
    let a = replay_phase3_tape(NOMINAL, SCENARIO, CONFIG).unwrap();
    let b = replay_phase3_tape(NOMINAL, SCENARIO, CONFIG).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.frames, 906);
    assert_eq!(a.final_step, 7200);
    assert_eq!(a.final_mode, 5);
    assert_eq!(a.cue_counts[3], 1);
}

#[test]
fn corruption_identity_and_truncation_fail_closed() {
    let mut corrupt = NOMINAL.to_vec();
    corrupt[100] ^= 1;
    assert!(matches!(
        replay_phase3_tape(&corrupt, SCENARIO, CONFIG),
        Err(ReplayError::Frame { .. })
    ));
    assert_eq!(
        replay_phase3_tape(NOMINAL, SCENARIO ^ 1, CONFIG),
        Err(ReplayError::Identity)
    );
    assert_eq!(
        replay_phase3_tape(&NOMINAL[..NOMINAL.len() - 1], SCENARIO, CONFIG),
        Err(ReplayError::Length)
    );
}
