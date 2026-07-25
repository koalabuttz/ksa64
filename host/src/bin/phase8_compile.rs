use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ksa64_host::phase8_compiler::compile_spatial_sources;

fn source(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name)).unwrap_or_else(|error| panic!("read {name}: {error}"))
}

fn main() {
    let mut arguments = env::args_os().skip(1);
    let source_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase8_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let output_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase8_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    if arguments.next().is_some() {
        eprintln!("usage: phase8_compile SOURCE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2);
    }
    let packs = compile_spatial_sources(
        &source(&source_directory, "firestorm54-spatial.json"),
        &source(&source_directory, "aerotech-i211w-spatial.json"),
        &source(&source_directory, "firestorm-i211-spatial-mission.json"),
        &source(&source_directory, "calm-wind.json"),
    )
    .unwrap_or_else(|error| panic!("compile Phase 8 sources: {error:?}"));
    fs::create_dir_all(&output_directory).expect("create output directory");
    for (name, bytes) in [
        ("firestorm54.kvp8", packs.vehicle.as_slice()),
        ("aerotech-i211w.kmp8", packs.motor.as_slice()),
        ("firestorm-i211.kmc8", packs.mission.as_slice()),
        ("firestorm-calm.kwp8", packs.wind.as_slice()),
    ] {
        fs::write(output_directory.join(name), bytes)
            .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }
    let report = serde_json::to_vec_pretty(&packs.report).expect("serialize compile report");
    fs::write(output_directory.join("compile-report.json"), report).expect("write compile report");

    println!(
        "compiled KVP8={} KMP8={} KMC8={} KWP8={} bytes; dry mass {:.9} kg, CG {:.6} m",
        packs.vehicle.len(),
        packs.motor.len(),
        packs.mission.len(),
        packs.wind.len(),
        packs.report.dry_mass_kg,
        packs.report.dry_cg_from_nose_m
    );
}
