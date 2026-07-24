use ksa64_host::phase6::{configure_socket, run_realtime_world_bridge};
use std::io::{self, Write};
use std::net::TcpListener;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:25232".into());
    let listener = TcpListener::bind(&addr)?;
    println!("KSA64_PHASE6_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    configure_socket(&stream)?;
    eprintln!("KSA64 phase6 endpoint connected: {peer}");
    let e = run_realtime_world_bridge(&mut stream, u32::MAX)
        .map_err(|e| io::Error::other(format!("bridge: {e:?}")))?;
    println!("KSA64_PHASE6_COMPLETE epochs={} steps={} position={:?} velocity={:?} nav_position={:?} nav_velocity={:?} status_flight_checksum={} final_flight_checksum={} navigation_checksum={} deadline_misses={} alarms={}",e.fast_epochs,e.mission_steps,e.terminal_position_q12,e.terminal_velocity_q24,e.navigation_position_q12,e.navigation_velocity_q24,e.flight_checksum,e.final_flight_checksum,e.navigation_checksum,e.deadline_misses,e.alarms);
    Ok(())
}
