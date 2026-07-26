use ksa64_host::phase9_5_link::{
    run_host_native_with_limit_observed, Phase95Placement, Phase95RecordingSink,
};
use ksa64_host::phase9_5_tui::{
    run_advanced_console_with_worker, AdvancedConsoleConfig, AdvancedConsolePace,
};
use std::fs;
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut max_releases = u32::from(u16::MAX);
    let mut display = "tui".to_owned();
    let mut pace = AdvancedConsolePace::Realtime;
    let mut record = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--max-releases" => {
                max_releases = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing max releases"))?
                    .parse()?
            }
            "--display" => {
                display = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing display"))?
            }
            "--pace" => {
                pace = match args.next().as_deref() {
                    Some("fast") => AdvancedConsolePace::Fast,
                    Some("realtime") => AdvancedConsolePace::Realtime,
                    _ => return Err(io::Error::other("pace must be fast or realtime").into()),
                }
            }
            "--record" => {
                record = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("missing record path"))?,
                )
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    if max_releases == 0 || max_releases > u32::from(u16::MAX) {
        return Err(io::Error::other("max releases must be 1..65535").into());
    }
    if display != "summary" && display != "tui" && display != "none" {
        return Err(io::Error::other("display must be summary, tui, or none").into());
    }
    let evidence = if display == "tui" {
        let output = run_advanced_console_with_worker(
            AdvancedConsoleConfig {
                title: "KSA64 // HOST WORLD + HOST ADVANCED FLIGHT".into(),
                pace,
                auto_exit: pace == AdvancedConsolePace::Fast,
            },
            move |sink| {
                run_host_native_with_limit_observed(max_releases, Some(sink))
                    .map(|_| ())
                    .map_err(|_| ())
            },
        )
        .map_err(|error| io::Error::other(format!("advanced mission control: {error:?}")))?;
        if let Some(path) = record {
            fs::write(path, serde_json::to_vec_pretty(&output.recording())?)?;
        }
        output.evidence
    } else {
        let mut sink = Phase95RecordingSink::new(Phase95Placement::HostHost);
        let evidence = run_host_native_with_limit_observed(
            max_releases,
            record
                .as_ref()
                .map(|_| &mut sink as &mut dyn ksa64_host::phase9_5_link::Phase95Sink),
        )
        .map_err(|error| io::Error::other(format!("advanced host mission: {error:?}")))?;
        if let Some(path) = record {
            fs::write(path, serde_json::to_vec_pretty(&sink.recording(&evidence))?)?;
        }
        evidence
    };
    if display != "none" {
        println!("KSA64_PHASE95_COMPLETE placement={:?} releases={} truth={:08x} nav={:08x} flight={:08x} allocator={:08x}",evidence.placement,evidence.releases,evidence.truth_checksum,evidence.navigation_checksum,evidence.flight_checksum,evidence.allocator_checksum);
    }
    Ok(())
}
