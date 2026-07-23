use ksa64_host::phase5_campaign::{encode_ksr5_stream, execute_phase5_campaign};
use ksa64_sim::phase5_campaign::{reviewed_phase5_campaign_config, write_ksc5, KSC5_LENGTH};
use std::{env, fs, path::PathBuf, process::ExitCode, time::Instant};
fn run() -> Result<(), String> {
    let mut a = env::args().skip(1);
    let runs = a
        .next()
        .ok_or("missing runs")?
        .parse::<u32>()
        .map_err(|e| format!("runs: {e}"))?;
    let workers = a
        .next()
        .ok_or("missing workers")?
        .parse::<usize>()
        .map_err(|e| format!("workers: {e}"))?;
    let out = PathBuf::from(a.next().ok_or("missing output directory")?);
    if a.next().is_some() {
        return Err("usage: phase5_campaign <runs> <workers> <output-directory>".into());
    }
    let config = reviewed_phase5_campaign_config(runs);
    let mut ksc = [0u8; KSC5_LENGTH];
    write_ksc5(&config, &mut ksc).map_err(|e| format!("KSC5: {e:?}"))?;
    let start = Instant::now();
    let campaign =
        execute_phase5_campaign(&config, workers).map_err(|e| format!("campaign: {e:?}"))?;
    let ksr = encode_ksr5_stream(&campaign).map_err(|e| format!("KSR5: {e:?}"))?;
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let stem = if runs == 256 {
        "ksa5-reference"
    } else if runs == 32 {
        "ksa5-routine"
    } else {
        "ksa5-campaign"
    };
    fs::write(out.join(format!("{stem}.ksc5")), ksc).map_err(|e| e.to_string())?;
    fs::write(out.join(format!("{stem}.ksr5")), ksr).map_err(|e| e.to_string())?;
    println!(
        "runs={} workers={} elapsed_ms={} chain=0x{:08x} outcomes={:?}",
        runs,
        workers,
        start.elapsed().as_millis(),
        campaign.aggregate.summary_chain,
        campaign.aggregate.outcome_counts
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
