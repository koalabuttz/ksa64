use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::{RUN_SUMMARY_LENGTH, STOCK_PLOT_STRIDE};
use ksa64_sim::phase4::mission::run_phase4_mission_observed;
use ksa64_sim::phase4::plot::{
    encoded_kph4_length, write_kph4, PlotIdentity, PlotRecorder, STOCK_PLOT_MAX_POINTS,
};
use ksa64_sim::phase4::summary::parse_ksr4;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../../phase2/examples/ksa2a-200km.ksc2");
const REFERENCE_KSR4: &[u8; 131_072] =
    include_bytes!("../../../phase4/examples/ksa4-reference.ksr4");

fn run() -> Result<(), String> {
    let output = PathBuf::from(env::args().nth(1).ok_or("missing output path")?);
    let scenario = parse_phase2_scenario(BASE).map_err(|error| format!("scenario: {error:?}"))?;
    let config = reviewed_campaign_config(1_024);
    let run = derive_run(&config, 0).map_err(|error| format!("run zero: {error:?}"))?;
    let baseline = parse_ksr4(&REFERENCE_KSR4[..RUN_SUMMARY_LENGTH])
        .map_err(|error| format!("baseline KSR4: {error:?}"))?;
    let mut recorder = PlotRecorder::<STOCK_PLOT_MAX_POINTS>::stock();
    let result = run_phase4_mission_observed(&scenario, run, &mut recorder)
        .map_err(|error| format!("baseline mission: {error:?}"))?;
    if result.truth_checksum != baseline.truth_checksum
        || result.sensor_checksum != baseline.sensor_checksum
        || result.nav_checksum != baseline.navigation_checksum
        || result.flight_checksum != baseline.flight_checksum
    {
        return Err("baseline checksums differ from frozen KSR4".to_owned());
    }
    let identity = PlotIdentity {
        campaign_crc32: baseline.campaign_crc32,
        run_index: baseline.run_index,
        sensor_seed: run.sensor_seed,
        variation_checksum: run.variation.checksum(),
        source_summary_crc32: crc32_ieee(&REFERENCE_KSR4[..RUN_SUMMARY_LENGTH]),
        stride: STOCK_PLOT_STRIDE as u16,
    };
    let mut bytes = vec![0u8; encoded_kph4_length(recorder.points().len()).unwrap()];
    write_kph4(identity, recorder.points(), &mut bytes)
        .map_err(|error| format!("KPH4: {error:?}"))?;
    fs::write(&output, &bytes).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!(
        "points={} bytes={} crc32=0x{:08x}",
        recorder.points().len(),
        bytes.len(),
        crc32_ieee(&bytes)
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
