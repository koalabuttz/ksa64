use ksa64_host::phase4_storage::FileArchiveStorage;
use ksa64_sim::phase5_archive::{scan_archive, ArchiveWriter};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
#[test]
fn kra5_host_file_round_trip() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ksa64-kra5-{}-{stamp}.bin", std::process::id()));
    let storage = FileArchiveStorage::create(&path, 8192).unwrap();
    let mut writer = ArchiveWriter::create(storage, 0x58851234).unwrap();
    writer.append(1, u32::MAX, b"KSC5 metadata").unwrap();
    writer.append(4, 0, &[0x44; 1664]).unwrap();
    writer.finish().unwrap();
    let mut storage = writer.into_storage();
    storage.sync().unwrap();
    drop(storage);
    let mut reopened = FileArchiveStorage::open(&path).unwrap();
    let scan = scan_archive(&mut reopened).unwrap();
    assert!(scan.complete);
    assert_eq!(scan.valid_records, 3);
    drop(reopened);
    fs::remove_file(path).unwrap();
}
