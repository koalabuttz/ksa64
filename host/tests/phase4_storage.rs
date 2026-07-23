use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use ksa64_host::phase4_storage::FileArchiveStorage;
use ksa64_sim::phase4::archive::{scan_archive, ArchiveWriter};

#[test]
fn host_file_archive_round_trips_through_shared_transport() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ksa64-kra4-{}-{stamp}.bin", std::process::id()));
    {
        let storage = FileArchiveStorage::create(&path, 8_192).unwrap();
        let mut writer = ArchiveWriter::create(storage, 0xa2e9_e9d5).unwrap();
        writer.append(1, u32::MAX, b"host metadata").unwrap();
        writer.append(3, 7, &[0x44; 256]).unwrap();
        writer.finish().unwrap();
        let mut storage = writer.into_storage();
        storage.sync().unwrap();
    }
    {
        let mut storage = FileArchiveStorage::open(&path).unwrap();
        let scan = scan_archive(&mut storage).unwrap();
        assert!(scan.complete);
        assert_eq!(scan.valid_records, 3);
    }
    fs::remove_file(path).unwrap();
}
