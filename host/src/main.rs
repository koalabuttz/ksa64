use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ksa64_core::phase2_scenario::parse_phase2_scenario;
use ksa64_core::scenario::parse_scenario_image;
use ksa64_host::phase2::{capture_phase2_mission, format_phase2_inspection, inspect_phase2_stream};
use ksa64_host::{capture_mission, format_inspection, inspect_stream};

const SCENARIO_IMAGE: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");
const PHASE2_SCENARIO_IMAGE: &[u8; 884] = include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

fn usage(program: &str) {
    eprintln!(
        "usage: {program} capture <stream.kst> | inspect <stream.kst> | phase2-capture <stream.kst2> | phase2-inspect <stream.kst2>"
    );
}

fn inspect_path(path: &Path) -> Result<(), String> {
    let scenario = parse_scenario_image(SCENARIO_IMAGE)
        .map_err(|error| format!("built-in scenario is invalid: {error:?}"))?;
    let stream =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let inspection = inspect_stream(&stream, &scenario).map_err(|error| {
        format!(
            "{} is not a valid mission stream: {error:?}",
            path.display()
        )
    })?;
    print!("{}", format_inspection(inspection));
    Ok(())
}

fn capture_path(path: &Path) -> Result<(), String> {
    let scenario = parse_scenario_image(SCENARIO_IMAGE)
        .map_err(|error| format!("built-in scenario is invalid: {error:?}"))?;
    let file = File::create(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let summary = capture_mission(&scenario, &mut writer)
        .map_err(|error| format!("mission capture failed: {error:?}"))?;
    writer
        .flush()
        .map_err(|error| format!("could not flush {}: {error}", path.display()))?;
    eprintln!(
        "captured {} frames through step {} to {}",
        summary.frames_written(),
        summary.mission().completed_steps(),
        path.display()
    );
    inspect_path(path)
}

fn inspect_phase2_path(path: &Path) -> Result<(), String> {
    let scenario = parse_phase2_scenario(PHASE2_SCENARIO_IMAGE)
        .map_err(|error| format!("built-in Phase 2 scenario is invalid: {error:?}"))?;
    let stream =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let inspection = inspect_phase2_stream(&stream, &scenario).map_err(|error| {
        format!(
            "{} is not a valid Phase 2 stream: {error:?}",
            path.display()
        )
    })?;
    print!("{}", format_phase2_inspection(inspection));
    Ok(())
}

fn capture_phase2_path(path: &Path) -> Result<(), String> {
    let scenario = parse_phase2_scenario(PHASE2_SCENARIO_IMAGE)
        .map_err(|error| format!("built-in Phase 2 scenario is invalid: {error:?}"))?;
    let file = File::create(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let summary = capture_phase2_mission(&scenario, &mut writer)
        .map_err(|error| format!("Phase 2 mission capture failed: {error:?}"))?;
    writer
        .flush()
        .map_err(|error| format!("could not flush {}: {error}", path.display()))?;
    eprintln!(
        "captured {} Phase 2 frames through step {} to {}",
        summary.frames_written(),
        summary.mission().truth().step(),
        path.display()
    );
    inspect_phase2_path(path)
}
fn run() -> Result<(), String> {
    let mut arguments = env::args();
    let program = arguments.next().unwrap_or_else(|| "ksa64-host".to_owned());
    let command = arguments.next();
    let path = arguments.next().map(PathBuf::from);
    if arguments.next().is_some() {
        usage(&program);
        return Err("too many arguments".to_owned());
    }
    match (command.as_deref(), path) {
        (Some("capture"), Some(path)) => capture_path(&path),
        (Some("inspect"), Some(path)) => inspect_path(&path),
        (Some("phase2-capture"), Some(path)) => capture_phase2_path(&path),
        (Some("phase2-inspect"), Some(path)) => inspect_phase2_path(&path),
        _ => {
            usage(&program);
            Err("missing or invalid command".to_owned())
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
