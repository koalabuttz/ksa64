use ksa64_core::phase5_contract::PHASE5_CAMPAIGN_SEED;
use ksa64_sim::phase4::storage::{ReuPreference, StorageMode, SUPPORTED_REU_KIB};
use ksa64_sim::phase5_campaign::{parse_ksr5, Phase5RunSummary, KSR5_LENGTH};
use ksa64_sim::phase5_history::{
    parse_kph5_point, select_phase5_histories, validate_kph5, write_kph5, Phase5HistoryHeader,
    Phase5HistoryRecorder, Phase5StockRetention, Phase5StoragePlan, KPH5_HEADER_LENGTH,
    KPH5_POINT_LENGTH, STOCK_HISTORY_POINTS, STOCK_HISTORY_STRIDE,
};
use ksa64_sim::phase5_mission::{
    run_phase5_mission, run_phase5_mission_observed, Phase5MissionCase,
};

const REFERENCE_KSR5: &[u8; 40_960] = include_bytes!("../../phase5/examples/ksa5-reference.ksr5");

fn summaries() -> Vec<Phase5RunSummary> {
    REFERENCE_KSR5
        .chunks_exact(KSR5_LENGTH)
        .map(|bytes| parse_ksr5(bytes).unwrap())
        .collect()
}

#[test]
fn stock_history_is_observational_and_strict() {
    let expected = run_phase5_mission(Phase5MissionCase::Nominal).unwrap();
    let mut recorder = Phase5HistoryRecorder::<STOCK_HISTORY_POINTS>::new(STOCK_HISTORY_STRIDE);
    let got = run_phase5_mission_observed(Phase5MissionCase::Nominal, &mut recorder).unwrap();
    assert_eq!(got, expected);
    assert_eq!(recorder.count(), 99);
    assert_eq!(recorder.points()[0].step, 0);
    assert_eq!(recorder.points().last().unwrap().step as u32, got.steps);

    let mut bytes = vec![0u8; KPH5_HEADER_LENGTH + recorder.count() * KPH5_POINT_LENGTH];
    let header = Phase5HistoryHeader {
        campaign_seed: PHASE5_CAMPAIGN_SEED,
        run_index: 0,
        sensor_seed: 0x5a00_0000,
        variation_checksum: 0,
        stride: STOCK_HISTORY_STRIDE,
        point_count: 0,
        terminal_step: got.steps,
        points_crc32: 0,
    };
    write_kph5(header, recorder.points(), &mut bytes).unwrap();
    let parsed = validate_kph5(&bytes).unwrap();
    assert_eq!(parsed.point_count, recorder.count() as u16);
    assert_eq!(
        parse_kph5_point(&bytes[KPH5_HEADER_LENGTH..KPH5_HEADER_LENGTH + KPH5_POINT_LENGTH])
            .unwrap(),
        recorder.points()[0]
    );
    bytes[KPH5_HEADER_LENGTH + 3] ^= 0x80;
    assert!(validate_kph5(&bytes).is_err());
}

#[test]
fn stock_retention_and_detail_selection_are_frozen() {
    let summaries = summaries();
    let mut stock = Phase5StockRetention::new();
    for &summary in &summaries {
        stock.observe(summary).unwrap();
    }
    let snapshot = stock.finish().unwrap();
    assert_eq!(snapshot.aggregate.runs, 256);
    let retained = snapshot.retained.map(|s| s.run_index);
    println!("retained={retained:?}");
    assert_eq!(retained[0], 0);
    assert_eq!(retained.len(), 5);

    let mut selected = [u32::MAX; 12];
    assert_eq!(
        select_phase5_histories(&summaries, &mut selected).unwrap(),
        selected.len()
    );
    println!("selected={selected:?}");
    assert_eq!(selected[0], 0);
    assert_eq!(
        selected
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        selected.len()
    );
}

#[test]
fn storage_plan_scales_from_stock_through_every_reu_tier() {
    let stock = Phase5StoragePlan::compute(0, ReuPreference::Auto, 256, 3134, 99).unwrap();
    assert_eq!(stock.mode, StorageMode::Stock);
    assert_eq!(stock.used_bytes, 2_464);
    assert_eq!(
        (
            stock.summary_slots,
            stock.full_histories,
            stock.compact_histories
        ),
        (5, 0, 1)
    );
    let mut previous_summaries = 0;
    let mut previous_full = 0;
    for capacity in SUPPORTED_REU_KIB {
        let plan =
            Phase5StoragePlan::compute(capacity, ReuPreference::Auto, 256, 3134, 393).unwrap();
        println!("{capacity} KiB => {plan:?}");
        assert!(plan.summary_slots >= previous_summaries);
        assert!(plan.full_histories >= previous_full);
        assert!(plan.used_bytes <= capacity * 1024);
        previous_summaries = plan.summary_slots;
        previous_full = plan.full_histories;
    }
    let disabled =
        Phase5StoragePlan::compute(16_384, ReuPreference::Disabled, 256, 3134, 99).unwrap();
    assert_eq!(disabled.mode, StorageMode::Stock);
    let capped =
        Phase5StoragePlan::compute(16_384, ReuPreference::CapKiB(512), 256, 3134, 393).unwrap();
    let direct = Phase5StoragePlan::compute(512, ReuPreference::Auto, 256, 3134, 393).unwrap();
    assert_eq!(capped.effective_kib, direct.effective_kib);
    assert_eq!(capped.summary_slots, direct.summary_slots);
    assert_eq!(capped.full_histories, direct.full_histories);
    assert_eq!(capped.compact_histories, direct.compact_histories);
}
use ksa64_sim::phase5_archive::{
    scan_archive, ArchiveError, ArchiveStorage, ArchiveStorageError, ArchiveWriter, SliceStorage,
};

#[test]
fn kra5_archive_commits_and_rejects_corruption() {
    let mut bytes = vec![0xa5; 8192];
    {
        let storage = SliceStorage::new(&mut bytes);
        let mut writer = ArchiveWriter::create(storage, 0x5885_1234).unwrap();
        writer.append(1, u32::MAX, b"KSC5 metadata").unwrap();
        writer.append(2, 0, &[0x55; 160]).unwrap();
        writer.append(3, 0, &[0x66; 1664]).unwrap();
        writer.finish().unwrap();
        let mut storage = writer.into_storage();
        let scan = scan_archive(&mut storage).unwrap();
        assert!(scan.complete);
        assert_eq!(scan.valid_records, 4);
    }
    bytes[256 + 32 + 2] ^= 0x40;
    let mut corrupt = SliceStorage::new(&mut bytes);
    assert_eq!(scan_archive(&mut corrupt), Err(ArchiveError::Checksum));
}

struct FailingStorage {
    bytes: Vec<u8>,
    writes: usize,
    fail_on: usize,
}
impl ArchiveStorage for FailingStorage {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ArchiveStorageError> {
        let p = offset as usize;
        out.copy_from_slice(
            self.bytes
                .get(p..p + out.len())
                .ok_or(ArchiveStorageError)?,
        );
        Ok(())
    }
    fn write(&mut self, offset: u32, input: &[u8]) -> Result<(), ArchiveStorageError> {
        self.writes += 1;
        if self.writes == self.fail_on {
            return Err(ArchiveStorageError);
        }
        let p = offset as usize;
        self.bytes
            .get_mut(p..p + input.len())
            .ok_or(ArchiveStorageError)?
            .copy_from_slice(input);
        Ok(())
    }
}
#[test]
fn kra5_failed_commit_preserves_prior_prefix() {
    let storage = FailingStorage {
        bytes: vec![0; 4096],
        writes: 0,
        fail_on: 4,
    };
    let mut writer = ArchiveWriter::create(storage, 0x12345678).unwrap();
    assert_eq!(
        writer.append(3, 7, &[0x77; 200]),
        Err(ArchiveError::Storage)
    );
    let mut storage = writer.into_storage();
    storage.fail_on = usize::MAX;
    let scan = scan_archive(&mut storage).unwrap();
    assert!(!scan.complete);
    assert_eq!(scan.valid_records, 0);
    assert_eq!(scan.valid_bytes, 256);
}
