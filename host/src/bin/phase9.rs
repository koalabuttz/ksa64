//! KSA64 Phase 9 optimization workbench CLI.
use ksa64_core::phase9_contract::{SearchEngineId, SearchManifest, SearchPresetId};
use ksa64_host::phase9::{baseline_vector, built_in_manifest, evaluate_candidate, StudyId};
use ksa64_host::phase9_archive::{encode_kpf9, write_search_archive_atomic};
use ksa64_host::phase9_manifest::compile_manifest_json;
use ksa64_host::phase9_protocol::serve_jsonl;
use ksa64_host::phase9_report::{report_csv, report_html, report_json};
use ksa64_host::phase9_search::{run_search_with_workers, SearchError};
use ksa64_host::phase9_sensitivity::{encode_ksn9, one_at_a_time};
use ksa64_host::phase9_tui::run_optimization_tui;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
fn main() {
    if let Err(e) = run() {
        eprintln!("phase9: {e}");
        std::process::exit(2)
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(usage());
    }
    match args[1].as_str() {
        "compile" => {
            let source =
                fs::read_to_string(args.get(2).ok_or_else(usage)?).map_err(|e| e.to_string())?;
            let (_, manifest) =
                compile_manifest_json(&source).map_err(|e| format!("manifest source: {e:?}"))?;
            fs::write(
                args.get(3).ok_or_else(usage)?,
                manifest.encode().map_err(|e| format!("KOM9: {e:?}"))?,
            )
            .map_err(|e| e.to_string())?;
            println!("compiled KOM9 {:08x}", manifest.identity);
            Ok(())
        }
        "search-kom9" => {
            let bytes = fs::read(args.get(2).ok_or_else(usage)?).map_err(|e| e.to_string())?;
            let manifest = SearchManifest::parse(&bytes).map_err(|e| format!("KOM9: {e:?}"))?;
            let study = study_from_manifest(&manifest)?;
            let output = PathBuf::from(
                args.get(3)
                    .cloned()
                    .unwrap_or_else(|| "phase9-output".into()),
            );
            let workers = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(1);
            execute_search(
                manifest,
                study,
                &output,
                workers,
                args.iter().any(|a| a == "--tui"),
            )
        }
        "serve-kom9" => {
            let bytes = fs::read(args.get(2).ok_or_else(usage)?).map_err(|e| e.to_string())?;
            let manifest = SearchManifest::parse(&bytes).map_err(|e| format!("KOM9: {e:?}"))?;
            let study = study_from_manifest(&manifest)?;
            serve(manifest, study, args.get(3))
        }
        "search" | "serve" => {
            let study = parse_study(args.get(2).map(String::as_str).unwrap_or("study-a"))?;
            let engine = parse_engine(args.get(3).map(String::as_str).unwrap_or("grid"))?;
            let preset = parse_preset(args.get(4).map(String::as_str).unwrap_or("quick"))?;
            let manifest = built_in_manifest(study, engine, preset);
            if args[1] == "serve" {
                serve(manifest, study, args.get(5))
            } else {
                let output = PathBuf::from(
                    args.get(5)
                        .cloned()
                        .unwrap_or_else(|| "phase9-output".into()),
                );
                let workers = args.get(6).and_then(|v| v.parse().ok()).unwrap_or(1);
                execute_search(
                    manifest,
                    study,
                    &output,
                    workers,
                    args.iter().any(|a| a == "--tui"),
                )
            }
        }
        _ => Err(usage()),
    }
}
fn execute_search(
    manifest: SearchManifest,
    study: StudyId,
    output: &Path,
    workers: usize,
    tui: bool,
) -> Result<(), String> {
    let evaluator = |d: &ksa64_core::phase9_contract::DesignVector, t: u8| {
        evaluate_candidate(&manifest, d, study, t).map_err(|_| SearchError::Evaluation)
    };
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let axes = if manifest.engine == SearchEngineId::GridV1 {
        vec![0, 1]
    } else {
        vec![]
    };
    let result = run_search_with_workers(&manifest, &evaluator, &axes, workers)
        .map_err(|e| format!("search failed: {e:?}"))?;
    fs::write(
        output.join("manifest.kom9"),
        manifest.encode().map_err(|e| format!("manifest: {e:?}"))?,
    )
    .map_err(|e| e.to_string())?;
    write_search_archive_atomic(
        &output.join("search.kra9"),
        manifest.identity,
        &result.generations,
        &result.evidence,
    )
    .map_err(|e| format!("archive: {e:?}"))?;
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report_json(&manifest, &result)).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    fs::write(output.join("report.csv"), report_csv(&result)).map_err(|e| e.to_string())?;
    fs::write(output.join("report.html"), report_html(&manifest, &result))
        .map_err(|e| e.to_string())?;
    result.generations.last().ok_or("empty search")?;
    let mut candidate_history = std::collections::BTreeMap::new();
    for generation in &result.generations {
        for (candidate, aggregate) in generation.candidates.iter().zip(&generation.aggregates) {
            candidate_history.insert(candidate.identity, (*candidate, *aggregate));
        }
    }
    let mut finalists = Vec::new();
    for finalist in &result.finalists {
        if let Some((candidate, _)) = candidate_history.get(&finalist.aggregate.candidate_identity)
        {
            finalists.push((*candidate, finalist.aggregate))
        }
    }
    fs::write(
        output.join("finalists.kfp9"),
        encode_kpf9(manifest.identity, study.raw(), &finalists)
            .map_err(|e| format!("finalists: {e:?}"))?,
    )
    .map_err(|e| e.to_string())?;
    let base = baseline_vector(&manifest);
    let sensitivity = one_at_a_time(&manifest, &base, &evaluator, 8)
        .map_err(|e| format!("sensitivity: {e:?}"))?;
    let mut bytes = Vec::new();
    for r in sensitivity {
        bytes.extend_from_slice(&encode_ksn9(r))
    }
    fs::write(output.join("sensitivity.ksn9"), bytes).map_err(|e| e.to_string())?;
    println!(
        "manifest {:08x}: {} generations, {} evaluations, {} Pareto candidates, {} finalists",
        manifest.identity,
        result.generations.len(),
        result.evaluations,
        result.pareto_indices.len(),
        result.finalists.len()
    );
    if tui {
        run_optimization_tui(&result).map_err(|e| format!("TUI: {e:?}"))?
    }
    Ok(())
}
fn serve(
    manifest: SearchManifest,
    study: StudyId,
    transcript: Option<&String>,
) -> Result<(), String> {
    let evaluator = |d: &ksa64_core::phase9_contract::DesignVector, t: u8| {
        evaluate_candidate(&manifest, d, study, t).map_err(|_| SearchError::Evaluation)
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut capture = Vec::new();
    serve_jsonl(
        &manifest,
        &evaluator,
        stdin.lock(),
        stdout.lock(),
        &mut capture,
    )
    .map_err(|e| format!("protocol: {e:?}"))?;
    if let Some(path) = transcript {
        fs::write(path, capture).map_err(|e| e.to_string())?
    }
    Ok(())
}
fn study_from_manifest(m: &SearchManifest) -> Result<StudyId, String> {
    match m.base_ids[6] {
        x if x == StudyId::PassiveRecovery.raw() => Ok(StudyId::PassiveRecovery),
        x if x == StudyId::GimbalControl.raw() => Ok(StudyId::GimbalControl),
        x if x == StudyId::Coupled.raw() => Ok(StudyId::Coupled),
        x if x == StudyId::ExperimentalAirframe.raw() => Ok(StudyId::ExperimentalAirframe),
        _ => Err("unknown study identity in KOM9".into()),
    }
}
fn parse_study(v: &str) -> Result<StudyId, String> {
    match v {
        "study-a" | "passive" => Ok(StudyId::PassiveRecovery),
        "study-b" | "gimbal" => Ok(StudyId::GimbalControl),
        "coupled" => Ok(StudyId::Coupled),
        "experimental" => Ok(StudyId::ExperimentalAirframe),
        _ => Err(format!("unknown study {v}")),
    }
}
fn parse_engine(v: &str) -> Result<SearchEngineId, String> {
    match v {
        "grid" => Ok(SearchEngineId::GridV1),
        "nsga2" => Ok(SearchEngineId::Nsga2V1),
        "de" => Ok(SearchEngineId::DifferentialEvolutionV1),
        _ => Err(format!("unknown engine {v}")),
    }
}
fn parse_preset(v: &str) -> Result<SearchPresetId, String> {
    match v {
        "quick" => Ok(SearchPresetId::Quick),
        "routine" => Ok(SearchPresetId::Routine),
        "accepted" => Ok(SearchPresetId::AcceptedBalanced),
        _ => Err(format!("unknown preset {v}")),
    }
}
fn usage() -> String {
    "usage: phase9 compile SOURCE.json OUT.kom9 | search STUDY ENGINE PRESET OUT [WORKERS] [--tui] | search-kom9 MANIFEST.kom9 OUT [WORKERS] [--tui] | serve STUDY ENGINE PRESET [TRANSCRIPT] | serve-kom9 MANIFEST.kom9 [TRANSCRIPT]".into()
}
