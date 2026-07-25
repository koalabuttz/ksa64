use ksa64_host::phase8_5::{checked_in_reference, run_host_host, LocalPlacement, RecordingSink};
use ksa64_host::phase8_5_tui::{run_local_console, ConsolePace, LocalConsoleConfig};
use std::fs;
use std::process::ExitCode;
fn run() -> Result<(), String> {
    let mut gimbal = false;
    let mut display = "summary".to_owned();
    let mut pace = ConsolePace::Realtime;
    let mut record = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--gimbal" => gimbal = true,
            "--display" => display = args.next().ok_or("missing display")?,
            "--pace" => {
                pace = match args.next().as_deref() {
                    Some("fast") => ConsolePace::Fast,
                    Some("realtime") => ConsolePace::Realtime,
                    _ => return Err("pace must be fast or realtime".into()),
                }
            }
            "--record" => record = Some(args.next().ok_or("missing record path")?),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    if display == "tui" {
        let evidence = run_local_console(LocalConsoleConfig {
            gimbal,
            pace,
            title: "KSA64 // PHASE 8.5 LOCAL-ENU MISSION CONTROL".into(),
        })
        .map_err(|e| format!("console: {e:?}"))?;
        println!(
            "KSA64_PHASE85_COMPLETE placement={:?} releases={} outcome={:?} checksums={:?}",
            evidence.placement,
            evidence.releases,
            evidence.summary.physical.outcome,
            evidence.summary.checksum_chains
        );
        return Ok(());
    }
    if display != "summary" && display != "none" {
        return Err("display must be tui, summary, or none".into());
    }
    let reference = checked_in_reference(gimbal).map_err(|e| format!("reference: {e:?}"))?;
    let mut sink = RecordingSink::new(reference, LocalPlacement::HostHost);
    let evidence = run_host_host(gimbal, Some(&mut sink)).map_err(|e| format!("mission: {e:?}"))?;
    if let Some(path) = record {
        let recording = sink.recording(evidence.summary.checksum_chains);
        fs::write(
            path,
            serde_json::to_vec_pretty(&recording).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?
    }
    if display == "summary" {
        println!(
            "KSA64_PHASE85_COMPLETE placement={:?} releases={} outcome={:?} checksums={:?}",
            evidence.placement,
            evidence.releases,
            evidence.summary.physical.outcome,
            evidence.summary.checksum_chains
        )
    }
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
