use ksa64_host::phase8_5_link::{run_host_external, run_host_external_with_limit};
use ksa64_host::phase8_5_tui::{run_local_console_with_worker, ConsolePace, LocalConsoleConfig};
use std::io::{self, Write};
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = "127.0.0.1:28585".to_owned();
    let mut gimbal = false;
    let mut max_releases = u32::MAX;
    let mut display = "summary".to_owned();
    let mut pace = ConsolePace::Fast;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--listen" => {
                address = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing listen address"))?
            }
            "--gimbal" => gimbal = true,
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
                pace = match args
                    .next()
                    .ok_or_else(|| io::Error::other("missing pace"))?
                    .as_str()
                {
                    "fast" => ConsolePace::Fast,
                    "realtime" => ConsolePace::Realtime,
                    other => return Err(io::Error::other(format!("unknown pace {other}")).into()),
                }
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    if display != "summary" && display != "tui" {
        return Err(io::Error::other(format!("unknown display {display}")).into());
    }
    if display == "tui" && max_releases != u32::MAX {
        return Err(io::Error::other("live TUI requires a complete external mission").into());
    }
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE85_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    eprintln!("Phase 8.5 external flight connected: {peer}");
    if display == "tui" {
        let config = LocalConsoleConfig {
            gimbal,
            pace,
            title: "KSA64 // HOST WORLD + VICE/C64 FLIGHT COMPUTER".into(),
        };
        let evidence = run_local_console_with_worker(config, move |sink| {
            run_host_external(&mut stream, gimbal, Some(sink))
                .map(|_| ())
                .map_err(|_| ())
        })
        .map_err(|error| io::Error::other(format!("mission control: {error:?}")))?;
        println!(
            "KSA64_PHASE85_COMPLETE placement={:?} releases={} outcome={:?} checksums={:?}",
            evidence.placement,
            evidence.releases,
            evidence.summary.physical.outcome,
            evidence.summary.checksum_chains
        );
    } else {
        let evidence = run_host_external_with_limit(&mut stream, gimbal, None, max_releases)
            .map_err(|error| io::Error::other(format!("external flight: {error:?}")))?;
        if let Some(evidence) = evidence {
            println!(
                "KSA64_PHASE85_COMPLETE placement={:?} releases={} outcome={:?} checksums={:?}",
                evidence.placement,
                evidence.releases,
                evidence.summary.physical.outcome,
                evidence.summary.checksum_chains
            )
        } else {
            println!("KSA64_PHASE85_BOUNDED releases={max_releases}")
        }
    }
    Ok(())
}
