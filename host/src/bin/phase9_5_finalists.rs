use ksa64_core::phase9_contract::{SearchEngineId, SearchManifest};
use ksa64_host::phase9_5_archive::{plan_advanced_retention, subset_kfe9, AdvancedFinalistPackage};
use ksa64_host::phase9_5_workbench::{
    built_in_advanced_manifest, evaluate_advanced_candidate, AdvancedStudyId,
};
use serde_json::json;
use std::fs;
use std::io;

fn study(raw: u32) -> Result<AdvancedStudyId, io::Error> {
    for value in [
        AdvancedStudyId::Canard,
        AdvancedStudyId::Rcs,
        AdvancedStudyId::Mixed,
        AdvancedStudyId::Research,
    ] {
        if value.raw() == raw {
            return Ok(value);
        }
    }
    Err(io::Error::other(format!(
        "unknown study identity {raw:08x}"
    )))
}
fn manifest(identity: u32, study: AdvancedStudyId) -> Result<SearchManifest, io::Error> {
    for engine in [SearchEngineId::GridV1, SearchEngineId::Nsga2V1] {
        let m = built_in_advanced_manifest(study, engine);
        if m.identity == identity {
            return Ok(m);
        }
    }
    Err(io::Error::other(format!(
        "unknown manifest identity {identity:08x}"
    )))
}
fn parse_indices(value: &str) -> Result<Vec<usize>, io::Error> {
    value
        .split(',')
        .map(|x| {
            x.parse::<usize>()
                .map_err(|_| io::Error::other("invalid subset index"))
        })
        .collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = None;
    let mut selected = 0usize;
    let mut rerun = false;
    let mut reu_kib = 0usize;
    let mut subset = None;
    let mut output = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--package" => {
                path = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("missing package"))?,
                )
            }
            "--index" => {
                selected = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing index"))?
                    .parse()?
            }
            "--rerun" => rerun = true,
            "--reu-kib" => {
                reu_kib = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing REU KiB"))?
                    .parse()?
            }
            "--subset" => {
                subset = Some(parse_indices(
                    &args
                        .next()
                        .ok_or_else(|| io::Error::other("missing indices"))?,
                )?)
            }
            "--output" => {
                output = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("missing output"))?,
                )
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    let path = path.ok_or_else(|| io::Error::other("--package is required"))?;
    let bytes = fs::read(&path)?;
    let package = AdvancedFinalistPackage::parse(&bytes)
        .map_err(|e| io::Error::other(format!("KFE9: {e:?}")))?;
    if let Some(indices) = subset {
        let out = subset_kfe9(&bytes, &indices)
            .map_err(|e| io::Error::other(format!("subset: {e:?}")))?;
        let target = output.ok_or_else(|| io::Error::other("--output required with --subset"))?;
        fs::write(target, &out)?;
    }
    if selected >= package.count as usize {
        return Err(io::Error::other("selected index outside package").into());
    }
    let record = package
        .record(selected)
        .map_err(|e| io::Error::other(format!("record: {e:?}")))?;
    let study = study(package.study_identity)?;
    let plan = plan_advanced_retention(reu_kib, package.count as usize, 640_128, 4_160);
    let rerun_result = if rerun {
        let m = manifest(package.manifest_identity, study)?;
        let evaluated = evaluate_advanced_candidate(
            &m,
            &record.design,
            study,
            record.aggregate.uncertainty_tier,
        )
        .map_err(|e| io::Error::other(format!("rerun: {e:?}")))?;
        if evaluated.aggregate != record.aggregate
            || evaluated.cases.first().map(|c| c.kas9) != Some(record.summary_to_bytes())
        {
            return Err(io::Error::other("rerun evidence mismatch").into());
        }
        Some(
            json!({"tier":evaluated.aggregate.uncertainty_tier,"case_crc":format!("0x{:08x}",evaluated.aggregate.case_crc),"exact":true}),
        )
    } else {
        None
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"ksa64.phase9_5.finalist-browser-v1","package":path,"manifest":format!("0x{:08x}",package.manifest_identity),"study":format!("{:?}",study),"count":package.count,"selected":{"index":selected,"candidate":format!("0x{:08x}",record.design.identity),"aggregate":format!("0x{:08x}",record.aggregate.identity),"feasible":record.aggregate.feasible,"objectives":record.aggregate.objectives[..record.aggregate.objective_count as usize].to_vec(),"constraints":record.aggregate.constraint_values[..record.aggregate.constraint_count as usize].to_vec(),"effector":format!("0x{:08x}",record.summary.effector_identity),"allocator":format!("0x{:08x}",record.summary.allocator_identity)},"retention":{"reu_kib":reu_kib,"summaries":plan.retained_summaries,"full_histories":plan.full_histories,"compact_histories":plan.compact_histories,"unused_bytes":plan.unused_bytes},"rerun":rerun_result})
        )?
    );
    Ok(())
}
trait SummaryBytes {
    fn summary_to_bytes(&self) -> [u8; ksa64_core::phase9_5_contract::KAS9_LENGTH];
}
impl SummaryBytes for ksa64_host::phase9_5_archive::AdvancedFinalistRecord {
    fn summary_to_bytes(&self) -> [u8; ksa64_core::phase9_5_contract::KAS9_LENGTH] {
        let mut out = [0; ksa64_core::phase9_5_contract::KAS9_LENGTH];
        ksa64_core::phase9_5_contract::write_advanced_effector_summary(self.summary, &mut out)
            .expect("parsed summary re-encodes");
        out
    }
}
