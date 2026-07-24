//! Transport-neutral append-only KRA5 spatial-history archives.

use ksa64_interface::crc32_ieee;

pub const ARCHIVE_RECORD_HEADER_LENGTH: usize = 32;
pub const ARCHIVE_SUPERBLOCK_LENGTH: usize = 256;
pub const KRA5_MAGIC: [u8; 4] = *b"KRA5";
pub const KRA5_CONTRACT_ID: u32 = 0x050c_0002;

pub const KRA5_VERSION: u16 = 5;
pub const RECORD_COMMITTED: u16 = 1;
pub const RECORD_CONFIG: u16 = 1;
pub const RECORD_AGGREGATE: u16 = 2;
pub const RECORD_SUMMARIES: u16 = 3;
pub const RECORD_COMPACT_HISTORY: u16 = 4;
pub const RECORD_FULL_KST5: u16 = 5;
pub const RECORD_FOOTER: u16 = 0xffff;
const SUPERBLOCK_CRC_OFFSET: usize = ARCHIVE_SUPERBLOCK_LENGTH - 4;
const RECORD_CRC_OFFSET: usize = ARCHIVE_RECORD_HEADER_LENGTH - 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveStorageError;

pub trait ArchiveStorage {
    fn capacity(&self) -> u32;
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ArchiveStorageError>;
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ArchiveStorageError>;
}

pub struct SliceStorage<'a> {
    bytes: &'a mut [u8],
}
impl<'a> SliceStorage<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes }
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }
}
impl ArchiveStorage for SliceStorage<'_> {
    fn capacity(&self) -> u32 {
        self.bytes.len().min(u32::MAX as usize) as u32
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), ArchiveStorageError> {
        let start = offset as usize;
        let end = start.checked_add(out.len()).ok_or(ArchiveStorageError)?;
        out.copy_from_slice(self.bytes.get(start..end).ok_or(ArchiveStorageError)?);
        Ok(())
    }
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ArchiveStorageError> {
        let start = offset as usize;
        let end = start.checked_add(bytes.len()).ok_or(ArchiveStorageError)?;
        self.bytes
            .get_mut(start..end)
            .ok_or(ArchiveStorageError)?
            .copy_from_slice(bytes);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    Storage,
    Capacity,
    Length,
    Magic,
    Version,
    Contract,
    Reserved,
    Checksum,
    Sequence,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveScan {
    pub campaign_crc32: u32,
    pub valid_records: u32,
    pub valid_bytes: u32,
    pub complete: bool,
    pub chain: u32,
}

pub struct ArchiveWriter<S: ArchiveStorage> {
    storage: S,
    campaign_crc32: u32,
    logical_length: u32,
    record_count: u32,
    chain: u32,
    finished: bool,
}

impl<S: ArchiveStorage> ArchiveWriter<S> {
    pub fn create(storage: S, campaign_crc32: u32) -> Result<Self, ArchiveError> {
        if storage.capacity() < ARCHIVE_SUPERBLOCK_LENGTH as u32 {
            return Err(ArchiveError::Capacity);
        }
        let mut writer = Self {
            storage,
            campaign_crc32,
            logical_length: ARCHIVE_SUPERBLOCK_LENGTH as u32,
            record_count: 0,
            chain: 2_166_136_261,
            finished: false,
        };
        writer.write_superblock(false)?;
        Ok(writer)
    }

    pub fn append(
        &mut self,
        kind: u16,
        run_index: u32,
        payload: &[u8],
    ) -> Result<(), ArchiveError> {
        if self.finished || kind == RECORD_FOOTER || payload.len() > u32::MAX as usize {
            return Err(ArchiveError::Length);
        }
        self.append_internal(kind, run_index, payload)
    }

    pub fn finish(&mut self) -> Result<(), ArchiveError> {
        if self.finished {
            return Ok(());
        }
        let mut footer = [0u8; 16];
        put_u32(&mut footer, 0, self.record_count);
        put_u32(&mut footer, 4, self.logical_length);
        put_u32(&mut footer, 8, self.chain);
        put_u32(&mut footer, 12, self.campaign_crc32);
        self.append_internal(RECORD_FOOTER, u32::MAX, &footer)?;
        self.finished = true;
        self.write_superblock(true)
    }

    pub fn into_storage(self) -> S {
        self.storage
    }

    fn append_internal(
        &mut self,
        kind: u16,
        run_index: u32,
        payload: &[u8],
    ) -> Result<(), ArchiveError> {
        let total = (ARCHIVE_RECORD_HEADER_LENGTH as u32)
            .checked_add(payload.len() as u32)
            .ok_or(ArchiveError::Length)?;
        let end = self
            .logical_length
            .checked_add(total)
            .ok_or(ArchiveError::Length)?;
        if end > self.storage.capacity() {
            return Err(ArchiveError::Capacity);
        }
        let payload_crc = crc32_ieee(payload);
        let mut header = record_header(
            kind,
            0,
            self.record_count,
            run_index,
            payload.len() as u32,
            payload_crc,
            self.chain,
        );
        self.storage
            .write(self.logical_length, &header)
            .map_err(|_| ArchiveError::Storage)?;
        self.storage
            .write(
                self.logical_length + ARCHIVE_RECORD_HEADER_LENGTH as u32,
                payload,
            )
            .map_err(|_| ArchiveError::Storage)?;
        put_u16(&mut header, 2, RECORD_COMMITTED);
        let header_crc = crc32_ieee(&header[..RECORD_CRC_OFFSET]);
        put_u32(&mut header, RECORD_CRC_OFFSET, header_crc);
        self.storage
            .write(self.logical_length, &header)
            .map_err(|_| ArchiveError::Storage)?;
        self.chain = mix_record(
            self.chain,
            kind,
            self.record_count,
            run_index,
            payload.len() as u32,
            payload_crc,
        );
        self.logical_length = end;
        self.record_count += 1;
        self.write_superblock(false)
    }

    fn write_superblock(&mut self, complete: bool) -> Result<(), ArchiveError> {
        let mut block = [0u8; ARCHIVE_SUPERBLOCK_LENGTH];
        block[0..4].copy_from_slice(&KRA5_MAGIC);
        put_u16(&mut block, 4, KRA5_VERSION);
        put_u16(&mut block, 6, ARCHIVE_SUPERBLOCK_LENGTH as u16);
        put_u32(&mut block, 8, KRA5_CONTRACT_ID);
        put_u32(&mut block, 12, self.campaign_crc32);
        put_u32(&mut block, 16, self.storage.capacity());
        put_u32(&mut block, 20, self.logical_length);
        put_u32(&mut block, 24, self.record_count);
        put_u32(&mut block, 28, self.chain);
        block[32] = complete as u8;
        let block_crc = crc32_ieee(&block[..SUPERBLOCK_CRC_OFFSET]);
        put_u32(&mut block, SUPERBLOCK_CRC_OFFSET, block_crc);
        self.storage
            .write(0, &block)
            .map_err(|_| ArchiveError::Storage)
    }
}

pub fn scan_archive<S: ArchiveStorage>(storage: &mut S) -> Result<ArchiveScan, ArchiveError> {
    let mut block = [0u8; ARCHIVE_SUPERBLOCK_LENGTH];
    storage
        .read(0, &mut block)
        .map_err(|_| ArchiveError::Storage)?;
    if block[0..4] != KRA5_MAGIC {
        return Err(ArchiveError::Magic);
    }
    if get_u16(&block, 4) != KRA5_VERSION || get_u16(&block, 6) != ARCHIVE_SUPERBLOCK_LENGTH as u16
    {
        return Err(ArchiveError::Version);
    }
    if get_u32(&block, 8) != KRA5_CONTRACT_ID {
        return Err(ArchiveError::Contract);
    }
    if block[33..SUPERBLOCK_CRC_OFFSET]
        .iter()
        .any(|&value| value != 0)
        || block[32] > 1
    {
        return Err(ArchiveError::Reserved);
    }
    if crc32_ieee(&block[..SUPERBLOCK_CRC_OFFSET]) != get_u32(&block, SUPERBLOCK_CRC_OFFSET) {
        return Err(ArchiveError::Checksum);
    }
    if get_u32(&block, 16) != storage.capacity() {
        return Err(ArchiveError::Capacity);
    }
    let declared_length = get_u32(&block, 20);
    let declared_records = get_u32(&block, 24);
    let declared_chain = get_u32(&block, 28);
    let campaign = get_u32(&block, 12);
    let mut offset = ARCHIVE_SUPERBLOCK_LENGTH as u32;
    let mut sequence = 0u32;
    let mut chain = 2_166_136_261u32;
    let mut footer_valid = false;
    while sequence < declared_records {
        let mut header = [0u8; ARCHIVE_RECORD_HEADER_LENGTH];
        storage
            .read(offset, &mut header)
            .map_err(|_| ArchiveError::Storage)?;
        if get_u32(&header, RECORD_CRC_OFFSET) != crc32_ieee(&header[..RECORD_CRC_OFFSET]) {
            return Err(ArchiveError::Checksum);
        }
        if get_u16(&header, 2) != RECORD_COMMITTED || get_u32(&header, 4) != sequence {
            return Err(ArchiveError::Incomplete);
        }
        if get_u32(&header, 20) != chain || get_u32(&header, 24) != 0 {
            return Err(ArchiveError::Sequence);
        }
        let kind = get_u16(&header, 0);
        let run = get_u32(&header, 8);
        let length = get_u32(&header, 12);
        let expected_crc = get_u32(&header, 16);
        let payload_offset = offset + ARCHIVE_RECORD_HEADER_LENGTH as u32;
        if payload_offset
            .checked_add(length)
            .ok_or(ArchiveError::Length)?
            > declared_length
        {
            return Err(ArchiveError::Length);
        }
        let actual_crc = crc_storage(storage, payload_offset, length)?;
        if actual_crc != expected_crc {
            return Err(ArchiveError::Checksum);
        }
        if kind == RECORD_FOOTER {
            if length != 16 || sequence + 1 != declared_records {
                return Err(ArchiveError::Sequence);
            }
            let mut footer = [0u8; 16];
            storage
                .read(payload_offset, &mut footer)
                .map_err(|_| ArchiveError::Storage)?;
            if get_u32(&footer, 0) != sequence
                || get_u32(&footer, 4) != offset
                || get_u32(&footer, 8) != chain
                || get_u32(&footer, 12) != campaign
            {
                return Err(ArchiveError::Sequence);
            }
            footer_valid = true;
        }
        chain = mix_record(chain, kind, sequence, run, length, expected_crc);
        offset = payload_offset + length;
        sequence += 1;
    }
    if offset != declared_length || chain != declared_chain {
        return Err(ArchiveError::Sequence);
    }
    let complete = block[32] == 1;
    if complete != footer_valid {
        return Err(ArchiveError::Incomplete);
    }
    Ok(ArchiveScan {
        campaign_crc32: campaign,
        valid_records: sequence,
        valid_bytes: offset,
        complete,
        chain,
    })
}

fn record_header(
    kind: u16,
    flags: u16,
    sequence: u32,
    run: u32,
    length: u32,
    payload_crc: u32,
    previous_chain: u32,
) -> [u8; ARCHIVE_RECORD_HEADER_LENGTH] {
    let mut header = [0u8; ARCHIVE_RECORD_HEADER_LENGTH];
    put_u16(&mut header, 0, kind);
    put_u16(&mut header, 2, flags);
    put_u32(&mut header, 4, sequence);
    put_u32(&mut header, 8, run);
    put_u32(&mut header, 12, length);
    put_u32(&mut header, 16, payload_crc);
    put_u32(&mut header, 20, previous_chain);
    let header_crc = crc32_ieee(&header[..RECORD_CRC_OFFSET]);
    put_u32(&mut header, RECORD_CRC_OFFSET, header_crc);
    header
}

fn mix_record(chain: u32, kind: u16, sequence: u32, run: u32, length: u32, crc: u32) -> u32 {
    let mut bytes = [0u8; 22];
    put_u32(&mut bytes, 0, chain);
    put_u16(&mut bytes, 4, kind);
    put_u32(&mut bytes, 6, sequence);
    put_u32(&mut bytes, 10, run);
    put_u32(&mut bytes, 14, length);
    put_u32(&mut bytes, 18, crc);
    crc32_ieee(&bytes)
}

fn crc_storage<S: ArchiveStorage>(
    storage: &mut S,
    mut offset: u32,
    mut length: u32,
) -> Result<u32, ArchiveError> {
    let mut state = 0xffff_ffffu32;
    let mut chunk = [0u8; 64];
    while length != 0 {
        let count = length.min(chunk.len() as u32) as usize;
        storage
            .read(offset, &mut chunk[..count])
            .map_err(|_| ArchiveError::Storage)?;
        for &byte in &chunk[..count] {
            state ^= byte as u32;
            for _ in 0..8 {
                state = (state >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(state & 1));
            }
        }
        offset += count as u32;
        length -= count as u32;
    }
    Ok(!state)
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
