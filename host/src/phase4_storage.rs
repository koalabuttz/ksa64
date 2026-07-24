//! Host file transport for transport-neutral KRA4 archives.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use ksa64_sim::phase4::archive::{ArchiveStorage, ArchiveStorageError};
use ksa64_sim::phase5_archive::{
    ArchiveStorage as Phase5ArchiveStorage, ArchiveStorageError as Phase5ArchiveStorageError,
};

pub struct FileArchiveStorage {
    file: File,
    capacity: u32,
}

impl FileArchiveStorage {
    pub fn create(path: &Path, capacity: u32) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.set_len(capacity as u64)?;
        Ok(Self { file, capacity })
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let length = file.metadata()?.len();
        let capacity = u32::try_from(length).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "archive exceeds u32 capacity",
            )
        })?;
        Ok(Self { file, capacity })
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.file.sync_data()
    }
}

impl ArchiveStorage for FileArchiveStorage {
    fn capacity(&self) -> u32 {
        self.capacity
    }

    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ArchiveStorageError> {
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_| ArchiveStorageError)?;
        self.file.read_exact(out).map_err(|_| ArchiveStorageError)
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ArchiveStorageError> {
        let end = offset
            .checked_add(bytes.len() as u32)
            .ok_or(ArchiveStorageError)?;
        if end > self.capacity {
            return Err(ArchiveStorageError);
        }
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_| ArchiveStorageError)?;
        self.file.write_all(bytes).map_err(|_| ArchiveStorageError)
    }
}

impl Phase5ArchiveStorage for FileArchiveStorage {
    fn capacity(&self) -> u32 {
        self.capacity
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), Phase5ArchiveStorageError> {
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_| Phase5ArchiveStorageError)?;
        self.file
            .read_exact(out)
            .map_err(|_| Phase5ArchiveStorageError)
    }
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Phase5ArchiveStorageError> {
        let end = offset
            .checked_add(bytes.len() as u32)
            .ok_or(Phase5ArchiveStorageError)?;
        if end > self.capacity {
            return Err(Phase5ArchiveStorageError);
        }
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|_| Phase5ArchiveStorageError)?;
        self.file
            .write_all(bytes)
            .map_err(|_| Phase5ArchiveStorageError)
    }
}
