use ksa64_sim::phase4::archive::{
    scan_archive, ArchiveError, ArchiveStorage, ArchiveWriter, SliceStorage,
};
use ksa64_sim::phase4::contracts::{REFERENCE_RUNS, RUN_SUMMARY_LENGTH};
use ksa64_sim::phase4::storage::{
    select_detailed_runs, ReuPreference, StorageMode, StoragePlan, SUPPORTED_REU_KIB,
};
use ksa64_sim::phase4::summary::parse_ksr4;

const REFERENCE_KSR4: &[u8; 131_072] = include_bytes!("../../phase4/examples/ksa4-reference.ksr4");

#[test]
fn storage_plan_scales_across_stock_and_every_reu_tier() {
    let stock = StoragePlan::compute(0, ReuPreference::Auto, 1_024, 906, 226).unwrap();
    assert_eq!(stock.mode, StorageMode::Stock);
    assert_eq!(stock.summary_slots, 5);
    assert_eq!(stock.full_histories, 0);
    assert_eq!(stock.compact_histories, 1);

    let mut previous_summaries = 0;
    let mut previous_full = 0;
    for capacity in SUPPORTED_REU_KIB {
        let plan = StoragePlan::compute(capacity, ReuPreference::Auto, 1_024, 906, 901).unwrap();
        println!("{capacity} KiB => {plan:?}");
        assert_eq!(plan.mode, StorageMode::Reu);
        assert!(plan.summary_slots >= previous_summaries);
        assert!(plan.full_histories >= previous_full);
        assert!(plan.used_bytes <= capacity * 1_024);
        previous_summaries = plan.summary_slots;
        previous_full = plan.full_histories;
    }
    let capped = StoragePlan::compute(16_384, ReuPreference::CapKiB(512), 1_024, 906, 901).unwrap();
    assert_eq!(capped.effective_kib, 512);
    let direct = StoragePlan::compute(512, ReuPreference::Auto, 1_024, 906, 901).unwrap();
    assert_eq!(capped.effective_kib, direct.effective_kib);
    assert_eq!(capped.summary_slots, direct.summary_slots);
    assert_eq!(capped.full_histories, direct.full_histories);
    assert_eq!(capped.compact_histories, direct.compact_histories);
    let disabled = StoragePlan::compute(16_384, ReuPreference::Disabled, 1_024, 906, 226).unwrap();
    assert_eq!(disabled.mode, StorageMode::Stock);
}

#[test]
fn kra4_append_scan_and_corruption_are_strict() {
    let mut bytes = vec![0xa5u8; 32_768];
    let mut storage = SliceStorage::new(&mut bytes);
    let mut writer = ArchiveWriter::create(storage, 0xa2e9_e9d5).unwrap();
    writer.append(1, u32::MAX, b"metadata").unwrap();
    writer.append(2, 0, &[0x5a; 128]).unwrap();
    writer.append(4, 8, &[0x33; 512]).unwrap();
    writer.finish().unwrap();
    storage = writer.into_storage();
    let scan = scan_archive(&mut storage).unwrap();
    assert!(scan.complete);
    assert_eq!(scan.campaign_crc32, 0xa2e9_e9d5);
    assert_eq!(scan.valid_records, 4);
    let valid_bytes = scan.valid_bytes;
    drop(storage);

    bytes[256 + 32 + 1] ^= 0x80;
    let mut corrupt = SliceStorage::new(&mut bytes);
    assert_eq!(scan_archive(&mut corrupt), Err(ArchiveError::Checksum));
    drop(corrupt);
    bytes[256 + 32 + 1] ^= 0x80;
    bytes[valid_bytes as usize..].fill(0xcc);
}

#[test]
fn unfinished_kra4_preserves_a_valid_prefix() {
    let mut bytes = vec![0u8; 4_096];
    let storage = SliceStorage::new(&mut bytes);
    let mut writer = ArchiveWriter::create(storage, 0x1234_5678).unwrap();
    writer.append(1, 0, &[7; 200]).unwrap();
    let mut storage = writer.into_storage();
    let scan = scan_archive(&mut storage).unwrap();
    assert!(!scan.complete);
    assert_eq!(scan.valid_records, 1);
}
#[test]
fn detail_selection_uses_frozen_priority_then_lowest_indices() {
    let mut summaries = Vec::with_capacity(REFERENCE_RUNS as usize);
    for index in 0..REFERENCE_RUNS as usize {
        let offset = index * RUN_SUMMARY_LENGTH;
        summaries.push(parse_ksr4(&REFERENCE_KSR4[offset..offset + RUN_SUMMARY_LENGTH]).unwrap());
    }
    let mut selected = [u32::MAX; 114];
    assert_eq!(
        select_detailed_runs(&summaries, &mut selected).unwrap(),
        selected.len()
    );
    assert_eq!(&selected[..5], &[0, 8, 96, 796, 1]);
    assert_eq!(&selected[5..10], &[2, 3, 4, 5, 6]);
}
struct FailingStorage {
    bytes: Vec<u8>,
    writes: usize,
    fail_on_write: Option<usize>,
}
impl ArchiveStorage for FailingStorage {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ()> {
        let start = offset as usize;
        out.copy_from_slice(self.bytes.get(start..start + out.len()).ok_or(())?);
        Ok(())
    }
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ()> {
        self.writes += 1;
        if self.fail_on_write == Some(self.writes) {
            return Err(());
        }
        let start = offset as usize;
        self.bytes
            .get_mut(start..start + bytes.len())
            .ok_or(())?
            .copy_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn failed_record_commit_leaves_the_prior_archive_prefix_valid() {
    let storage = FailingStorage {
        bytes: vec![0u8; 4_096],
        writes: 0,
        fail_on_write: Some(4),
    };
    let mut writer = ArchiveWriter::create(storage, 0xa2e9_e9d5).unwrap();
    assert_eq!(
        writer.append(3, 12, &[0x77; 200]),
        Err(ArchiveError::Storage)
    );
    let mut storage = writer.into_storage();
    storage.fail_on_write = None;
    let scan = scan_archive(&mut storage).unwrap();
    assert!(!scan.complete);
    assert_eq!(scan.valid_records, 0);
    assert_eq!(scan.valid_bytes, 256);
}
