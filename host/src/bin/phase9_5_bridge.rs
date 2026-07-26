use ksa64_host::phase9_5_link::run_host_external_with_limit;
use std::io::{self, Write};
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = "127.0.0.1:29595".to_owned();
    let mut max_releases = 8u32;
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
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE95_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    eprintln!("Phase 9.5 externally paced C64 flight connected: {peer}");
    let e = run_host_external_with_limit(&mut stream, max_releases)
        .map_err(|error| io::Error::other(format!("advanced external flight: {error:?}")))?;
    println!(
        "KSA64_PHASE95_BOUNDED releases={} sensor={:08x} command={:08x} status={:08x} truth={:08x} nav={:08x} flight={:08x} allocator={:08x}",
        e.releases, e.sensor_checksum, e.command_checksum, e.status_checksum,
        e.truth_checksum, e.navigation_checksum, e.flight_checksum, e.allocator_checksum
    );
    Ok(())
}
