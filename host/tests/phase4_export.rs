use ksa64_host::phase4_export::{
    build_selected_report, build_stock_report, encode_volumes, join_volumes,
    stock_aggregate_payload, validate_joined_archive, HistoryRecord, ReportSources,
};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::{DETAIL_HEADER_LENGTH, REFERENCE_RUNS};
use ksa64_sim::phase4::detail::write_kst4;
use ksa64_sim::phase4::export::{
    parse_kxv4, ExportError, ExportManifest, ExportMode, SummaryRange,
};
use ksa64_sim::telemetry::PHASE3_TELEMETRY_HEADER_LENGTH;

const KSC4: &[u8; 512] = include_bytes!("../../phase4/examples/ksa4-reference.ksc4");
const KSR4: &[u8; 131_072] = include_bytes!("../../phase4/examples/ksa4-reference.ksr4");
const KPH4: &[u8; 1_872] = include_bytes!("../../phase4/examples/ksa4-baseline.kph4");
const KST3: &[u8] = include_bytes!("../../phase3/examples/ksa3-nominal.kst3");

#[test]
fn default_stock_report_is_one_strict_volume() {
    let (mut report, manifest) = build_stock_report(KSC4, KSR4, KPH4).unwrap();
    validate_joined_archive(&mut report).unwrap();
    let archive_crc = crc32_ieee(&report);
    let volumes = encode_volumes(
        &report,
        archive_crc,
        manifest.selection_crc32(),
        manifest.mode,
        manifest.volume_payload_limit,
    )
    .unwrap();
    assert_eq!(volumes.len(), 1);
    let parsed = parse_kxv4(&volumes[0]).unwrap();
    assert_eq!(parsed.payload, report);
    assert_eq!(join_volumes(&volumes).unwrap(), report);
}

#[test]
fn synthetic_three_volume_join_rejects_every_order_and_identity_failure() {
    let logical: Vec<u8> = (0..3_000).map(|index| (index * 37) as u8).collect();
    let archive_crc = crc32_ieee(&logical);
    let volumes = encode_volumes(
        &logical,
        archive_crc,
        0x1234_5678,
        ExportMode::MultiVolume,
        1_000,
    )
    .unwrap();
    assert_eq!(volumes.len(), 3);
    assert_eq!(join_volumes(&volumes).unwrap(), logical);
    assert_eq!(join_volumes(&volumes[..2]), Err(ExportError::Order));
    let reordered = vec![volumes[1].clone(), volumes[0].clone(), volumes[2].clone()];
    assert_eq!(join_volumes(&reordered), Err(ExportError::Order));
    let duplicated = vec![volumes[0].clone(), volumes[0].clone(), volumes[2].clone()];
    assert_eq!(join_volumes(&duplicated), Err(ExportError::Order));
    let mut corrupt = volumes.clone();
    corrupt[1][64] ^= 0x80;
    assert_eq!(join_volumes(&corrupt), Err(ExportError::Checksum));
    let mut mismatch = volumes.clone();
    mismatch[1][16] ^= 1;
    assert!(join_volumes(&mismatch).is_err());
}

#[test]
fn oversized_one_volume_is_rejected_before_output() {
    let logical = vec![0u8; 1_001];
    assert_eq!(
        encode_volumes(
            &logical,
            crc32_ieee(&logical),
            1,
            ExportMode::OneVolume,
            1_000
        ),
        Err(ExportError::Oversized)
    );
}

#[test]
fn configurable_report_selects_summary_compact_and_full_history() {
    let run = derive_run(&reviewed_campaign_config(REFERENCE_RUNS), 0).unwrap();
    let frames = &KST3[PHASE3_TELEMETRY_HEADER_LENGTH..];
    let mut kst4 = vec![0u8; DETAIL_HEADER_LENGTH + frames.len()];
    write_kst4(
        0xa2e9_e9d5,
        run.index,
        run.sensor_seed,
        run.variation.checksum(),
        frames,
        &mut kst4,
    )
    .unwrap();
    let mut ranges = [SummaryRange::default(); 8];
    ranges[0] = SummaryRange { start: 0, count: 1 };
    let mut compact_runs = [0u32; 16];
    compact_runs[0] = 0;
    let mut full_runs = [0u32; 16];
    full_runs[0] = 0;
    let manifest = ExportManifest {
        include_config: true,
        include_aggregate: true,
        mode: ExportMode::OneVolume,
        volume_payload_limit: 160 * 1_024,
        summary_range_count: 1,
        summary_ranges: ranges,
        compact_count: 1,
        compact_runs,
        full_count: 1,
        full_runs,
    };
    let compact = [HistoryRecord {
        run_index: 0,
        bytes: KPH4,
    }];
    let full = [HistoryRecord {
        run_index: 0,
        bytes: &kst4,
    }];
    let aggregate = stock_aggregate_payload();
    let sources = ReportSources {
        campaign_crc32: 0xa2e9_e9d5,
        run_count: REFERENCE_RUNS,
        ksc4: KSC4,
        aggregate: &aggregate,
        ksr4: KSR4,
        compact: &compact,
        full: &full,
    };
    let mut report = build_selected_report(&manifest, &sources).unwrap();
    assert!(report.len() < 160 * 1_024);
    validate_joined_archive(&mut report).unwrap();
}
