use ksa64_host::phase10_mission::{
    capture_nominal_global_mission, q12, q16, write_global_mission_artifacts,
};
use ksa64_host::phase10_tui::{run_global_console, GlobalConsoleConfig, GlobalConsolePace};
use std::io;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut display = "tui".to_owned();
    let mut pace = GlobalConsolePace::Fast;
    let mut auto_exit = false;
    let mut output = PathBuf::from("phase10/evidence");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--display" => {
                display = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing display"))?
            }
            "--pace" => {
                pace = match args.next().as_deref() {
                    Some("fast") => GlobalConsolePace::Fast,
                    Some("realtime") => GlobalConsolePace::Realtime,
                    _ => return Err(io::Error::other("pace must be fast or realtime").into()),
                }
            }
            "--auto-exit" => auto_exit = true,
            "--output" => {
                output = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing output path"))?
                    .into()
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    if !matches!(display.as_str(), "tui" | "summary" | "none") {
        return Err(io::Error::other("display must be tui, summary, or none").into());
    }
    let capture = if display == "tui" {
        run_global_console(GlobalConsoleConfig {
            title: "KSA64 // KSA-G10R GLOBAL MISSION CONTROL".into(),
            pace,
            auto_exit,
        })
        .map_err(|error| io::Error::other(format!("mission control: {error:?}")))?
    } else {
        capture_nominal_global_mission(|_| {})
            .map_err(|error| io::Error::other(format!("global mission: {error:?}")))?
    };
    write_global_mission_artifacts(&capture, &output)
        .map_err(|error| io::Error::other(format!("artifacts: {error}")))?;
    if display != "none" {
        println!(
            "KSA64_PHASE10_COMPLETE outcome={:?} releases={} duration={:.3}s apogee={:.3}km downrange={:.3}km transitions={} evidence={}",
            capture.summary.common.outcome,
            capture.releases,
            capture
                .frames
                .last()
                .map_or(0.0, |frame| q16(frame.mission_time_q16)),
            q12(capture.summary.apogee_q12_km),
            q12(capture.summary.downrange_q12_km),
            capture.summary.transition_count,
            output.display(),
        );
    }
    Ok(())
}
