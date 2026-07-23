use ksa64_core::phase2_scenario::PHASE2_SCENARIO_IMAGE_LENGTH;
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::phase4::campaign::distribution_fixture_config;
use ksa64_sim::phase4::config::{
    parse_campaign_config, write_campaign_config, CampaignConfigError,
};
use ksa64_sim::phase4::contracts::CAMPAIGN_CONFIG_LENGTH;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const PHASE3: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");

#[test]
fn ksc4_round_trips_and_binds_exact_inputs() {
    let expected = distribution_fixture_config();
    let mut bytes = [0u8; CAMPAIGN_CONFIG_LENGTH];
    write_campaign_config(BASE, PHASE3, &expected, &mut bytes).unwrap();
    assert_eq!(
        parse_campaign_config(&bytes, BASE, PHASE3).unwrap(),
        expected
    );
}

#[test]
fn ksc4_corruption_reserved_and_identity_fail_closed() {
    let config = distribution_fixture_config();
    let mut bytes = [0u8; CAMPAIGN_CONFIG_LENGTH];
    write_campaign_config(BASE, PHASE3, &config, &mut bytes).unwrap();
    for offset in [0, 8, 16, 20, 24, 32, 40, 128, 148, 511] {
        let mut corrupt = bytes;
        corrupt[offset] ^= 0x55;
        assert!(
            parse_campaign_config(&corrupt, BASE, PHASE3).is_err(),
            "offset {offset}"
        );
    }
    assert_eq!(
        parse_campaign_config(&bytes[..511], BASE, PHASE3),
        Err(CampaignConfigError::Length)
    );
}
