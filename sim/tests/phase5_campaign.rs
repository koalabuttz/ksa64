use ksa64_core::phase5_contract::PHASE5_CAMPAIGN_SEED;
use ksa64_sim::phase5_campaign::{
    derive_phase5_run, parse_ksc5, parse_ksr5, reviewed_phase5_campaign_config,
    run_phase5_campaign_mission, write_ksc5, write_ksr5, KSC5_LENGTH, KSR5_LENGTH,
};
use ksa64_sim::phase5_mission::{run_phase5_mission, Phase5MissionCase};
#[test]
fn run_zero_is_exact_gate8_nominal() {
    let c = reviewed_phase5_campaign_config(4);
    assert_eq!(c.master_seed, PHASE5_CAMPAIGN_SEED);
    let mut config_bytes = [0u8; KSC5_LENGTH];
    write_ksc5(&c, &mut config_bytes).unwrap();
    assert_eq!(parse_ksc5(&config_bytes), Ok(c));
    let r = derive_phase5_run(&c, 0).unwrap();
    assert_eq!(r.sensor_seed, 0x5a00_0000);
    assert!(r.variation.values().iter().all(|&v| v == 0));
    let got = run_phase5_campaign_mission(&c, 0).unwrap();
    assert_eq!(
        got.mission,
        run_phase5_mission(Phase5MissionCase::Nominal).unwrap()
    );
    let mut b = [0u8; KSR5_LENGTH];
    write_ksr5(&got, &mut b).unwrap();
    assert_eq!(parse_ksr5(&b), Ok(got));
}
#[test]
fn keyed_sample_is_repeatable() {
    let c = reviewed_phase5_campaign_config(4);
    let a = run_phase5_campaign_mission(&c, 1).unwrap();
    let b = run_phase5_campaign_mission(&c, 1).unwrap();
    assert_eq!(a, b);
    assert_ne!(a.variation_checksum, 0);
}
