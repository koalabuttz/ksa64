use ksa64_core::phase2_scenario::PHASE2_SCENARIO_IMAGE_LENGTH;
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::phase4::aggregate::CampaignAggregate;
use ksa64_sim::phase4::campaign::reviewed_campaign_config;
use ksa64_sim::phase4::config::{
    campaign_config_identity, parse_campaign_config, CampaignConfigError,
};
use ksa64_sim::phase4::contracts::{CAMPAIGN_CONFIG_LENGTH, RUN_SUMMARY_LENGTH};
use ksa64_sim::phase4::summary::{parse_ksr4, SummaryError};

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const PHASE3: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");
const KSC4: &[u8; CAMPAIGN_CONFIG_LENGTH] =
    include_bytes!("../../phase4/examples/ksa4-reference.ksc4");
const KSR4: &[u8; 1_024 * RUN_SUMMARY_LENGTH] =
    include_bytes!("../../phase4/examples/ksa4-reference.ksr4");

#[test]
fn frozen_reference_records_are_strict_and_ordered() {
    assert_eq!(
        parse_campaign_config(KSC4, BASE, PHASE3).unwrap(),
        reviewed_campaign_config(1_024)
    );
    let identity = campaign_config_identity(KSC4).unwrap();
    assert_eq!(identity, 0xa2e9_e9d5);
    let mut aggregate = CampaignAggregate::new();
    for (index, record) in KSR4.chunks_exact(RUN_SUMMARY_LENGTH).enumerate() {
        let summary = parse_ksr4(record).unwrap();
        assert_eq!(summary.run_index, index as u32);
        assert_eq!(summary.campaign_crc32, identity);
        aggregate.update(&summary);
        if index == 0 {
            assert_eq!(summary.truth_checksum, 0xc860_45a0);
            assert_eq!(summary.sensor_checksum, 0x47d1_1fb0);
            assert_eq!(summary.navigation_checksum, 0xc6f9_da7b);
            assert_eq!(summary.flight_checksum, 0x02ce_28ef);
        }
    }
    assert_eq!(aggregate.run_count, 1_024);
    assert_eq!(aggregate.outcome_counts, [857, 166, 1, 0, 0, 0]);
    assert_eq!(aggregate.summary_chain, 0x813c_e420);
}

#[test]
fn frozen_reference_rejects_first_corrupt_record_and_config() {
    let mut record = [0u8; RUN_SUMMARY_LENGTH];
    record.copy_from_slice(&KSR4[17 * RUN_SUMMARY_LENGTH..18 * RUN_SUMMARY_LENGTH]);
    record[64] ^= 1;
    assert_eq!(parse_ksr4(&record), Err(SummaryError::Checksum));
    let mut config = *KSC4;
    config[200] ^= 1;
    assert!(matches!(
        parse_campaign_config(&config, BASE, PHASE3),
        Err(CampaignConfigError::Record | CampaignConfigError::Checksum)
    ));
}
