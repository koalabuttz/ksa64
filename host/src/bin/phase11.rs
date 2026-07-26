use ksa64_host::phase11_authoring::{
    build_definition_bundle, compile_project_source, complete_project_session, inspect_bundle,
    lint_project_source, project_from_bundle, replay_completed_session, verify_session,
    write_debrief_reports,
};
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("phase11: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let command = arguments.first().map(String::as_str).unwrap_or("help");
    match command {
        "lint" => {
            expect_len(&arguments, 2)?;
            let source = read(&arguments[1])?;
            let project = lint_project_source(&source).map_err(debug)?;
            println!(
                "valid {} / {} / {}",
                project.name, project.package, project.scenario
            );
        }
        "compile" => {
            expect_len(&arguments, 3)?;
            let source = read(&arguments[1])?;
            let project = compile_project_source(&source).map_err(debug)?;
            let bundle = build_definition_bundle(&project).map_err(debug)?;
            write(&arguments[2], &bundle)?;
            println!(
                "compiled definition 0x{:08x} ({} bytes)",
                project.definition_identity,
                bundle.len()
            );
        }
        "inspect" => {
            expect_len(&arguments, 2)?;
            let bytes = read_bytes(&arguments[1])?;
            let report = inspect_bundle(&bytes).map_err(debug)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
            );
        }
        "run" | "script" => {
            expect_len(&arguments, 3)?;
            let source = read(&arguments[1])?;
            let project = compile_project_source(&source).map_err(debug)?;
            let completed =
                complete_project_session(&project, command == "script").map_err(debug)?;
            write(&arguments[2], &completed.bundle)?;
            println!(
                "completed evidence 0x{:08x} ({} bytes)",
                completed.evidence.evidence_identity,
                completed.bundle.len()
            );
        }
        "replay" => {
            expect_len(&arguments, 2)?;
            let bytes = read_bytes(&arguments[1])?;
            let replay = replay_completed_session(&bytes).map_err(debug)?;
            println!(
                "exact replay 0x{:08x}; flight 0x{:08x}; nav 0x{:08x}",
                replay.evidence_identity, replay.flight_checksum, replay.navigation_checksum
            );
        }
        "debrief" => {
            expect_len(&arguments, 3)?;
            let bytes = read_bytes(&arguments[1])?;
            let project = project_from_bundle(&bytes).map_err(debug)?;
            let completed = complete_project_session(&project, true).map_err(debug)?;
            if completed.bundle != bytes {
                return Err("session replay differs; refusing derived report".into());
            }
            write_debrief_reports(&completed, Path::new(&arguments[2])).map_err(debug)?;
            println!("wrote deterministic debrief reports");
        }
        "verify" => {
            expect_len(&arguments, 2)?;
            let bytes = read_bytes(&arguments[1])?;
            let scan = verify_session(&bytes).map_err(debug)?;
            println!(
                "verified completed session 0x{:08x}; {} segments",
                scan.identity.completed_evidence,
                scan.segments.len()
            );
        }
        "help" | "--help" | "-h" => usage(),
        _ => return Err(format!("unknown command {command}\n{}", usage_text())),
    }
    Ok(())
}

fn usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "KSA64 Phase 11 mission SDK\n\
     phase11 lint SOURCE.json\n\
     phase11 compile SOURCE.json DEFINITION.ksb11\n\
     phase11 inspect BUNDLE.ksb11\n\
     phase11 run SOURCE.json SESSION.ksb11\n\
     phase11 script SOURCE.json SESSION.ksb11\n\
     phase11 replay SESSION.ksb11\n\
     phase11 debrief SESSION.ksb11 OUTPUT_DIR\n\
     phase11 verify SESSION.ksb11"
}

fn expect_len(arguments: &[String], expected: usize) -> Result<(), String> {
    (arguments.len() == expected)
        .then_some(())
        .ok_or_else(|| usage_text().to_string())
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("{path}: {error}"))
}

fn write(path: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("{path}: {error}"))
}

fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
