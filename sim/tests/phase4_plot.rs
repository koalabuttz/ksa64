use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::{RUN_SUMMARY_LENGTH, STOCK_PLOT_STRIDE};
use ksa64_sim::phase4::mission::run_phase4_mission_observed;
use ksa64_sim::phase4::plot::{
    encoded_kph4_length, parse_kph4, write_kph4, PlotError, PlotIdentity, PlotRecorder,
    STOCK_PLOT_MAX_POINTS,
};
use ksa64_sim::phase4::summary::parse_ksr4;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const REFERENCE_KSR4: &[u8; 131_072] = include_bytes!("../../phase4/examples/ksa4-reference.ksr4");
const BASELINE_KPH4: &[u8; 1_872] = include_bytes!("../../phase4/examples/ksa4-baseline.kph4");

#[test]
fn frozen_stock_plot_is_exact_sparse_and_strict() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let run = derive_run(&reviewed_campaign_config(1_024), 0).unwrap();
    let baseline = parse_ksr4(&REFERENCE_KSR4[..RUN_SUMMARY_LENGTH]).unwrap();
    let mut recorder = PlotRecorder::<STOCK_PLOT_MAX_POINTS>::stock();
    let result = run_phase4_mission_observed(&scenario, run, &mut recorder).unwrap();
    assert_eq!(recorder.points().len(), 226);
    assert_eq!(recorder.points().first().unwrap().step, 0);
    assert_eq!(recorder.points().last().unwrap().step, 7_200);
    assert_eq!(result.truth_checksum, baseline.truth_checksum);
    let identity = PlotIdentity {
        campaign_crc32: baseline.campaign_crc32,
        run_index: 0,
        sensor_seed: run.sensor_seed,
        variation_checksum: run.variation.checksum(),
        source_summary_crc32: crc32_ieee(&REFERENCE_KSR4[..RUN_SUMMARY_LENGTH]),
        stride: STOCK_PLOT_STRIDE as u16,
    };
    let mut encoded = [0u8; 1_872];
    assert_eq!(
        encoded_kph4_length(recorder.points().len()),
        Some(encoded.len())
    );
    write_kph4(identity, recorder.points(), &mut encoded).unwrap();
    assert_eq!(&encoded, BASELINE_KPH4);
    let archive = parse_kph4(&encoded).unwrap();
    assert_eq!(archive.identity, identity);
    assert_eq!(archive.point_count, 226);
    assert_eq!(archive.point(0), recorder.points().first().copied());
    assert_eq!(archive.point(225), recorder.points().last().copied());

    for offset in [0, 12, 32, 40, 60, 64, 1_871] {
        let mut corrupt = encoded;
        corrupt[offset] ^= 0x80;
        assert!(parse_kph4(&corrupt).is_err(), "offset {offset}");
    }
    assert_eq!(parse_kph4(&encoded[..1_871]).err(), Some(PlotError::Length));
}
