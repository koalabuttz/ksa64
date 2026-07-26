use ksa64_host::phase10_link::{run_host_external_transition_probe, run_host_external_with_limit};
use std::io::{self, Write};
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = "127.0.0.1:21010".to_owned();
    let mut max_releases = 33u32;
    let mut transition_probe = false;
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
            "--transition-probe" => transition_probe = true,
            "--pace" => {
                let value = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing pace"))?;
                if value != "externally-paced" && value != "step" {
                    return Err(
                        io::Error::other("Phase 10 stock placement is externally paced").into(),
                    );
                }
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    if max_releases == 0 || max_releases > u32::from(u16::MAX) {
        return Err(io::Error::other("max releases must be 1..65535").into());
    }
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE10_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    eprintln!("Phase 10 externally paced C64 flight connected: {peer}");
    let evidence = if transition_probe {
        run_host_external_transition_probe(&mut stream)
    } else {
        run_host_external_with_limit(&mut stream, max_releases)
    }
    .map_err(|error| io::Error::other(format!("Phase 10 split flight: {error:?}")))?;
    println!(
        "KSA64_PHASE10_BOUNDED releases={} transitions={:02x} sensor={:08x} command={:08x} status={:08x} nav={:08x} flight={:08x}",
        evidence.releases,
        evidence.transition_mask,
        evidence.sensor_checksum,
        evidence.command_checksum,
        evidence.status_checksum,
        evidence.navigation_checksum,
        evidence.flight_checksum,
    );
    Ok(())
}
