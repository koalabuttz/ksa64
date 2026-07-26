use ksa64_host::phase11_authoring::compile_project_source;
use ksa64_host::phase11_tui::run_operations_console;
use std::env;
use std::fs;

fn main() {
    if let Err(error) = run() {
        eprintln!("phase11-mission-control: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty() || arguments.iter().any(|item| item == "--help") {
        println!(
            "KSA64 Phase 11 Mission Control\n\
             usage: phase11_mission_control SOURCE.json [--role ROLE]\n\
             roles: observer, guided-operator, flight-controller, \
             flight-software-engineer, sim-director, scripted-operator"
        );
        return Ok(());
    }
    let mut source =
        fs::read_to_string(&arguments[0]).map_err(|error| format!("{}: {error}", arguments[0]))?;
    if let Some(index) = arguments.iter().position(|value| value == "--role") {
        let role = arguments
            .get(index + 1)
            .ok_or_else(|| "--role requires a value".to_string())?;
        let mut value: serde_json::Value =
            serde_json::from_str(&source).map_err(|error| error.to_string())?;
        value["role"] = serde_json::Value::String(role.clone());
        source = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    }
    let project = compile_project_source(&source).map_err(|error| format!("{error:?}"))?;
    let completed = run_operations_console(&project).map_err(|error| format!("{error:?}"))?;
    println!(
        "session evidence 0x{:08x}",
        completed.evidence.evidence_identity
    );
    Ok(())
}
