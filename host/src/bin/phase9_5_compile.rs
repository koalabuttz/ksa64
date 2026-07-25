use std::env;
use std::fs;
use std::path::PathBuf;

use ksa64_host::phase9_5_compiler::compile_advanced_sources;

fn main() {
    let mut args = env::args_os().skip(1);
    let source = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: phase9_5_compile BASE_KVP8 SOURCE_JSON OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let advanced = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: phase9_5_compile BASE_KVP8 SOURCE_JSON OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let output = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: phase9_5_compile BASE_KVP8 SOURCE_JSON OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    if args.next().is_some() {
        eprintln!("usage: phase9_5_compile BASE_KVP8 SOURCE_JSON OUTPUT_DIRECTORY");
        std::process::exit(2)
    }
    let base =
        fs::read(&source).unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
    let source_bytes =
        fs::read(&advanced).unwrap_or_else(|error| panic!("read {}: {error}", advanced.display()));
    let compiled = compile_advanced_sources(&base, &source_bytes)
        .unwrap_or_else(|error| panic!("compile advanced effectors: {error:?}"));
    fs::create_dir_all(&output).expect("create output directory");
    let mut reports = Vec::new();
    for variant in compiled.variants {
        let stem = variant.name.to_ascii_lowercase();
        fs::write(output.join(format!("{stem}.kvp8")), variant.vehicle).expect("write KVP8");
        fs::write(output.join(format!("{stem}.kpe9")), variant.effector).expect("write KPE9");
        fs::write(output.join(format!("{stem}.kpa9")), variant.allocator).expect("write KPA9");
        reports.push(variant.report);
    }
    fs::write(
        output.join("compile-report.json"),
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .expect("write report");
    println!("compiled {} Phase 9.5 variants", reports.len());
}
