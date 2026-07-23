use ksa64_core::phase5_contract::PHASE5_CAMPAIGN_SEED;
use ksa64_sim::phase5_campaign::{derive_phase5_run, reference_phase5_campaign_config};
use ksa64_sim::phase5_history::{
    write_kph5, Phase5HistoryHeader, Phase5HistoryRecorder, KPH5_HEADER_LENGTH, KPH5_POINT_LENGTH,
    STOCK_HISTORY_POINTS, STOCK_HISTORY_STRIDE,
};
use ksa64_sim::phase5_mission::{
    run_phase5_mission_observed, Phase5MissionCase, Phase5MissionSummary,
};

pub fn capture_phase5_stock_history() -> (Phase5MissionSummary, Vec<u8>) {
    let config = reference_phase5_campaign_config();
    let run = derive_phase5_run(&config, 0).expect("frozen run zero");
    let mut recorder = Phase5HistoryRecorder::<STOCK_HISTORY_POINTS>::new(STOCK_HISTORY_STRIDE);
    let summary = run_phase5_mission_observed(Phase5MissionCase::Nominal, &mut recorder)
        .expect("infallible stock history capacity");
    let mut bytes = vec![0u8; KPH5_HEADER_LENGTH + recorder.count() * KPH5_POINT_LENGTH];
    write_kph5(
        Phase5HistoryHeader {
            campaign_seed: PHASE5_CAMPAIGN_SEED,
            run_index: 0,
            sensor_seed: run.sensor_seed,
            variation_checksum: run.variation.checksum(),
            stride: STOCK_HISTORY_STRIDE,
            point_count: 0,
            terminal_step: summary.steps,
            points_crc32: 0,
        },
        recorder.points(),
        &mut bytes,
    )
    .expect("valid KPH5 history");
    (summary, bytes)
}
