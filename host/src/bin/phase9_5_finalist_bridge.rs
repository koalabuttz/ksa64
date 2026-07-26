use ksa64_core::phase9_contract::{SearchEngineId, SearchManifest};
use ksa64_host::phase9_5_archive::AdvancedFinalistPackage;
use ksa64_host::phase9_5_link::run_host_external_finalist_with_limit_observed;
use ksa64_host::phase9_5_workbench::{built_in_advanced_manifest, AdvancedStudyId};
use std::fs;
use std::io;
use std::net::TcpListener;

fn study(raw: u32) -> Result<AdvancedStudyId, io::Error> {
    [
        AdvancedStudyId::Canard,
        AdvancedStudyId::Rcs,
        AdvancedStudyId::Mixed,
        AdvancedStudyId::Research,
    ]
    .into_iter()
    .find(|value| value.raw() == raw)
    .ok_or_else(|| io::Error::other(format!("unknown study identity {raw:08x}")))
}
fn manifest(identity: u32, study: AdvancedStudyId) -> Result<SearchManifest, io::Error> {
    [SearchEngineId::GridV1, SearchEngineId::Nsga2V1]
        .into_iter()
        .map(|engine| built_in_advanced_manifest(study, engine))
        .find(|value| value.identity == identity)
        .ok_or_else(|| io::Error::other(format!("unknown manifest identity {identity:08x}")))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = "127.0.0.1:6512".to_owned();
    let mut package_path = None;
    let mut selected = 0usize;
    let mut max_releases = 8u32;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--listen" => {
                listen = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing address"))?
            }
            "--package" => package_path = args.next(),
            "--index" => {
                selected = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing finalist index"))?
                    .parse()?;
            }
            "--max-releases" => {
                max_releases = args
                    .next()
                    .ok_or_else(|| io::Error::other("missing release count"))?
                    .parse()?;
            }
            "--pace" => {
                if args.next().as_deref() != Some("externally-paced") {
                    return Err(io::Error::other("only externally-paced is supported").into());
                }
            }
            _ => return Err(io::Error::other(format!("unknown option {flag}")).into()),
        }
    }
    if max_releases == 0 {
        return Err(io::Error::other("max releases must be positive").into());
    }
    let package_path = package_path.ok_or_else(|| io::Error::other("--package is required"))?;
    let bytes = fs::read(&package_path)?;
    let package = AdvancedFinalistPackage::parse(&bytes)
        .map_err(|error| io::Error::other(format!("KFE9: {error:?}")))?;
    let record = package
        .record(selected)
        .map_err(|error| io::Error::other(format!("finalist: {error:?}")))?;
    let study = study(package.study_identity)?;
    let manifest = manifest(package.manifest_identity, study)?;
    let listener = TcpListener::bind(&listen)?;
    let (mut stream, _) = listener.accept()?;
    stream.set_nodelay(true)?;
    let evidence = run_host_external_finalist_with_limit_observed(
        &mut stream,
        &manifest,
        &record.design,
        study,
        max_releases,
        None,
    )
    .map_err(|error| io::Error::other(format!("split: {error:?}")))?;
    println!(
        "KSA64_PHASE95_FINALIST_BOUNDED study={study:?} candidate={:08x} releases={} sensor={:08x} command={:08x} status={:08x} truth={:08x} nav={:08x} flight={:08x} allocator={:08x}",
        record.design.identity,
        evidence.releases,
        evidence.sensor_checksum,
        evidence.command_checksum,
        evidence.status_checksum,
        evidence.truth_checksum,
        evidence.navigation_checksum,
        evidence.flight_checksum,
        evidence.allocator_checksum,
    );
    Ok(())
}
