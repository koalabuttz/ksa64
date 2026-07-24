use ksa64_host::phase6_runner::{
    run_native_host_mission, RunnerEvidence, RunnerOptions, RunnerPace,
};
use ksa64_host::phase6_session::{default_session_path, Session};
use ksa64_host::phase6_tui::{
    run_native_console, run_native_recorded, run_replay_console, ConsoleConfig, DisplayMode,
    SoundProfile, UnitSystem,
};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

struct Args {
    options: RunnerOptions,
    display: DisplayMode,
    config: ConsoleConfig,
    replay: Option<PathBuf>,
}
fn usage() {
    eprintln!("usage: phase6-launch [--world host] [--flight host] [--mission-control host|disabled] [--pace fast|realtime|step] [--display adaptive|tui|summary|none] [--units si|dual|us] [--sound off|cues|cinematic] [--record auto|off|PATH] [--replay PATH]");
}
fn parse() -> Result<Args, String> {
    let mut options = RunnerOptions::default();
    let mut display = DisplayMode::Adaptive;
    let mut config = ConsoleConfig::default();
    let mut replay = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--world" if value != "host" => return Err("only --world host is implemented".into()),
            "--world" => {}
            "--flight" if value != "host" => {
                return Err("use phase6/run.ps1 for --flight vice".into())
            }
            "--flight" => {}
            "--mission-control" => {
                options.mission_control = match value.as_str() {
                    "host" => true,
                    "disabled" => false,
                    _ => return Err("mission control must be host or disabled".into()),
                }
            }
            "--pace" => {
                options.pace = match value.as_str() {
                    "fast" => RunnerPace::Fast,
                    "realtime" => RunnerPace::Realtime,
                    "step" => RunnerPace::Step,
                    _ => return Err("pace must be fast, realtime, or step".into()),
                }
            }
            "--display" => {
                display = match value.as_str() {
                    "adaptive" => DisplayMode::Adaptive,
                    "tui" => DisplayMode::Tui,
                    "summary" => DisplayMode::Summary,
                    "none" => DisplayMode::None,
                    _ => return Err("display must be adaptive, tui, summary, or none".into()),
                }
            }
            "--units" => {
                config.units = match value.as_str() {
                    "si" => UnitSystem::Si,
                    "dual" => UnitSystem::Dual,
                    "us" => UnitSystem::Us,
                    _ => return Err("units must be si, dual, or us".into()),
                }
            }
            "--sound" => {
                config.sound = match value.as_str() {
                    "off" => SoundProfile::Off,
                    "cues" => SoundProfile::Cues,
                    "cinematic" => SoundProfile::Cinematic,
                    _ => return Err("sound must be off, cues, or cinematic".into()),
                }
            }
            "--record" => {
                config.recording = match value.as_str() {
                    "auto" => Some(default_session_path()),
                    "off" => None,
                    _ => Some(PathBuf::from(value)),
                }
            }
            "--replay" => replay = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(Args {
        options,
        display,
        config,
        replay,
    })
}
fn show(e: &RunnerEvidence) {
    println!("KSA64_PHASE6_COMPLETE complete={} epochs={} operator_stopped={} steps={} position={:?} velocity={:?} nav_position={:?} nav_velocity={:?} status_flight_checksum={} final_flight_checksum={} navigation_checksum={} deadline_misses={} alarms={}",e.complete,e.fast_epochs,e.operator_stopped,e.mission_steps,e.terminal_position_q12,e.terminal_velocity_q24,e.navigation_position_q12,e.navigation_velocity_q24,e.status_flight_checksum,e.final_flight_checksum,e.navigation_checksum,e.deadline_misses,e.alarms);
    if let Some(mc) = e.mission_control {
        println!("KSA64_PHASE6_MISSION_CONTROL world_cells={} flight_cells={} ground_fixes={} transcript_checksum={} ground_checksum={} alarms={} comparison={:?}",mc.world_cells,mc.flight_cells,mc.ground_fixes,mc.transcript_checksum,mc.ground_checksum,mc.alarms,mc.comparison)
    }
}
fn run() -> Result<(), String> {
    let mut a = parse()?;
    if let Some(path) = a.replay.take() {
        let session = Session::load(path).map_err(|e| format!("session: {e:?}"))?;
        return run_replay_console(session, a.config).map_err(|e| format!("replay: {e:?}"));
    }
    let display = match a.display {
        DisplayMode::Adaptive if a.options.pace == RunnerPace::Fast => DisplayMode::Summary,
        DisplayMode::Adaptive if std::io::stderr().is_terminal() => DisplayMode::Tui,
        DisplayMode::Adaptive => DisplayMode::Summary,
        x => x,
    };
    if display == DisplayMode::Tui && !a.options.mission_control {
        return Err("the TUI requires --mission-control host".into());
    }
    let record_path = a.config.recording.clone();
    let evidence = match display {
        DisplayMode::Tui => {
            run_native_console(a.options, a.config).map_err(|e| format!("mission: {e:?}"))?
        }
        _ if record_path.is_some() => run_native_recorded(a.options, record_path.unwrap())
            .map_err(|e| format!("mission: {e:?}"))?,
        _ => run_native_host_mission(a.options).map_err(|e| format!("mission: {e:?}"))?,
    };
    if display != DisplayMode::None {
        show(&evidence)
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            usage();
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
