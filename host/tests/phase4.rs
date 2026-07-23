use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_host::phase4::{encode_summary_stream, execute_host_campaign, HostCampaignError};
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::phase4::campaign::reviewed_campaign_config;
use ksa64_sim::phase4::config::{campaign_config_identity, write_campaign_config};
use ksa64_sim::phase4::contracts::{CAMPAIGN_CONFIG_LENGTH, RUN_SUMMARY_LENGTH};
use ksa64_sim::phase4::summary::parse_ksr4;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const PHASE3: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");

#[test]
fn serial_and_worker_counts_produce_identical_ordered_artifacts() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let config = reviewed_campaign_config(8);
    let mut ksc4 = [0u8; CAMPAIGN_CONFIG_LENGTH];
    write_campaign_config(BASE, PHASE3, &config, &mut ksc4).unwrap();
    let identity = campaign_config_identity(&ksc4).unwrap();
    let serial = execute_host_campaign(&scenario, &config, identity, 1).unwrap();
    let two = execute_host_campaign(&scenario, &config, identity, 2).unwrap();
    let four = execute_host_campaign(&scenario, &config, identity, 4).unwrap();
    assert_eq!(serial, two);
    assert_eq!(serial, four);
    let bytes = encode_summary_stream(&serial).unwrap();
    assert_eq!(bytes.len(), 8 * RUN_SUMMARY_LENGTH);
    for (index, record) in bytes.chunks_exact(RUN_SUMMARY_LENGTH).enumerate() {
        assert_eq!(parse_ksr4(record).unwrap().run_index, index as u32);
    }
    assert_eq!(
        execute_host_campaign(&scenario, &config, identity, 0),
        Err(HostCampaignError::WorkerCount)
    );
}
