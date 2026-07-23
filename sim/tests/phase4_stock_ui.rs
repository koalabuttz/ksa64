use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::contracts::{REFERENCE_RUNS, RUN_SUMMARY_LENGTH};
use ksa64_sim::phase4::plot::parse_kph4;
use ksa64_sim::phase4::stock::StockRetention;
use ksa64_sim::phase4::stock_ui::{
    render_stock_page, StockPage, StockUiData, REFERENCE_STOCK_UI, SCREEN_BYTES,
};
use ksa64_sim::phase4::summary::parse_ksr4;

const REFERENCE_KSR4: &[u8; 131_072] = include_bytes!("../../phase4/examples/ksa4-reference.ksr4");
const BASELINE_KPH4: &[u8; 1_872] = include_bytes!("../../phase4/examples/ksa4-baseline.kph4");

#[test]
fn frozen_stock_display_data_matches_streaming_evidence() {
    let mut retention = StockRetention::new();
    for index in 0..REFERENCE_RUNS as usize {
        let offset = index * RUN_SUMMARY_LENGTH;
        retention
            .observe(parse_ksr4(&REFERENCE_KSR4[offset..offset + RUN_SUMMARY_LENGTH]).unwrap())
            .unwrap();
    }
    let snapshot = retention.finish().unwrap();
    let data = StockUiData::from_snapshot(
        0xa2e9_e9d5,
        &snapshot,
        BASELINE_KPH4.len() as u16,
        crc32_ieee(BASELINE_KPH4),
    );
    assert_eq!(data, REFERENCE_STOCK_UI);
}

#[test]
fn all_stock_pages_render_inside_fixed_screen() {
    let plot = parse_kph4(BASELINE_KPH4).unwrap();
    for (page, marker) in [
        (StockPage::Campaign, b"CAMPAIGN".as_slice()),
        (StockPage::Histogram, b"OUTCOME HISTOGRAM".as_slice()),
        (StockPage::Trajectory, b"BASELINE TRAJECTORY".as_slice()),
        (StockPage::Storage, b"STORAGE".as_slice()),
    ] {
        let mut screen = [0u8; SCREEN_BYTES];
        render_stock_page(page, &REFERENCE_STOCK_UI, &plot, &mut screen);
        assert!(screen.windows(marker.len()).any(|window| window == marker));
        assert!(screen.iter().all(|byte| byte.is_ascii()));
        assert_eq!(&screen[24 * 40..24 * 40 + 11], b"F1 CAMPAIGN");
    }
}
