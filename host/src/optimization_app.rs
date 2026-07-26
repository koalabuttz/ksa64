//! Product-facing adapters for the accepted Phase 9 and 9.5 workbenches.
//!
//! The search implementations and canonical archives remain in their accepted
//! modules.  This file only maps stable product IDs onto those services.

use crate::application::{ApplicationError, ApplicationOutcome, OptimizationRequest};
use crate::phase9::{baseline_vector, built_in_manifest, evaluate_candidate, StudyId};
use crate::phase9_5_archive::{encode_kae9, encode_kfe9, validate_kae9, validate_kfe9};
use crate::phase9_5_workbench::{built_in_advanced_manifest, run_advanced_search, AdvancedStudyId};
use crate::phase9_archive::{
    encode_kpf9, resume_search_archive_atomic, write_search_archive_atomic,
};
use crate::phase9_manifest::compile_manifest_json;
use crate::phase9_protocol::serve_jsonl;
use crate::phase9_report::{
    report_csv, report_html_with_sensitivity, report_json_with_sensitivity,
};
use crate::phase9_search::{
    run_search_with_workers, run_search_with_workers_and_progress, SearchError, SearchResult,
};
use crate::phase9_sensitivity::{encode_ksn9, one_at_a_time};
use crate::phase9_tui::run_optimization_tui;
use ksa64_core::phase9_contract::{DesignVector, SearchEngineId, SearchManifest, SearchPresetId};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;

pub fn run_product_optimization(
    request: &OptimizationRequest,
) -> Result<ApplicationOutcome, ApplicationError> {
    if request.workers == 0 {
        return Err(ApplicationError::invalid(
            "optimize.workers",
            "worker count must be nonzero",
        ));
    }
    match request.id.as_str() {
        "firestorm.design" => run_phase9_study(request, StudyId::PassiveRecovery),
        "firestorm.control" => run_phase9_study(request, StudyId::GimbalControl),
        "firestorm.effectors" => run_advanced_study(request, AdvancedStudyId::Mixed),
        _ => Err(ApplicationError::unsupported(
            "optimize.adapter",
            format!("`{}` has no optimization adapter", request.id),
        )),
    }
}

pub fn compile_optimization_manifest(
    source: &str,
    output: &Path,
) -> Result<ApplicationOutcome, ApplicationError> {
    let (_, manifest) = compile_manifest_json(source).map_err(|error| {
        ApplicationError::invalid(
            "optimize.manifest",
            format!("manifest source rejected: {error:?}"),
        )
    })?;
    let bytes = manifest.encode().map_err(|error| {
        ApplicationError::integrity("optimize.kom9", format!("KOM9 encode failed: {error:?}"))
    })?;
    write_file(output, &bytes)?;
    Ok(ApplicationOutcome::new(
        "optimize.compile",
        format!("compiled KOM9 0x{:08x}", manifest.identity),
        json!({
            "manifest_identity": format!("0x{:08x}", manifest.identity),
            "bytes": bytes.len(),
        }),
    )
    .identity(manifest.identity)
    .artifact(output))
}

pub fn run_optimization_manifest(
    manifest_path: &Path,
    output: &Path,
    workers: usize,
    tui: bool,
    resume: bool,
) -> Result<ApplicationOutcome, ApplicationError> {
    let bytes = fs::read(manifest_path).map_err(io_error("optimize.manifest-read"))?;
    let manifest = SearchManifest::parse(&bytes).map_err(|error| {
        ApplicationError::integrity("optimize.kom9", format!("KOM9 rejected: {error:?}"))
    })?;
    let study = study_from_manifest(&manifest)?;
    run_phase9_manifest(manifest, study, output, workers, tui, resume)
}

pub fn serve_product_optimizer<R: BufRead, W: Write>(
    id: &str,
    engine: SearchEngineId,
    preset: SearchPresetId,
    reader: R,
    writer: W,
    transcript: &mut Vec<u8>,
) -> Result<ApplicationOutcome, ApplicationError> {
    let study = match id {
        "firestorm.design" => StudyId::PassiveRecovery,
        "firestorm.control" => StudyId::GimbalControl,
        _ => return Err(ApplicationError::unsupported(
            "optimize.protocol",
            "the external JSONL protocol currently accepts firestorm.design or firestorm.control",
        )),
    };
    let manifest = built_in_manifest(study, engine, preset);
    let evaluator = |design: &DesignVector, tier: u8| {
        evaluate_candidate(&manifest, design, study, tier).map_err(|_| SearchError::Evaluation)
    };
    serve_jsonl(&manifest, &evaluator, reader, writer, transcript).map_err(|error| {
        ApplicationError::execution(
            "optimize.protocol",
            format!("JSONL service failed: {error:?}"),
        )
    })?;
    Ok(ApplicationOutcome::new(
        "optimize.serve",
        format!(
            "external evaluator session closed for manifest 0x{:08x}",
            manifest.identity
        ),
        json!({
            "manifest_identity": format!("0x{:08x}", manifest.identity),
            "transcript_bytes": transcript.len(),
        }),
    )
    .identity(manifest.identity))
}

fn run_phase9_study(
    request: &OptimizationRequest,
    study: StudyId,
) -> Result<ApplicationOutcome, ApplicationError> {
    let manifest = built_in_manifest(study, request.engine, request.preset);
    run_phase9_manifest(
        manifest,
        study,
        &request.output,
        request.workers,
        request.tui,
        request.resume,
    )
}

fn run_phase9_manifest(
    manifest: SearchManifest,
    study: StudyId,
    output: &Path,
    workers: usize,
    tui: bool,
    resume: bool,
) -> Result<ApplicationOutcome, ApplicationError> {
    if workers == 0 {
        return Err(ApplicationError::invalid(
            "optimize.workers",
            "worker count must be nonzero",
        ));
    }
    fs::create_dir_all(output).map_err(io_error("optimize.output"))?;
    let evaluator = |design: &DesignVector, tier: u8| {
        evaluate_candidate(&manifest, design, study, tier).map_err(|_| SearchError::Evaluation)
    };
    let axes = if manifest.engine == SearchEngineId::GridV1 {
        vec![0, 1]
    } else {
        Vec::new()
    };
    let result = if tui {
        run_search_with_workers_and_progress(
            &manifest,
            &evaluator,
            &axes,
            workers,
            |progress| {
                eprintln!(
                    "KSA64 optimize  generation {:>3}  candidates {:>4}  evaluations {:>5}  cache {:>5}",
                    progress.generation,
                    progress.candidates,
                    progress.evaluations,
                    progress.cache_hits
                );
            },
        )
    } else {
        run_search_with_workers(&manifest, &evaluator, &axes, workers)
    }
    .map_err(search_error)?;
    let manifest_path = output.join("manifest.kom9");
    let archive_path = output.join("search.kra9");
    write_file(
        &manifest_path,
        &manifest.encode().map_err(|error| {
            ApplicationError::integrity(
                "optimize.manifest",
                format!("manifest encode failed: {error:?}"),
            )
        })?,
    )?;
    if resume {
        resume_search_archive_atomic(
            &archive_path,
            manifest.identity,
            &result.generations,
            &result.evidence,
        )
    } else {
        write_search_archive_atomic(
            &archive_path,
            manifest.identity,
            &result.generations,
            &result.evidence,
        )
    }
    .map_err(|error| {
        ApplicationError::integrity(
            "optimize.archive",
            format!("search archive failed: {error:?}"),
        )
    })?;
    let sensitivity = one_at_a_time(&manifest, &baseline_vector(&manifest), &evaluator, 8)
        .map_err(search_error)?;
    let report_json_path = output.join("report.json");
    let report_csv_path = output.join("report.csv");
    let report_html_path = output.join("report.html");
    write_file(
        &report_json_path,
        &serde_json::to_vec_pretty(&report_json_with_sensitivity(
            &manifest,
            &result,
            &sensitivity,
        ))
        .map_err(json_error)?,
    )?;
    write_file(&report_csv_path, report_csv(&result).as_bytes())?;
    write_file(
        &report_html_path,
        report_html_with_sensitivity(&manifest, &result, &sensitivity).as_bytes(),
    )?;
    let finalists = finalists(&result);
    let finalist_path = output.join("finalists.kfp9");
    let finalist_bytes =
        encode_kpf9(manifest.identity, study.raw(), &finalists).map_err(|error| {
            ApplicationError::integrity(
                "optimize.finalists",
                format!("KFP9 encode failed: {error:?}"),
            )
        })?;
    write_file(&finalist_path, &finalist_bytes)?;
    let sensitivity_path = output.join("sensitivity.ksn9");
    let mut sensitivity_bytes = Vec::new();
    for record in &sensitivity {
        sensitivity_bytes.extend_from_slice(&encode_ksn9(*record));
    }
    write_file(&sensitivity_path, &sensitivity_bytes)?;
    if tui {
        run_optimization_tui(&result).map_err(|error| {
            ApplicationError::execution(
                "optimize.tui",
                format!("optimization TUI failed: {error:?}"),
            )
        })?;
    }
    Ok(ApplicationOutcome::new(
        "optimize.run",
        format!(
            "manifest 0x{:08x}: {} generations, {} evaluations, {} Pareto candidates, {} finalists",
            manifest.identity,
            result.generations.len(),
            result.evaluations,
            result.pareto_indices.len(),
            result.finalists.len()
        ),
        json!({
            "manifest_identity": format!("0x{:08x}", manifest.identity),
            "study": format!("{study:?}"),
            "engine": format!("{:?}", manifest.engine),
            "generations": result.generations.len(),
            "evaluations": result.evaluations,
            "cache_hits": result.cache_hits,
            "pareto_candidates": result.pareto_indices.len(),
            "finalists": result.finalists.len(),
            "workers": workers,
            "resumed": resume,
        }),
    )
    .identity(manifest.identity)
    .artifact(&manifest_path)
    .artifact(&archive_path)
    .artifact(&report_json_path)
    .artifact(&report_csv_path)
    .artifact(&report_html_path)
    .artifact(&finalist_path)
    .artifact(&sensitivity_path))
}

fn run_advanced_study(
    request: &OptimizationRequest,
    study: AdvancedStudyId,
) -> Result<ApplicationOutcome, ApplicationError> {
    if request.resume {
        return Err(ApplicationError::unsupported(
            "optimize.advanced-resume",
            "Phase 9.5 KAE9 studies do not define segmented resume",
        ));
    }
    if request.engine == SearchEngineId::DifferentialEvolutionV1 {
        return Err(ApplicationError::unsupported(
            "optimize.advanced-engine",
            "advanced-effector workbench supports grid or NSGA-II",
        ));
    }
    let mut manifest = built_in_advanced_manifest(study, request.engine);
    apply_advanced_preset(&mut manifest, request.preset)?;
    fs::create_dir_all(&request.output).map_err(io_error("optimize.output"))?;
    let result = run_advanced_search(&manifest, study, request.workers).map_err(search_error)?;
    let archive = encode_kae9(&result, study).map_err(|error| {
        ApplicationError::integrity("optimize.kae9", format!("KAE9 encode failed: {error:?}"))
    })?;
    validate_kae9(&archive).map_err(|error| {
        ApplicationError::integrity("optimize.kae9", format!("KAE9 rejected: {error:?}"))
    })?;
    let finalists = encode_kfe9(&result, study).map_err(|error| {
        ApplicationError::integrity("optimize.kfe9", format!("KFE9 encode failed: {error:?}"))
    })?;
    validate_kfe9(&finalists).map_err(|error| {
        ApplicationError::integrity("optimize.kfe9", format!("KFE9 rejected: {error:?}"))
    })?;
    let stem = format!(
        "{}-{}",
        advanced_study_name(study),
        engine_name(request.engine)
    );
    let archive_path = request.output.join(format!("{stem}.kae9"));
    let finalist_path = request.output.join(format!("{stem}.kfe9"));
    let report_path = request.output.join(format!("{stem}.json"));
    write_file(&archive_path, &archive)?;
    write_file(&finalist_path, &finalists)?;
    let feasible = result
        .search
        .finalists
        .iter()
        .filter(|item| item.aggregate.feasible)
        .count();
    let report = json!({
        "schema": "ksa64.phase11_5.advanced-search.v1",
        "study": advanced_study_name(study),
        "engine": engine_name(request.engine),
        "manifest_identity": format!("0x{:08x}", manifest.identity),
        "generations": result.search.generations.len(),
        "evaluations": result.search.evaluations,
        "cache_hits": result.search.cache_hits,
        "finalists": result.search.finalists.len(),
        "feasible_finalists": feasible,
        "archive_crc32": format!("0x{:08x}", ksa64_interface::crc32_ieee(&archive)),
        "workers": request.workers,
    });
    write_file(
        &report_path,
        &serde_json::to_vec_pretty(&report).map_err(json_error)?,
    )?;
    Ok(ApplicationOutcome::new(
        "optimize.run",
        format!(
            "{} {}: {} evaluations, {} feasible finalists",
            advanced_study_name(study),
            engine_name(request.engine),
            result.search.evaluations,
            feasible
        ),
        report,
    )
    .identity(manifest.identity)
    .artifact(&archive_path)
    .artifact(&finalist_path)
    .artifact(&report_path))
}

fn apply_advanced_preset(
    manifest: &mut SearchManifest,
    preset: SearchPresetId,
) -> Result<(), ApplicationError> {
    manifest.preset = preset;
    match preset {
        SearchPresetId::Quick => {
            manifest.budgets.grid_points = 5;
            manifest.budgets.population = 8;
            manifest.budgets.generations = 4;
            manifest.budgets.finalists = 4;
            manifest.budgets.max_candidates = 128;
        }
        SearchPresetId::Routine => {
            manifest.budgets.grid_points = 9;
            manifest.budgets.population = 24;
            manifest.budgets.generations = 12;
            manifest.budgets.finalists = 12;
            manifest.budgets.max_candidates = 1_024;
        }
        SearchPresetId::AcceptedBalanced => {
            manifest.budgets.grid_points = 17;
            manifest.budgets.population = 48;
            manifest.budgets.generations = 32;
            manifest.budgets.finalists = 32;
            manifest.budgets.max_candidates = 4_096;
        }
        SearchPresetId::Custom => {}
    }
    *manifest = manifest.seal().map_err(|error| {
        ApplicationError::invalid(
            "optimize.advanced-manifest",
            format!("advanced manifest invalid: {error:?}"),
        )
    })?;
    Ok(())
}

fn finalists(
    result: &SearchResult,
) -> Vec<(
    DesignVector,
    ksa64_core::phase9_contract::CandidateAggregate,
)> {
    let mut history = BTreeMap::new();
    for generation in &result.generations {
        for (candidate, aggregate) in generation.candidates.iter().zip(&generation.aggregates) {
            history.insert(candidate.identity, (*candidate, *aggregate));
        }
    }
    result
        .finalists
        .iter()
        .filter_map(|finalist| {
            history
                .get(&finalist.aggregate.candidate_identity)
                .map(|(candidate, _)| (*candidate, finalist.aggregate))
        })
        .collect()
}

pub fn study_from_manifest(manifest: &SearchManifest) -> Result<StudyId, ApplicationError> {
    match manifest.base_ids[6] {
        value if value == StudyId::PassiveRecovery.raw() => Ok(StudyId::PassiveRecovery),
        value if value == StudyId::GimbalControl.raw() => Ok(StudyId::GimbalControl),
        value if value == StudyId::Coupled.raw() => Ok(StudyId::Coupled),
        value if value == StudyId::ExperimentalAirframe.raw() => Ok(StudyId::ExperimentalAirframe),
        _ => Err(ApplicationError::invalid(
            "optimize.study",
            "unknown study identity in KOM9",
        )),
    }
}

fn advanced_study_name(study: AdvancedStudyId) -> &'static str {
    match study {
        AdvancedStudyId::Canard => "canard",
        AdvancedStudyId::Rcs => "rcs",
        AdvancedStudyId::Mixed => "mixed",
        AdvancedStudyId::Research => "research",
    }
}

fn engine_name(engine: SearchEngineId) -> &'static str {
    match engine {
        SearchEngineId::GridV1 => "grid",
        SearchEngineId::Nsga2V1 => "nsga2",
        SearchEngineId::DifferentialEvolutionV1 => "de",
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ApplicationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io_error("filesystem.create-directory"))?;
    }
    fs::write(path, bytes).map_err(io_error("filesystem.write"))
}

fn search_error(error: SearchError) -> ApplicationError {
    ApplicationError::execution("optimize.search", format!("search failed: {error:?}"))
}

fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> ApplicationError {
    move |error| {
        ApplicationError::execution("application.io", format!("{operation} failed: {error}"))
    }
}

fn json_error(error: serde_json::Error) -> ApplicationError {
    ApplicationError::execution(
        "application.json",
        format!("JSON operation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn product_ids_select_expected_phase9_studies() {
        let design = OptimizationRequest {
            id: "firestorm.design".into(),
            engine: SearchEngineId::GridV1,
            preset: SearchPresetId::Quick,
            workers: 1,
            output: PathBuf::new(),
            tui: false,
            resume: false,
        };
        assert_eq!(
            match design.id.as_str() {
                "firestorm.design" => Some(StudyId::PassiveRecovery),
                _ => None,
            },
            Some(StudyId::PassiveRecovery)
        );
    }

    #[test]
    fn advanced_presets_are_sealed_and_deterministic() {
        let mut first = built_in_advanced_manifest(AdvancedStudyId::Mixed, SearchEngineId::Nsga2V1);
        let mut second = first;
        apply_advanced_preset(&mut first, SearchPresetId::Quick).unwrap();
        apply_advanced_preset(&mut second, SearchPresetId::Quick).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.budgets.population, 8);
        assert_eq!(first.budgets.generations, 4);
    }
}
