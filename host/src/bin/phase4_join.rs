use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use ksa64_host::phase4_export::join_volumes;

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let output = PathBuf::from(arguments.next().ok_or("missing output path")?);
    let paths: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err("missing KXV4 volumes".to_owned());
    }
    let mut volumes = Vec::with_capacity(paths.len());
    for path in paths {
        volumes.push(fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?);
    }
    let logical = join_volumes(&volumes).map_err(|error| format!("join: {error:?}"))?;
    fs::write(&output, &logical).map_err(|error| format!("{}: {error}", output.display()))?;
    println!(
        "joined {} volumes into {} bytes",
        volumes.len(),
        logical.len()
    );
    Ok(())
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
