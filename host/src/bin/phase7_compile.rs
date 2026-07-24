use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ksa64_host::phase7_compiler::compile_sources;

fn source(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name)).unwrap_or_else(|error| panic!("read {name}: {error}"))
}

fn main() {
    let mut arguments = env::args_os().skip(1);
    let source_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let output_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    if arguments.next().is_some() {
        eprintln!("usage: phase7_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2);
    }
    let packs = compile_sources(
        &source(&source_directory, "firestorm54.json"),
        &source(&source_directory, "aerotech-i211w.json"),
        &source(&source_directory, "firestorm-i211-mission.json"),
    )
    .unwrap_or_else(|error| panic!("compile Phase 7 sources: {error:?}"));
    fs::create_dir_all(&output_directory).expect("create output directory");
    for (name, bytes) in [
        ("firestorm54.kvp7", packs.vehicle.as_slice()),
        ("aerotech-i211w.kmp7", packs.motor.as_slice()),
        ("firestorm-i211.kmc7", packs.mission.as_slice()),
    ] {
        fs::write(output_directory.join(name), bytes)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }
    println!(
        "compiled KVP7={} bytes KMP7={} bytes KMC7={} bytes",
        packs.vehicle.len(),
        packs.motor.len(),
        packs.mission.len()
    );
}
