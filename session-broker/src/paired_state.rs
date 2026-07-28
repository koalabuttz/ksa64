//! Host-owned persistent identity and peer registry for the explicit paired-LAN launcher.
//!
//! This is noncanonical transport configuration. It is deliberately separate
//! from KSB11 evidence and strict mission records.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{NoiseTransportError, PeerRegistry, StaticNoiseKeypair};

pub const SERVER_IDENTITY_FILE: &str = "paired-server.ksk1";
pub const PEER_REGISTRY_FILE: &str = "paired-peers.ppr1";
const SERVER_MAGIC: [u8; 4] = *b"KSK1";
const SERVER_VERSION: u16 = 1;
const SERVER_LENGTH: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairedStateError {
    Io,
    IdentityLength,
    IdentityMagic,
    IdentityVersion,
    IdentityReserved,
    IdentityCrc,
    IdentityKey,
    Registry(NoiseTransportError),
}

pub fn load_or_create_server_identity(
    directory: &Path,
) -> Result<StaticNoiseKeypair, PairedStateError> {
    match read_recoverable(directory, SERVER_IDENTITY_FILE) {
        Ok(Some(bytes)) => decode_server_identity(&bytes),
        Ok(None) => {
            let keys = StaticNoiseKeypair::generate().map_err(|_| PairedStateError::IdentityKey)?;
            save_server_identity(directory, &keys)?;
            Ok(keys)
        }
        Err(error) => Err(error),
    }
}

pub fn load_peer_registry(directory: &Path) -> Result<PeerRegistry, PairedStateError> {
    match read_recoverable(directory, PEER_REGISTRY_FILE)? {
        Some(bytes) => PeerRegistry::import_bounded(&bytes).map_err(PairedStateError::Registry),
        None => Ok(PeerRegistry::default()),
    }
}

pub fn save_server_identity(
    directory: &Path,
    keys: &StaticNoiseKeypair,
) -> Result<(), PairedStateError> {
    atomic_write(
        directory,
        SERVER_IDENTITY_FILE,
        &encode_server_identity(keys),
    )
}

pub fn save_peer_registry(directory: &Path, bytes: &[u8]) -> Result<(), PairedStateError> {
    PeerRegistry::import_bounded(bytes).map_err(PairedStateError::Registry)?;
    atomic_write(directory, PEER_REGISTRY_FILE, bytes)
}

pub fn peer_registry_path(directory: &Path) -> PathBuf {
    directory.join(PEER_REGISTRY_FILE)
}

fn encode_server_identity(keys: &StaticNoiseKeypair) -> [u8; SERVER_LENGTH] {
    let mut output = [0_u8; SERVER_LENGTH];
    output[..4].copy_from_slice(&SERVER_MAGIC);
    put_u16(&mut output, 4, SERVER_VERSION);
    put_u16(&mut output, 6, SERVER_LENGTH as u16);
    output[8..40].copy_from_slice(&keys.private_key_for_secure_store());
    output[40..72].copy_from_slice(&keys.public_key());
    let checksum = crc32_ieee(&output[..76]);
    put_u32(&mut output, 76, checksum);
    output
}

fn decode_server_identity(input: &[u8]) -> Result<StaticNoiseKeypair, PairedStateError> {
    if input.len() != SERVER_LENGTH {
        return Err(PairedStateError::IdentityLength);
    }
    if input[..4] != SERVER_MAGIC {
        return Err(PairedStateError::IdentityMagic);
    }
    if get_u16(input, 4) != SERVER_VERSION || usize::from(get_u16(input, 6)) != SERVER_LENGTH {
        return Err(PairedStateError::IdentityVersion);
    }
    if input[72..76].iter().any(|value| *value != 0) {
        return Err(PairedStateError::IdentityReserved);
    }
    if get_u32(input, 76) != crc32_ieee(&input[..76]) {
        return Err(PairedStateError::IdentityCrc);
    }
    let mut private = [0_u8; 32];
    let mut public = [0_u8; 32];
    private.copy_from_slice(&input[8..40]);
    public.copy_from_slice(&input[40..72]);
    if private.iter().all(|value| *value == 0) || public.iter().all(|value| *value == 0) {
        return Err(PairedStateError::IdentityKey);
    }
    Ok(StaticNoiseKeypair::from_parts(private, public))
}

fn read_recoverable(directory: &Path, filename: &str) -> Result<Option<Vec<u8>>, PairedStateError> {
    let primary = directory.join(filename);
    match fs::read(&primary) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let backup = backup_path(&primary);
            match fs::read(backup) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(PairedStateError::Io),
            }
        }
        Err(_) => Err(PairedStateError::Io),
    }
}

fn atomic_write(directory: &Path, filename: &str, bytes: &[u8]) -> Result<(), PairedStateError> {
    fs::create_dir_all(directory).map_err(|_| PairedStateError::Io)?;
    let target = directory.join(filename);
    let temporary = temporary_path(&target);
    let backup = backup_path(&target);
    let _ = fs::remove_file(&temporary);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|_| PairedStateError::Io)?;
    file.write_all(bytes).map_err(|_| PairedStateError::Io)?;
    file.sync_all().map_err(|_| PairedStateError::Io)?;
    drop(file);

    let had_primary = target.exists();
    if had_primary {
        let _ = fs::remove_file(&backup);
        fs::rename(&target, &backup).map_err(|_| PairedStateError::Io)?;
    }
    if fs::rename(&temporary, &target).is_err() {
        if had_primary {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_file(&temporary);
        return Err(PairedStateError::Io);
    }
    if had_primary {
        let _ = fs::remove_file(&backup);
    }
    sync_directory(directory);
    Ok(())
}

fn temporary_path(target: &Path) -> PathBuf {
    target.with_extension("tmp")
}

fn backup_path(target: &Path) -> PathBuf {
    target.with_extension("bak")
}

#[cfg(unix)]
fn sync_directory(directory: &Path) {
    if let Ok(file) = std::fs::File::open(directory) {
        let _ = file.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) {}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn crc32_ieee(input: &[u8]) -> u32 {
    let mut value = 0xffff_ffff_u32;
    for byte in input {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
        }
    }
    !value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("ksa64-paired-state-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn server_identity_and_peer_registry_round_trip_and_reject_corruption() {
        let directory = test_directory("roundtrip");
        let created = load_or_create_server_identity(&directory).unwrap();
        let loaded = load_or_create_server_identity(&directory).unwrap();
        assert_eq!(created.public_key(), loaded.public_key());
        assert_eq!(load_peer_registry(&directory).unwrap().records().len(), 0);
        let path = directory.join(SERVER_IDENTITY_FILE);
        let mut corrupt = fs::read(&path).unwrap();
        corrupt[12] ^= 1;
        fs::write(&path, corrupt).unwrap();
        assert!(matches!(
            load_or_create_server_identity(&directory),
            Err(PairedStateError::IdentityCrc)
        ));
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_pairing_secrets_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let directory = test_directory("permissions");
        let _ = load_or_create_server_identity(&directory).unwrap();
        save_peer_registry(&directory, &PeerRegistry::new()).unwrap();
        for filename in [SERVER_IDENTITY_FILE, PEER_REGISTRY_FILE] {
            let mode = fs::metadata(directory.join(filename))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn backup_recovers_only_when_primary_is_absent() {
        let directory = test_directory("backup");
        let keys = load_or_create_server_identity(&directory).unwrap();
        let path = directory.join(SERVER_IDENTITY_FILE);
        fs::rename(&path, backup_path(&path)).unwrap();
        let restored = load_or_create_server_identity(&directory).unwrap();
        assert_eq!(keys.public_key(), restored.public_key());
        let _ = fs::remove_dir_all(directory);
    }
}
