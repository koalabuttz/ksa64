use ksa64_sim::phase5_campaign::{
    parse_ksc5, parse_ksr5, Phase5CampaignAggregate, KSC5_LENGTH, KSR5_LENGTH,
};
const KSC: &[u8; KSC5_LENGTH] = include_bytes!("../../phase5/examples/ksa5-reference.ksc5");
const KSR: &[u8; 256 * KSR5_LENGTH] = include_bytes!("../../phase5/examples/ksa5-reference.ksr5");
#[test]
fn frozen_reference_campaign_is_strict_and_ordered() {
    let c = parse_ksc5(KSC).unwrap();
    assert_eq!(c.run_count, 256);
    assert_eq!(c.master_seed, 0x4b53_4135);
    let mut a = Phase5CampaignAggregate::new();
    for (i, b) in KSR.chunks_exact(KSR5_LENGTH).enumerate() {
        let s = parse_ksr5(b).unwrap();
        assert_eq!(s.run_index, i as u32);
        a.update(&s)
    }
    assert_eq!(a.outcome_counts, [180, 28, 48, 0, 0]);
    assert_eq!(a.summary_chain, 0x3103_d833);
}
