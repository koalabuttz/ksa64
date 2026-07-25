//! KSA64 Phase 9 optimization workbench CLI.
use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
use ksa64_host::phase9::{baseline_vector, built_in_manifest, evaluate_candidate, StudyId};
use ksa64_host::phase9_archive::{encode_kpf9, write_archive_atomic};
use ksa64_host::phase9_protocol::serve_jsonl;
use ksa64_host::phase9_report::{report_csv, report_html, report_json};
use ksa64_host::phase9_search::{run_search_with_workers, SearchError};
use ksa64_host::phase9_sensitivity::{encode_ksn9, one_at_a_time};
use ksa64_host::phase9_tui::run_optimization_tui;
use std::fs;
use std::io;
use std::path::PathBuf;

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
    let study = parse_study(args.get(2).map(String::as_str).unwrap_or("study-a"))?;
    let engine = parse_engine(args.get(3).map(String::as_str).unwrap_or("grid"))?;
    let preset = parse_preset(args.get(4).map(String::as_str).unwrap_or("quick"))?;
    let manifest = built_in_manifest(study, engine, preset);
    let evaluator = |d: &ksa64_core::phase9_contract::DesignVector, t: u8| {
        evaluate_candidate(&manifest, d, study, t).map_err(|_| SearchError::Evaluation)
    };
    match args[1].as_str() {
        "search" => {
            let output = PathBuf::from(
                args.get(5)
                    .cloned()
                    .unwrap_or_else(|| "phase9-output".into()),
            );
            let workers = args.get(6).and_then(|v| v.parse().ok()).unwrap_or(1usize);
            fs::create_dir_all(&output).map_err(|e| e.to_string())?;
            let axes = if engine == SearchEngineId::GridV1 {
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
            write_archive_atomic(
                &output.join("search.kra9"),
                manifest.identity,
                &result.generations,
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
            let last = result.generations.last().ok_or("empty search")?;
            let mut finalists = Vec::new();
            for f in &result.finalists {
                if let Some((i, c)) = last
                    .candidates
                    .iter()
                    .enumerate()
                    .find(|(_, c)| c.identity == f.aggregate.candidate_identity)
                {
                    finalists.push((*c, last.aggregates[i]))
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
            println!("manifest {:08x}: {} generations, {} evaluations, {} Pareto candidates, {} finalists",manifest.identity,result.generations.len(),result.evaluations,result.pareto_indices.len(),result.finalists.len());
            if args.iter().any(|a| a == "--tui") {
                run_optimization_tui(&result).map_err(|e| format!("TUI: {e:?}"))?
            }
            Ok(())
        }
        "serve" => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut transcript = Vec::new();
            serve_jsonl(
                &manifest,
                &evaluator,
                stdin.lock(),
                stdout.lock(),
                &mut transcript,
            )
            .map_err(|e| format!("protocol: {e:?}"))?;
            if let Some(path) = args.get(5) {
                fs::write(path, &transcript).map_err(|e| e.to_string())?
            }
            Ok(())
        }
        _ => Err(usage()),
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
    "usage: ksa64-phase9 <search|serve> <study-a|study-b|coupled|experimental> <grid|nsga2|de> <quick|routine|accepted> [output-or-transcript] [workers] [--tui]".into()
}
