use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use ksa64_host::phase4_export::{build_stock_report, encode_volumes};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::{DETAIL_HEADER_LENGTH, REFERENCE_RUNS};
use ksa64_sim::phase4::detail::write_kst4;
use ksa64_sim::phase4::export::ExportMode;
use ksa64_sim::telemetry::PHASE3_TELEMETRY_HEADER_LENGTH;

const KSC4: &[u8; 512] = include_bytes!("../../../phase4/examples/ksa4-reference.ksc4");
const KSR4: &[u8; 131_072] = include_bytes!("../../../phase4/examples/ksa4-reference.ksr4");
const KPH4: &[u8; 1_872] = include_bytes!("../../../phase4/examples/ksa4-baseline.kph4");
const KST3: &[u8] = include_bytes!("../../../phase3/examples/ksa3-nominal.kst3");

fn run() -> Result<(), String> {
    let output = PathBuf::from(env::args().nth(1).ok_or("missing output directory")?);
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let (report, manifest) =
        build_stock_report(KSC4, KSR4, KPH4).map_err(|error| format!("report: {error:?}"))?;
    let report_crc = crc32_ieee(&report);
    let volumes = encode_volumes(
        &report,
        report_crc,
        manifest.selection_crc32(),
        manifest.mode,
        manifest.volume_payload_limit,
    )
    .map_err(|error| format!("stock volume: {error:?}"))?;
    fs::write(output.join("ksa4-stock-report.kra4"), &report).map_err(|error| error.to_string())?;
    fs::write(output.join("ksa4-stock-report.kxv4"), &volumes[0])
        .map_err(|error| error.to_string())?;

    let run = derive_run(&reviewed_campaign_config(REFERENCE_RUNS), 0)
        .map_err(|error| format!("baseline run: {error:?}"))?;
    let frames = &KST3[PHASE3_TELEMETRY_HEADER_LENGTH..];
    let mut detail = vec![0u8; DETAIL_HEADER_LENGTH + frames.len()];
    write_kst4(
        0xa2e9_e9d5,
        run.index,
        run.sensor_seed,
        run.variation.checksum(),
        frames,
        &mut detail,
    )
    .map_err(|error| format!("baseline KST4: {error:?}"))?;
    fs::write(output.join("ksa4-baseline.kst4"), &detail).map_err(|error| error.to_string())?;

    let synthetic: Vec<u8> = (0..3_000).map(|index| (index * 37) as u8).collect();
    let synthetic_crc = crc32_ieee(&synthetic);
    let synthetic_volumes = encode_volumes(
        &synthetic,
        synthetic_crc,
        0x5359_4e34,
        ExportMode::MultiVolume,
        1_000,
    )
    .map_err(|error| format!("synthetic volumes: {error:?}"))?;
    for (index, volume) in synthetic_volumes.iter().enumerate() {
        fs::write(
            output.join(format!("ksa4-synthetic-{:02}.kxv4", index + 1)),
            volume,
        )
        .map_err(|error| error.to_string())?;
    }
    println!(
        "report_bytes={} report_crc=0x{report_crc:08x} selection=0x{:08x} detail_bytes={} detail_crc=0x{:08x} synthetic_volumes={}",
        report.len(),
        manifest.selection_crc32(),
        detail.len(),
        crc32_ieee(&detail),
        synthetic_volumes.len()
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
