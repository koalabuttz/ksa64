use ksa64_sim::phase4::contracts::{REFERENCE_RUNS, RUN_SUMMARY_LENGTH};
use ksa64_sim::phase4::stock::{StockError, StockRetention};
use ksa64_sim::phase4::summary::parse_ksr4;

const REFERENCE_KSR4: &[u8; 131_072] = include_bytes!("../../phase4/examples/ksa4-reference.ksr4");

#[test]
fn stock_retention_is_ordered_bounded_and_deterministic() {
    let mut retention = StockRetention::new();
    for index in 0..REFERENCE_RUNS as usize {
        let offset = index * RUN_SUMMARY_LENGTH;
        let summary = parse_ksr4(&REFERENCE_KSR4[offset..offset + RUN_SUMMARY_LENGTH]).unwrap();
        retention.observe(summary).unwrap();
    }
    let snapshot = retention.finish().unwrap();
    assert_eq!(snapshot.aggregate.run_count, REFERENCE_RUNS);
    assert_eq!(snapshot.aggregate.outcome_counts, [857, 166, 1, 0, 0, 0]);
    assert_eq!(snapshot.aggregate.summary_chain, 0x813c_e420);
    let runs = snapshot.retained.map(|summary| summary.run_index);
    println!("retained={runs:?}");
    println!("aggregate={:?}", snapshot.aggregate);
    assert_eq!(runs, [0, 8, 96, 796, 1]);
    assert_eq!(runs.len(), 5);
    for left in 0..runs.len() {
        for right in left + 1..runs.len() {
            assert_ne!(runs[left], runs[right]);
        }
    }
}

#[test]
fn stock_retention_rejects_reordering() {
    let first = parse_ksr4(&REFERENCE_KSR4[..RUN_SUMMARY_LENGTH]).unwrap();
    let second = parse_ksr4(&REFERENCE_KSR4[RUN_SUMMARY_LENGTH..2 * RUN_SUMMARY_LENGTH]).unwrap();
    let mut retention = StockRetention::new();
    assert_eq!(retention.observe(second), Err(StockError::RunOrder));
    retention.observe(first).unwrap();
    assert_eq!(retention.observe(first), Err(StockError::RunOrder));
}
