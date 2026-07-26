use ksa64_core::phase10_telemetry::KSC10_LENGTH;
use ksa64_host::phase10::{
    encode_kra10, run_global_campaign, validate_kra10, GlobalFixtureSet, PHASE10_MASTER_SEED,
};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn run() -> Result<(), String> {
    let mut workers = 1usize;
    let mut runs = 64u16;
    let mut output = PathBuf::from("phase10/campaign");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--workers" => {
                workers = args
                    .next()
                    .ok_or("missing worker count")?
                    .parse()
                    .map_err(|_| "bad worker count")?;
            }
            "--runs" => {
                runs = args
                    .next()
                    .ok_or("missing run count")?
                    .parse()
                    .map_err(|_| "bad run count")?;
            }
            "--output" => output = args.next().ok_or("missing output directory")?.into(),
            _ => return Err(format!("unknown option {flag}")),
        }
    }

    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let fixtures = GlobalFixtureSet::embedded();
    let started = std::time::Instant::now();
    let result =
        run_global_campaign(&fixtures, runs, workers).map_err(|error| format!("{error:?}"))?;
    let archive = encode_kra10(&result).map_err(|error| format!("{error:?}"))?;
    let verified = validate_kra10(&archive).map_err(|error| format!("{error:?}"))?;
    if verified != result.aggregate {
        return Err("archive aggregate changed during validation".to_owned());
    }

    let stem = format!("ksa-g10r-{runs}");
    let archive_path = output.join(format!("{stem}.kra10"));
    let config_path = output.join(format!("{stem}.ksc10"));
    let evidence_path = output.join(format!("{stem}.json"));
    let mut config = [0; KSC10_LENGTH];
    result
        .config
        .encode(&mut config)
        .map_err(|error| format!("{error:?}"))?;
    fs::write(&archive_path, &archive).map_err(|error| error.to_string())?;
    fs::write(&config_path, config).map_err(|error| error.to_string())?;

    let aggregate = result.aggregate;
    let elapsed = started.elapsed().as_secs_f64();
    let report = serde_json::json!({
        "schema": "ksa64.phase10.campaign-evidence-v1",
        "master_seed": format!("0x{PHASE10_MASTER_SEED:08x}"),
        "runs": runs,
        "workers": workers,
        "archive_bytes": archive.len(),
        "summaries_crc32": format!("0x{:08x}", aggregate.summaries_crc32),
        "ground_contacts": aggregate.ground_contacts,
        "physical_recoveries": aggregate.physical_recoveries,
        "numeric_frame_time_faults": aggregate.numeric_frame_time_faults,
        "model_envelope_exceeded": aggregate.model_envelope_exceeded,
        "minimum_apogee_q12_km": aggregate.minimum_apogee_q12_km,
        "maximum_apogee_q12_km": aggregate.maximum_apogee_q12_km,
        "maximum_downrange_q12_km": aggregate.maximum_downrange_q12_km,
        "maximum_navigation_error_q12_km": aggregate.maximum_navigation_error_q12_km,
        "wall_seconds": elapsed,
    });
    fs::write(
        &evidence_path,
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
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
