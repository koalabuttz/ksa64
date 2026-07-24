use ksa64_host::phase6_runner::{run_native_host_mission, RunnerOptions, RunnerPace};
use std::process::ExitCode;

fn usage() {
    eprintln!("usage: phase6-launch --world host --flight host --mission-control host|disabled --pace fast|realtime|step");
    eprintln!("       use phase6/run.ps1 when --flight vice is selected");
}

fn parse() -> Result<RunnerOptions, String> {
    let mut world = "host".to_owned();
    let mut flight = "host".to_owned();
    let mut mission_control = "host".to_owned();
    let mut pace = "fast".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--world" => world = value,
            "--flight" => flight = value,
            "--mission-control" => mission_control = value,
            "--pace" => pace = value,
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    if world != "host" {
        return Err("only --world host is implemented".into());
    }
    if flight != "host" {
        return Err("use phase6/run.ps1 for --flight vice".into());
    }
    let mission_control = match mission_control.as_str() {
        "host" => true,
        "disabled" => false,
        _ => return Err("--mission-control must be host or disabled".into()),
    };
    let pace = match pace.as_str() {
        "fast" => RunnerPace::Fast,
        "realtime" => RunnerPace::Realtime,
        "step" => RunnerPace::Step,
        _ => return Err("--pace must be fast, realtime, or step".into()),
    };
    Ok(RunnerOptions {
        mission_control,
        pace,
    })
}

fn run() -> Result<(), String> {
    let options = parse()?;
    println!(
        "KSA64 Phase 6: world=host flight=host mission-control={} pace={:?}",
        if options.mission_control {
            "host"
        } else {
            "disabled"
        },
        options.pace
    );
    let evidence =
        run_native_host_mission(options).map_err(|error| format!("mission failed: {error:?}"))?;
    println!("MISSION COMPLETE");
    debug_assert!(evidence.complete);
    println!("  fast epochs: {}", evidence.fast_epochs);
    println!("  mission steps: {}", evidence.mission_steps);
    println!(
        "  terminal position Q12: {:?}",
        evidence.terminal_position_q12
    );
    println!(
        "  terminal velocity Q24: {:?}",
        evidence.terminal_velocity_q24
    );
    println!(
        "  navigation position Q12: {:?}",
        evidence.navigation_position_q12
    );
    println!(
        "  navigation velocity Q24: {:?}",
        evidence.navigation_velocity_q24
    );
    println!(
        "  navigation checksum: 0x{:08x}",
        evidence.navigation_checksum
    );
    println!(
        "  last status flight checksum: 0x{:08x}",
        evidence.status_flight_checksum
    );
    println!(
        "  final flight checksum: 0x{:08x}",
        evidence.final_flight_checksum
    );
    println!(
        "  flight deadline misses / alarms: {} / {}",
        evidence.deadline_misses, evidence.alarms
    );
    if let Some(mission_control) = evidence.mission_control {
        println!("MISSION CONTROL");
        println!(
            "  observed world / flight cells: {} / {}",
            mission_control.world_cells, mission_control.flight_cells
        );
        println!(
            "  independent ground fixes: {}",
            mission_control.ground_fixes
        );
        println!(
            "  transcript checksum: 0x{:08x}",
            mission_control.transcript_checksum
        );
        println!(
            "  ground checksum: 0x{:08x}",
            mission_control.ground_checksum
        );
        println!("  alarms: {}", mission_control.alarms);
        if let Some(comparison) = mission_control.comparison {
            println!(
                "  final ground-onboard position delta Q12: {:?}",
                comparison.position_delta_q12
            );
            println!(
                "  final ground-onboard velocity delta Q24: {:?}",
                comparison.velocity_delta_q24
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            usage();
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
