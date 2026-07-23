use ksa64_host::phase5_campaign::{encode_ksr5_stream, execute_phase5_campaign};
use ksa64_sim::phase5_campaign::reviewed_phase5_campaign_config;
#[test]
fn worker_counts_preserve_ordered_artifacts() {
    let c = reviewed_phase5_campaign_config(8);
    let serial = execute_phase5_campaign(&c, 1).unwrap();
    let parallel = execute_phase5_campaign(&c, 3).unwrap();
    assert_eq!(serial, parallel);
    assert_eq!(
        encode_ksr5_stream(&serial).unwrap(),
        encode_ksr5_stream(&parallel).unwrap()
    );
}
