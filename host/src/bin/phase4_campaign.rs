use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_host::phase4::{encode_summary_stream, execute_host_campaign};
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::phase4::campaign::reviewed_campaign_config;
use ksa64_sim::phase4::config::{campaign_config_identity, write_campaign_config};
use ksa64_sim::phase4::contracts::CAMPAIGN_CONFIG_LENGTH;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../../phase2/examples/ksa2a-200km.ksc2");
const PHASE3: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../../phase3/examples/ksa3-nominal.ksc3");

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let run_count = arguments
        .next()
        .ok_or("missing run count")?
        .parse::<u32>()
        .map_err(|error| format!("invalid run count: {error}"))?;
    let workers = arguments
        .next()
        .ok_or("missing worker count")?
        .parse::<usize>()
        .map_err(|error| format!("invalid worker count: {error}"))?;
    let output = PathBuf::from(arguments.next().ok_or("missing output directory")?);
    if arguments.next().is_some() {
        return Err("usage: phase4_campaign <runs> <workers> <output-directory>".to_owned());
    }
    let scenario = parse_phase2_scenario(BASE).map_err(|error| format!("scenario: {error:?}"))?;
    let config = reviewed_campaign_config(run_count);
    let mut ksc4 = [0u8; CAMPAIGN_CONFIG_LENGTH];
    write_campaign_config(BASE, PHASE3, &config, &mut ksc4)
        .map_err(|error| format!("KSC4: {error:?}"))?;
    let campaign_crc =
        campaign_config_identity(&ksc4).map_err(|error| format!("campaign identity: {error:?}"))?;
    let started = Instant::now();
    let campaign = execute_host_campaign(&scenario, &config, campaign_crc, workers)
        .map_err(|error| format!("campaign: {error:?}"))?;
    let elapsed = started.elapsed();
    let ksr4 = encode_summary_stream(&campaign).map_err(|error| format!("KSR4: {error:?}"))?;
    fs::create_dir_all(&output).map_err(|error| format!("create {}: {error}", output.display()))?;
    let stem = if run_count == 1_024 {
        "ksa4-reference"
    } else if run_count == 64 {
        "ksa4-smoke"
    } else {
        "ksa4-campaign"
    };
    fs::write(output.join(format!("{stem}.ksc4")), ksc4)
        .map_err(|error| format!("write KSC4: {error}"))?;
    fs::write(output.join(format!("{stem}.ksr4")), ksr4)
        .map_err(|error| format!("write KSR4: {error}"))?;
    println!(
        "runs={} workers={} elapsed_ms={} campaign=0x{campaign_crc:08x} summary_chain=0x{:08x} outcomes={:?}",
        run_count,
        workers,
        elapsed.as_millis(),
        campaign.aggregate.summary_chain,
        campaign.aggregate.outcome_counts
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
