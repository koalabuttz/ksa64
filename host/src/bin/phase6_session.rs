use ksa64_host::phase6_session::Session;
use std::path::PathBuf;
use std::process::ExitCode;
fn run() -> Result<(), String> {
    let mut input = None;
    let mut csv = None;
    let mut json = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = PathBuf::from(
            args.next()
                .ok_or_else(|| format!("missing value for {flag}"))?,
        );
        match flag.as_str() {
            "--input" => input = Some(value),
            "--csv" => csv = Some(value),
            "--json" => json = Some(value),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    let input = input.ok_or("--input is required")?;
    let session = Session::load(&input).map_err(|e| format!("session: {e:?}"))?;
    if let Some(path) = csv {
        session
            .export_csv(path)
            .map_err(|e| format!("csv: {e:?}"))?
    }
    if let Some(path) = json {
        session
            .export_json(path)
            .map_err(|e| format!("json: {e:?}"))?
    }
    println!(
        "KMR6_SESSION updates={} complete={} recovered={} evidence={}",
        session.updates.len(),
        session.complete,
        session.recovered,
        session.evidence.is_some()
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
