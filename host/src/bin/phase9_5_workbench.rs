use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
use ksa64_host::phase9_5_archive::{encode_kae9, encode_kfe9, validate_kae9, validate_kfe9};
use ksa64_host::phase9_5_workbench::{
    built_in_advanced_manifest, run_advanced_campaign, run_advanced_search, AdvancedStudyId,
};
use serde::Serialize;
use std::{env, fs, path::PathBuf, time::Instant};
#[derive(Serialize)]
struct Row {
    study: String,
    engine: String,
    manifest: String,
    generations: usize,
    evaluations: u32,
    cache_hits: u32,
    finalists: usize,
    feasible_finalists: usize,
    archive_bytes: usize,
    archive_crc32: String,
    finalist_bytes: usize,
    seconds: f64,
}
fn study_name(s: AdvancedStudyId) -> &'static str {
    match s {
        AdvancedStudyId::Canard => "canard",
        AdvancedStudyId::Rcs => "rcs",
        AdvancedStudyId::Mixed => "mixed",
        AdvancedStudyId::Research => "research",
    }
}
fn engine_name(e: SearchEngineId) -> &'static str {
    match e {
        SearchEngineId::GridV1 => "grid",
        SearchEngineId::Nsga2V1 => "nsga2",
        SearchEngineId::DifferentialEvolutionV1 => "de",
    }
}
fn main() {
    let args: Vec<String> = env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("phase9_5/evidence/workbench"),
    );
    let workers = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(8usize);
    let smoke = args.iter().any(|x| x == "--smoke");
    fs::create_dir_all(&out).unwrap();
    let campaign_start = Instant::now();
    let c1 = run_advanced_campaign(AdvancedStudyId::Mixed, 1).unwrap();
    let c4 = run_advanced_campaign(AdvancedStudyId::Mixed, 4).unwrap();
    let c8 = run_advanced_campaign(AdvancedStudyId::Mixed, 8).unwrap();
    assert_eq!(c1, c4);
    assert_eq!(c1, c8);
    let mut cb = Vec::with_capacity(c1.config.len() + c1.records.len() * 512);
    cb.extend_from_slice(&c1.config);
    for r in &c1.records {
        cb.extend_from_slice(r)
    }
    fs::write(out.join("mixed-64-campaign.ksc9-kas9"), &cb).unwrap();
    let specs = [
        (AdvancedStudyId::Canard, SearchEngineId::GridV1),
        (AdvancedStudyId::Canard, SearchEngineId::Nsga2V1),
        (AdvancedStudyId::Rcs, SearchEngineId::GridV1),
        (AdvancedStudyId::Rcs, SearchEngineId::Nsga2V1),
        (AdvancedStudyId::Mixed, SearchEngineId::GridV1),
        (AdvancedStudyId::Mixed, SearchEngineId::Nsga2V1),
        (AdvancedStudyId::Research, SearchEngineId::Nsga2V1),
    ];
    let mut rows = Vec::new();
    for (study, engine) in specs {
        let mut manifest = built_in_advanced_manifest(study, engine);
        if smoke {
            manifest.preset = SearchPresetId::Custom;
            manifest.budgets.grid_points = 2;
            manifest.budgets.population = 4;
            manifest.budgets.generations = 1;
            manifest.budgets.finalists = if study.experimental() { 0 } else { 1 };
            manifest.budgets.max_candidates = 16;
            manifest = manifest.seal().unwrap()
        }
        eprintln!(
            "starting {} {} 0x{:08x}",
            study_name(study),
            engine_name(engine),
            manifest.identity
        );
        let started = Instant::now();
        let result = run_advanced_search(&manifest, study, workers).unwrap();
        let archive = encode_kae9(&result, study).unwrap();
        validate_kae9(&archive).unwrap();
        let finalist = encode_kfe9(&result, study).unwrap();
        validate_kfe9(&finalist).unwrap();
        let stem = format!("{}-{}", study_name(study), engine_name(engine));
        fs::write(out.join(format!("{stem}.kae9")), &archive).unwrap();
        fs::write(out.join(format!("{stem}.kfe9")), &finalist).unwrap();
        let feasible = result
            .search
            .finalists
            .iter()
            .filter(|x| x.aggregate.feasible)
            .count();
        rows.push(Row {
            study: study_name(study).into(),
            engine: engine_name(engine).into(),
            manifest: format!("0x{:08x}", manifest.identity),
            generations: result.search.generations.len(),
            evaluations: result.search.evaluations,
            cache_hits: result.search.cache_hits,
            finalists: result.search.finalists.len(),
            feasible_finalists: feasible,
            archive_bytes: archive.len(),
            archive_crc32: format!("0x{:08x}", ksa64_interface::crc32_ieee(&archive)),
            finalist_bytes: finalist.len(),
            seconds: started.elapsed().as_secs_f64(),
        });
    }
    let report = serde_json::json!({"schema":"ksa64-phase9.5-workbench-evidence-v1","seed":"0x4b534195","workers":workers,"smoke":smoke,"campaign_crc32":format!("0x{:08x}",c1.crc32),"campaign_seconds":campaign_start.elapsed().as_secs_f64(),"studies":rows});
    fs::write(
        out.join(if smoke {
            "smoke-report.json"
        } else {
            "accepted-report.json"
        }),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("wrote Phase 9.5 workbench evidence to {}", out.display())
}
