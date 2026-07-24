use ksa64_core::phase7_format::{KPH7_HEADER_LENGTH, KPH7_POINT_LENGTH};
use ksa64_host::phase7_plot::{build_stock_kph7, parse_kph7, STOCK_KPH7_MAX_POINTS};

const TELEMETRY: &[u8] = include_bytes!("../../phase7/examples/firestorm-i211.kst7");
const PLOT: &[u8] = include_bytes!("../../phase7/examples/firestorm-i211.kph7");

#[test]
fn stock_plot_rebuilds_exactly_and_preserves_endpoints() {
    let rebuilt = build_stock_kph7(TELEMETRY).unwrap();
    assert_eq!(rebuilt.as_slice(), PLOT);
    assert_eq!(
        PLOT.len(),
        KPH7_HEADER_LENGTH + STOCK_KPH7_MAX_POINTS * KPH7_POINT_LENGTH + 4
    );
    let points = parse_kph7(PLOT).unwrap();
    assert_eq!(points.len(), STOCK_KPH7_MAX_POINTS);
    assert_eq!(points[0].time_raw, 0);
    assert_eq!(points[0].altitude_raw, 0);
    assert_eq!(points.last().unwrap().altitude_raw, 0);
    assert!(points.iter().any(|point| point.altitude_raw > 8_000_000));
}

#[test]
fn plot_corruption_and_unknown_bytes_fail_closed() {
    let mut corrupt = PLOT.to_vec();
    corrupt[500] ^= 1;
    assert!(parse_kph7(&corrupt).is_err());
    let mut reserved = PLOT.to_vec();
    reserved[60] = 1;
    ksa64_core::phase7_format::seal_phase7_record(&mut reserved).unwrap();
    assert!(parse_kph7(&reserved).is_err());
}
