use ksa64_sim::{phase5_campaign_probe_signature, run_phase5_campaign_self_tests};
#[test]
fn probe_is_frozen() {
    assert_eq!(phase5_campaign_probe_signature(), 0xc921_a2d2);
    assert_eq!(run_phase5_campaign_self_tests(), 0);
}
