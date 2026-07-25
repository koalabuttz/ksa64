use ksa64_host::phase8_5_link::run_host_external_with_limit;
use std::io::{self, Write};
use std::net::TcpListener;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = "127.0.0.1:28585".to_owned();
    let mut gimbal = false;
    let mut max_releases = u32::MAX;
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
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE85_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    stream.set_nodelay(true)?;
    eprintln!("Phase 8.5 external flight connected: {peer}");
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
    Ok(())
}
