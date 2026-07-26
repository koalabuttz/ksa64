use ksa64_host::phase9_5_link::{
    run_host_external_with_limit, run_host_external_with_limit_observed, Phase95Placement,
    Phase95RecordingSink,
};
use ksa64_host::phase9_5_tui::{
    run_advanced_console_with_worker, AdvancedConsoleConfig, AdvancedConsolePace,
};
use std::fs;
use std::io::{self, Write};
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = "127.0.0.1:29595".to_owned();
    let mut max_releases = 8u32;
    let mut display = "summary".to_owned();
    let mut record = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--listen" => {
                address = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing listen address"))?
            }
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
            "--record" => {
                record = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("missing record path"))?,
                )
            }
            "--pace" => {
                let value = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing pace"))?;
                if value != "externally-paced" && value != "step" {
                    return Err(io::Error::other("Phase 9.5 stock baseline is externally paced; realtime is not yet accepted").into());
                }
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
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE95_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    eprintln!("Phase 9.5 externally paced C64 flight connected: {peer}");

    let evidence = if display == "tui" {
        let output = run_advanced_console_with_worker(
            AdvancedConsoleConfig {
                title: "KSA64 // HOST WORLD + STOCK C64 ADVANCED FLIGHT".into(),
                pace: AdvancedConsolePace::Fast,
                auto_exit: max_releases != u32::from(u16::MAX),
            },
            move |sink| {
                run_host_external_with_limit_observed(&mut stream, max_releases, Some(sink))
                    .map(|_| ())
                    .map_err(|_| ())
            },
        )
        .map_err(|error| io::Error::other(format!("advanced mission control: {error:?}")))?;
        if let Some(path) = record {
            fs::write(path, serde_json::to_vec_pretty(&output.recording())?)?;
        }
        output.evidence
    } else if let Some(path) = record {
        let mut sink = Phase95RecordingSink::new(Phase95Placement::HostExternalFlight);
        let evidence =
            run_host_external_with_limit_observed(&mut stream, max_releases, Some(&mut sink))
                .map_err(|error| {
                    io::Error::other(format!("advanced external flight: {error:?}"))
                })?;
        fs::write(path, serde_json::to_vec_pretty(&sink.recording(&evidence))?)?;
        evidence
    } else {
        run_host_external_with_limit(&mut stream, max_releases)
            .map_err(|error| io::Error::other(format!("advanced external flight: {error:?}")))?
    };
    if display != "none" {
        println!(
            "KSA64_PHASE95_BOUNDED releases={} sensor={:08x} command={:08x} status={:08x} truth={:08x} nav={:08x} flight={:08x} allocator={:08x}",
            evidence.releases, evidence.sensor_checksum, evidence.command_checksum,
            evidence.status_checksum, evidence.truth_checksum, evidence.navigation_checksum,
            evidence.flight_checksum, evidence.allocator_checksum
        );
    }
    Ok(())
}
