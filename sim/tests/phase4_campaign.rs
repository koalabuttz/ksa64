use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::mission::{run_phase3_mission, MissionCase};
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config, ParameterId};
use ksa64_sim::phase4::config::{campaign_config_identity, write_campaign_config};
use ksa64_sim::phase4::contracts::{CAMPAIGN_CONFIG_LENGTH, RUN_SUMMARY_LENGTH, SMOKE_RUNS};
use ksa64_sim::phase4::mission::run_phase4_mission;
use ksa64_sim::phase4::runner::{run_campaign, CampaignSink};
use ksa64_sim::phase4::summary::{parse_ksr4, write_ksr4, RunSummary, SummaryError};

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const PHASE3: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");

#[test]
fn reviewed_catalog_and_run_zero_are_frozen() {
    let config = reviewed_campaign_config(1_024);
    config.validate().unwrap();
    assert_eq!(config.distribution_count, 15);
    let zero = derive_run(&config, 0).unwrap();
    assert_eq!(zero.sensor_seed, 0x4b53_4133);
    assert_eq!(zero.variation.values(), [0; 15]);
    assert!((-2..=2).contains(
        &derive_run(&config, 1)
            .unwrap()
            .variation
            .value(ParameterId::ActuatorLagSteps)
    ));
}

#[test]
fn parameterized_run_zero_is_exact_phase3_nominal() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let run = derive_run(&reviewed_campaign_config(1), 0).unwrap();
    let phase3 = run_phase3_mission(&scenario, MissionCase::Nominal).unwrap();
    let phase4 = run_phase4_mission(&scenario, run).unwrap();
    assert_eq!(phase4.truth_checksum, phase3.truth_checksum);
    assert_eq!(phase4.sensor_checksum, phase3.sensor_checksum);
    assert_eq!(phase4.nav_checksum, phase3.nav_checksum);
    assert_eq!(phase4.flight_checksum, phase3.flight_checksum);
    assert_eq!(phase4.truth, phase3.truth);
}

#[test]
fn ksr4_round_trips_and_rejects_corruption() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let run = derive_run(&reviewed_campaign_config(1), 0).unwrap();
    let result = run_phase4_mission(&scenario, run).unwrap();
    let summary = RunSummary::from_result(&scenario, 0x1234_5678, run, result);
    let mut bytes = [0u8; RUN_SUMMARY_LENGTH];
    write_ksr4(&summary, &mut bytes).unwrap();
    assert_eq!(parse_ksr4(&bytes).unwrap(), summary);
    for offset in [0, 8, 20, 32, 64, 112, 124] {
        let mut corrupt = bytes;
        corrupt[offset] ^= 0x80;
        assert!(parse_ksr4(&corrupt).is_err(), "offset {offset}");
    }
    assert_eq!(
        parse_ksr4(&bytes[..RUN_SUMMARY_LENGTH - 1]),
        Err(SummaryError::Length)
    );
}

struct SmokeSink {
    next: u32,
    baseline: Option<RunSummary>,
}
impl CampaignSink for SmokeSink {
    type Error = ();
    fn observe(&mut self, summary: &RunSummary) -> Result<(), Self::Error> {
        assert_eq!(summary.run_index, self.next);
        let mut bytes = [0u8; RUN_SUMMARY_LENGTH];
        write_ksr4(summary, &mut bytes).unwrap();
        assert_eq!(parse_ksr4(&bytes).unwrap(), *summary);
        if summary.run_index == 0 {
            self.baseline = Some(*summary);
        }
        self.next += 1;
        Ok(())
    }
}

#[test]
fn smoke_campaign_64_is_ordered_and_repeatable() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let config = reviewed_campaign_config(SMOKE_RUNS);
    let mut ksc4 = [0u8; CAMPAIGN_CONFIG_LENGTH];
    write_campaign_config(BASE, PHASE3, &config, &mut ksc4).unwrap();
    let campaign_crc = campaign_config_identity(&ksc4).unwrap();
    let mut first_sink = SmokeSink {
        next: 0,
        baseline: None,
    };
    let first = run_campaign(&scenario, &config, campaign_crc, &mut first_sink).unwrap();
    let mut second_sink = SmokeSink {
        next: 0,
        baseline: None,
    };
    let second = run_campaign(&scenario, &config, campaign_crc, &mut second_sink).unwrap();
    assert_eq!(first, second);
    assert_eq!(campaign_crc, 0x3ad7_ff88);
    assert_eq!(first.run_count, SMOKE_RUNS);
    assert_eq!(first.outcome_counts, [55, 9, 0, 0, 0, 0]);
    assert_eq!(first.outcome_counts.iter().sum::<u32>(), SMOKE_RUNS);
    assert_eq!(first.cutoff_altitude_km.minimum, 180);
    assert_eq!(first.cutoff_altitude_km.maximum, 192);
    assert_eq!(first.cutoff_altitude_km.mean_q16(), 12_321_787);
    assert_eq!(first.cutoff_altitude_km.sample_variance(), 6);
    assert_eq!(first.max_dynamic_pressure_kpa.minimum, 40);
    assert_eq!(first.max_dynamic_pressure_kpa.maximum, 43);
    assert_eq!(first.max_proper_acceleration_mps2.minimum, 54);
    assert_eq!(first.max_proper_acceleration_mps2.maximum, 55);
    assert_eq!(first.navigation_position_error_m.minimum, 1);
    assert_eq!(first.navigation_position_error_m.maximum, 47);
    assert_eq!(first.summary_chain, 586_068_286);
    assert_eq!(
        first.insertion_histogram,
        [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let baseline = first_sink.baseline.unwrap();
    assert_eq!(baseline.truth_checksum, 0xc860_45a0);
    assert_eq!(baseline.sensor_checksum, 0x47d1_1fb0);
    assert_eq!(baseline.navigation_checksum, 0xc6f9_da7b);
    assert_eq!(baseline.flight_checksum, 0x02ce_28ef);
    println!("campaign_crc={campaign_crc:08x}");
    println!("aggregate={first:?}");
}
