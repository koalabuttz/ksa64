use ksa64_host::phase8_5_campaign::{
    encode_phase85_campaign, run_phase85_campaign, PHASE85_CAMPAIGN_RUNS, PHASE85_CAMPAIGN_SEED,
};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
fn run() -> Result<(), String> {
    let mut workers = 1usize;
    let mut output = PathBuf::from("phase8_5/campaign-64.kas8");
    let mut evidence = PathBuf::from("phase8_5/campaign-64.json");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--workers" => {
                workers = args
                    .next()
                    .ok_or("missing worker count")?
                    .parse()
                    .map_err(|_| "bad worker count")?
            }
            "--output" => output = args.next().ok_or("missing output")?.into(),
            "--evidence" => evidence = args.next().ok_or("missing evidence")?.into(),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    let started = std::time::Instant::now();
    let result = run_phase85_campaign(workers).map_err(|e| format!("campaign: {e:?}"))?;
    let bytes = encode_phase85_campaign(&result);
    fs::write(&output, &bytes).map_err(|e| e.to_string())?;
    let a = result.aggregate;
    let report = serde_json::json!({
        "schema": "ksa64.phase8_5.campaign-evidence-v1",
        "master_seed": format!("0x{PHASE85_CAMPAIGN_SEED:08x}"),
        "runs": PHASE85_CAMPAIGN_RUNS,
        "workers": workers,
        "record_bytes": bytes.len(),
        "records_crc32": format!("0x{:08x}", a.records_crc32),
        "completed": a.completed,
        "recovery_incomplete": a.recovery_incomplete,
        "model_envelope_exceeded": a.model_envelope_exceeded,
        "alarmed": a.alarmed,
        "saturated": a.saturated,
        "maximum_navigation_error_q13": a.maximum_navigation_error_q13,
        "maximum_attitude_error_turn16": a.maximum_attitude_error_turn16,
        "wall_seconds": started.elapsed().as_secs_f64(),
    });
    fs::write(
        &evidence,
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
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
