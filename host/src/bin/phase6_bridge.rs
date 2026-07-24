use ksa64_host::phase6::configure_socket;
use ksa64_host::phase6_runner::{run_world_with_flight, RunnerOptions, RunnerPace};
use std::io::{self, Write};
use std::net::TcpListener;

fn parse() -> Result<(String, bool, u32), String> {
    let mut address = "127.0.0.1:25232".to_owned();
    let mut mission_control = true;
    let mut max_epochs = u32::MAX;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--listen" => address = value,
            "--mission-control" => {
                mission_control = match value.as_str() {
                    "host" => true,
                    "disabled" => false,
                    _ => return Err("mission control must be host or disabled".into()),
                }
            }
            "--max-epochs" => {
                max_epochs = value
                    .parse()
                    .map_err(|_| "max epochs must be a positive integer")?;
                if max_epochs == 0 {
                    return Err("max epochs must be positive".into());
                }
            }
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok((address, mission_control, max_epochs))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, mission_control, max_epochs) = parse().map_err(io::Error::other)?;
    let listener = TcpListener::bind(&address)?;
    println!("KSA64_PHASE6_LISTENING {}", listener.local_addr()?);
    io::stdout().flush()?;
    let (mut stream, peer) = listener.accept()?;
    configure_socket(&stream)?;
    eprintln!("KSA64 phase6 endpoint connected: {peer}");
    let options = RunnerOptions {
        mission_control,
        pace: RunnerPace::Fast,
    };
    let evidence = run_world_with_flight(&mut stream, max_epochs, options)
        .map_err(|error| io::Error::other(format!("bridge: {error:?}")))?;
    println!("KSA64_PHASE6_COMPLETE complete={} epochs={} steps={} position={:?} velocity={:?} nav_position={:?} nav_velocity={:?} status_flight_checksum={} final_flight_checksum={} navigation_checksum={} deadline_misses={} alarms={}", evidence.complete, evidence.fast_epochs, evidence.mission_steps, evidence.terminal_position_q12, evidence.terminal_velocity_q24, evidence.navigation_position_q12, evidence.navigation_velocity_q24, evidence.status_flight_checksum, evidence.final_flight_checksum, evidence.navigation_checksum, evidence.deadline_misses, evidence.alarms);
    if let Some(mc) = evidence.mission_control {
        println!("KSA64_PHASE6_MISSION_CONTROL world_cells={} flight_cells={} ground_fixes={} transcript_checksum={} ground_checksum={} alarms={} comparison={:?}", mc.world_cells, mc.flight_cells, mc.ground_fixes, mc.transcript_checksum, mc.ground_checksum, mc.alarms, mc.comparison);
    }
    Ok(())
}
