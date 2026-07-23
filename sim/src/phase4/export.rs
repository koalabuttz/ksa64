//! Configurable export manifests and strict KXV4 volume framing.

use ksa64_interface::crc32_ieee;

use super::contracts::{EXPORT_VOLUME_HEADER_LENGTH, KXV4_MAGIC};
use super::PHASE4_CONTRACT_ID;

pub const KXV4_VERSION: u16 = 4;
pub const DEFAULT_VOLUME_PAYLOAD: u32 = 160 * 1_024;
pub const MAX_SUMMARY_RANGES: usize = 8;
pub const MAX_SELECTED_HISTORIES: usize = 16;
const HEADER_CRC_OFFSET: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportMode {
    OneVolume,
    MultiVolume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SummaryRange {
    pub start: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportManifest {
    pub include_config: bool,
    pub include_aggregate: bool,
    pub mode: ExportMode,
    pub volume_payload_limit: u32,
    pub summary_range_count: u8,
    pub summary_ranges: [SummaryRange; MAX_SUMMARY_RANGES],
    pub compact_count: u8,
    pub compact_runs: [u32; MAX_SELECTED_HISTORIES],
    pub full_count: u8,
    pub full_runs: [u32; MAX_SELECTED_HISTORIES],
}

impl ExportManifest {
    pub const fn stock_default() -> Self {
        let mut ranges = [SummaryRange { start: 0, count: 0 }; MAX_SUMMARY_RANGES];
        ranges[0] = SummaryRange { start: 0, count: 2 };
        ranges[1] = SummaryRange { start: 8, count: 1 };
        ranges[2] = SummaryRange {
            start: 96,
            count: 1,
        };
        ranges[3] = SummaryRange {
            start: 796,
            count: 1,
        };
        let mut compact = [0u32; MAX_SELECTED_HISTORIES];
        compact[0] = 0;
        Self {
            include_config: true,
            include_aggregate: true,
            mode: ExportMode::OneVolume,
            volume_payload_limit: DEFAULT_VOLUME_PAYLOAD,
            summary_range_count: 4,
            summary_ranges: ranges,
            compact_count: 1,
            compact_runs: compact,
            full_count: 0,
            full_runs: [0; MAX_SELECTED_HISTORIES],
        }
    }

    pub fn validate(&self, run_count: u32) -> Result<(), ExportError> {
        if self.volume_payload_limit == 0
            || self.summary_range_count as usize > MAX_SUMMARY_RANGES
            || self.compact_count as usize > MAX_SELECTED_HISTORIES
            || self.full_count as usize > MAX_SELECTED_HISTORIES
        {
            return Err(ExportError::Manifest);
        }
        let mut prior_end = 0u32;
        for range in &self.summary_ranges[..self.summary_range_count as usize] {
            let end = range
                .start
                .checked_add(range.count)
                .ok_or(ExportError::Manifest)?;
            if range.count == 0 || range.start < prior_end || end > run_count {
                return Err(ExportError::Manifest);
            }
            prior_end = end;
        }
        validate_runs(&self.compact_runs[..self.compact_count as usize], run_count)?;
        validate_runs(&self.full_runs[..self.full_count as usize], run_count)?;
        Ok(())
    }

    pub fn selection_crc32(&self) -> u32 {
        let mut bytes = [0u8; 208];
        bytes[0] = self.include_config as u8;
        bytes[1] = self.include_aggregate as u8;
        bytes[2] = match self.mode {
            ExportMode::OneVolume => 0,
            ExportMode::MultiVolume => 1,
        };
        bytes[3] = self.summary_range_count;
        bytes[4] = self.compact_count;
        bytes[5] = self.full_count;
        put_u32(&mut bytes, 8, self.volume_payload_limit);
        let mut offset = 16usize;
        for range in self.summary_ranges {
            put_u32(&mut bytes, offset, range.start);
            put_u32(&mut bytes, offset + 4, range.count);
            offset += 8;
        }
        for run in self.compact_runs {
            put_u32(&mut bytes, offset, run);
            offset += 4;
        }
        for run in self.full_runs {
            put_u32(&mut bytes, offset, run);
            offset += 4;
        }
        crc32_ieee(&bytes)
    }
}

fn validate_runs(runs: &[u32], run_count: u32) -> Result<(), ExportError> {
    for (index, run) in runs.iter().enumerate() {
        if *run >= run_count || runs[..index].contains(run) {
            return Err(ExportError::Manifest);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeHeader {
    pub archive_crc32: u32,
    pub selection_crc32: u32,
    pub volume_index: u16,
    pub volume_count: u16,
    pub logical_offset: u32,
    pub payload_length: u32,
    pub logical_length: u32,
    pub payload_crc32: u32,
}

pub struct ExportVolume<'a> {
    pub header: VolumeHeader,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportError {
    Manifest,
    Oversized,
    Length,
    Magic,
    Version,
    Contract,
    Reserved,
    Checksum,
    Identity,
    Order,
}

pub fn write_kxv4(
    mut header: VolumeHeader,
    payload: &[u8],
    output: &mut [u8],
) -> Result<(), ExportError> {
    if output.len() != EXPORT_VOLUME_HEADER_LENGTH + payload.len()
        || payload.len() > u32::MAX as usize
        || header.volume_count == 0
        || header.volume_index >= header.volume_count
        || header.payload_length != payload.len() as u32
        || header
            .logical_offset
            .checked_add(header.payload_length)
            .ok_or(ExportError::Length)?
            > header.logical_length
    {
        return Err(ExportError::Length);
    }
    header.payload_crc32 = crc32_ieee(payload);
    output.fill(0);
    output[0..4].copy_from_slice(&KXV4_MAGIC);
    put_u16(output, 4, KXV4_VERSION);
    put_u16(output, 6, EXPORT_VOLUME_HEADER_LENGTH as u16);
    put_u32(output, 8, PHASE4_CONTRACT_ID);
    put_u32(output, 12, header.archive_crc32);
    put_u32(output, 16, header.selection_crc32);
    put_u16(output, 20, header.volume_index);
    put_u16(output, 22, header.volume_count);
    put_u32(output, 24, header.logical_offset);
    put_u32(output, 28, header.payload_length);
    put_u32(output, 32, header.logical_length);
    put_u32(output, 36, header.payload_crc32);
    output[EXPORT_VOLUME_HEADER_LENGTH..].copy_from_slice(payload);
    let header_crc = crc32_ieee(&output[..HEADER_CRC_OFFSET]);
    put_u32(output, HEADER_CRC_OFFSET, header_crc);
    Ok(())
}

pub fn parse_kxv4(input: &[u8]) -> Result<ExportVolume<'_>, ExportError> {
    if input.len() < EXPORT_VOLUME_HEADER_LENGTH {
        return Err(ExportError::Length);
    }
    if input[0..4] != KXV4_MAGIC {
        return Err(ExportError::Magic);
    }
    if get_u16(input, 4) != KXV4_VERSION || get_u16(input, 6) != EXPORT_VOLUME_HEADER_LENGTH as u16
    {
        return Err(ExportError::Version);
    }
    if get_u32(input, 8) != PHASE4_CONTRACT_ID {
        return Err(ExportError::Contract);
    }
    if input[40..HEADER_CRC_OFFSET].iter().any(|&byte| byte != 0) {
        return Err(ExportError::Reserved);
    }
    if get_u32(input, HEADER_CRC_OFFSET) != crc32_ieee(&input[..HEADER_CRC_OFFSET]) {
        return Err(ExportError::Checksum);
    }
    let header = VolumeHeader {
        archive_crc32: get_u32(input, 12),
        selection_crc32: get_u32(input, 16),
        volume_index: get_u16(input, 20),
        volume_count: get_u16(input, 22),
        logical_offset: get_u32(input, 24),
        payload_length: get_u32(input, 28),
        logical_length: get_u32(input, 32),
        payload_crc32: get_u32(input, 36),
    };
    if header.volume_count == 0
        || header.volume_index >= header.volume_count
        || input.len() != EXPORT_VOLUME_HEADER_LENGTH + header.payload_length as usize
        || header
            .logical_offset
            .checked_add(header.payload_length)
            .ok_or(ExportError::Length)?
            > header.logical_length
    {
        return Err(ExportError::Length);
    }
    let payload = &input[EXPORT_VOLUME_HEADER_LENGTH..];
    if crc32_ieee(payload) != header.payload_crc32 {
        return Err(ExportError::Checksum);
    }
    Ok(ExportVolume { header, payload })
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
