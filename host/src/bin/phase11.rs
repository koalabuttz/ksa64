use ksa64_host::application::Ksa64Application;
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
    let application = Ksa64Application::default();
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let command = arguments.first().map(String::as_str).unwrap_or("help");
    match command {
        "lint" => {
            expect_len(&arguments, 2)?;
            let outcome = application
                .lint_project(&read(&arguments[1])?)
                .map_err(app_error)?;
            let source = &outcome.details;
            println!(
                "valid {} / {} / {}",
                source["name"].as_str().unwrap(),
                source["package"].as_str().unwrap(),
                source["scenario"].as_str().unwrap()
            );
        }
        "compile" => {
            expect_len(&arguments, 3)?;
            let outcome = application
                .compile_project(&read(&arguments[1])?, Path::new(&arguments[2]))
                .map_err(app_error)?;
            println!("{}", outcome.summary);
        }
        "inspect" => {
            expect_len(&arguments, 2)?;
            let outcome = application
                .inspect_evidence(Path::new(&arguments[1]))
                .map_err(app_error)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome.details).map_err(|error| error.to_string())?
            );
        }
        "run" | "script" => {
            expect_len(&arguments, 3)?;
            let outcome = application
                .run_project(
                    &read(&arguments[1])?,
                    Path::new(&arguments[2]),
                    command == "script",
                )
                .map_err(app_error)?;
            println!("{}", outcome.summary);
        }
        "replay" => {
            expect_len(&arguments, 2)?;
            let outcome = application
                .replay_evidence(Path::new(&arguments[1]))
                .map_err(app_error)?;
            println!("{}", outcome.summary);
        }
        "debrief" => {
            expect_len(&arguments, 3)?;
            let outcome = application
                .debrief_evidence(Path::new(&arguments[1]), Path::new(&arguments[2]))
                .map_err(app_error)?;
            println!("{}", outcome.summary);
        }
        "verify" => {
            expect_len(&arguments, 2)?;
            let outcome = application
                .verify_evidence(Path::new(&arguments[1]))
                .map_err(app_error)?;
            println!("{}", outcome.summary);
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
fn app_error(error: ksa64_host::application::ApplicationError) -> String {
    format!("{}: {}", error.diagnostic.code, error.diagnostic.message)
}
