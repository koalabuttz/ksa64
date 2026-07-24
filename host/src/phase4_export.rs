//! Host report-pack construction, KXV4 splitting, and strict joining.

use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::archive::{scan_archive, ArchiveStorage, ArchiveWriter, SliceStorage};
use ksa64_sim::phase4::detail::parse_kst4;
use ksa64_sim::phase4::export::{
    parse_kxv4, write_kxv4, ExportError, ExportManifest, ExportMode, VolumeHeader,
};
use ksa64_sim::phase4::plot::parse_kph4;

pub const STOCK_REPORT_CAPACITY: usize = 3_712;
pub const STOCK_RETAINED_RUNS: [u32; 5] = [0, 8, 96, 796, 1];

#[derive(Clone, Copy)]
pub struct HistoryRecord<'a> {
    pub run_index: u32,
    pub bytes: &'a [u8],
}

pub fn encode_volumes(
    logical: &[u8],
    archive_crc32: u32,
    selection_crc32: u32,
    mode: ExportMode,
    payload_limit: u32,
) -> Result<Vec<Vec<u8>>, ExportError> {
    if payload_limit == 0 || logical.len() > u32::MAX as usize {
        return Err(ExportError::Length);
    }
    if matches!(mode, ExportMode::OneVolume) && logical.len() > payload_limit as usize {
        return Err(ExportError::Oversized);
    }
    let volume_count = logical.len().max(1).div_ceil(payload_limit as usize);
    if volume_count > u16::MAX as usize {
        return Err(ExportError::Oversized);
    }
    let mut volumes = Vec::with_capacity(volume_count);
    for index in 0..volume_count {
        let offset = index * payload_limit as usize;
        let end = (offset + payload_limit as usize).min(logical.len());
        let payload = &logical[offset..end];
        let header = VolumeHeader {
            archive_crc32,
            selection_crc32,
            volume_index: index as u16,
            volume_count: volume_count as u16,
            logical_offset: offset as u32,
            payload_length: payload.len() as u32,
            logical_length: logical.len() as u32,
            payload_crc32: 0,
        };
        let mut encoded = vec![0u8; 64 + payload.len()];
        write_kxv4(header, payload, &mut encoded)?;
        volumes.push(encoded);
    }
    Ok(volumes)
}

pub fn join_volumes(volumes: &[Vec<u8>]) -> Result<Vec<u8>, ExportError> {
    if volumes.is_empty() {
        return Err(ExportError::Order);
    }
    let first = parse_kxv4(&volumes[0])?;
    if volumes.len() != first.header.volume_count as usize {
        return Err(ExportError::Order);
    }
    let mut logical = vec![0u8; first.header.logical_length as usize];
    let mut expected_offset = 0u32;
    for (index, encoded) in volumes.iter().enumerate() {
        let volume = parse_kxv4(encoded)?;
        if volume.header.archive_crc32 != first.header.archive_crc32
            || volume.header.selection_crc32 != first.header.selection_crc32
            || volume.header.volume_count != first.header.volume_count
        {
            return Err(ExportError::Identity);
        }
        if volume.header.volume_index as usize != index
            || volume.header.logical_offset != expected_offset
            || volume.header.logical_length != first.header.logical_length
        {
            return Err(ExportError::Order);
        }
        let start = volume.header.logical_offset as usize;
        logical[start..start + volume.payload.len()].copy_from_slice(volume.payload);
        expected_offset += volume.header.payload_length;
    }
    if expected_offset != first.header.logical_length
        || crc32_ieee(&logical) != first.header.archive_crc32
    {
        return Err(ExportError::Checksum);
    }
    Ok(logical)
}

pub struct ReportSources<'a> {
    pub campaign_crc32: u32,
    pub run_count: u32,
    pub ksc4: &'a [u8],
    pub aggregate: &'a [u8],
    pub ksr4: &'a [u8],
    pub compact: &'a [HistoryRecord<'a>],
    pub full: &'a [HistoryRecord<'a>],
}

pub fn build_selected_report(
    manifest: &ExportManifest,
    sources: &ReportSources<'_>,
) -> Result<Vec<u8>, ExportError> {
    let ReportSources {
        campaign_crc32,
        run_count,
        ksc4,
        aggregate,
        ksr4,
        compact,
        full,
    } = *sources;
    manifest.validate(run_count)?;
    if ksc4.len() != 512 || aggregate.is_empty() || ksr4.len() != run_count as usize * 128 {
        return Err(ExportError::Length);
    }
    let mut records = 0usize;
    let mut payload_bytes = 0usize;
    let mut add = |length: usize| -> Result<(), ExportError> {
        records = records.checked_add(1).ok_or(ExportError::Length)?;
        payload_bytes = payload_bytes
            .checked_add(length)
            .ok_or(ExportError::Length)?;
        Ok(())
    };
    if manifest.include_config {
        add(ksc4.len())?;
    }
    if manifest.include_aggregate {
        add(aggregate.len())?;
    }
    for range in &manifest.summary_ranges[..manifest.summary_range_count as usize] {
        for _ in 0..range.count {
            add(128)?;
        }
    }
    for run in &manifest.compact_runs[..manifest.compact_count as usize] {
        let history = find_history(compact, *run)?;
        let parsed = parse_kph4(history.bytes).map_err(|_| ExportError::Checksum)?;
        if parsed.identity.run_index != *run || parsed.identity.campaign_crc32 != campaign_crc32 {
            return Err(ExportError::Identity);
        }
        add(history.bytes.len())?;
    }
    for run in &manifest.full_runs[..manifest.full_count as usize] {
        let history = find_history(full, *run)?;
        let parsed = parse_kst4(history.bytes).map_err(|_| ExportError::Checksum)?;
        if parsed.header.run_index != *run || parsed.header.campaign_crc32 != campaign_crc32 {
            return Err(ExportError::Identity);
        }
        add(history.bytes.len())?;
    }
    let capacity = 256usize
        .checked_add(records.checked_mul(32).ok_or(ExportError::Length)?)
        .and_then(|value| value.checked_add(payload_bytes))
        .and_then(|value| value.checked_add(48))
        .ok_or(ExportError::Length)?;
    if matches!(manifest.mode, ExportMode::OneVolume)
        && capacity > manifest.volume_payload_limit as usize
    {
        return Err(ExportError::Oversized);
    }
    let mut report = vec![0u8; capacity];
    let storage = SliceStorage::new(&mut report);
    let mut writer =
        ArchiveWriter::create(storage, campaign_crc32).map_err(|_| ExportError::Length)?;
    if manifest.include_config {
        writer
            .append(1, u32::MAX, ksc4)
            .map_err(|_| ExportError::Length)?;
    }
    if manifest.include_aggregate {
        writer
            .append(2, u32::MAX, aggregate)
            .map_err(|_| ExportError::Length)?;
    }
    for range in &manifest.summary_ranges[..manifest.summary_range_count as usize] {
        for run in range.start..range.start + range.count {
            let start = run as usize * 128;
            writer
                .append(3, run, &ksr4[start..start + 128])
                .map_err(|_| ExportError::Length)?;
        }
    }
    for run in &manifest.compact_runs[..manifest.compact_count as usize] {
        let history = find_history(compact, *run)?;
        writer
            .append(4, *run, history.bytes)
            .map_err(|_| ExportError::Length)?;
    }
    for run in &manifest.full_runs[..manifest.full_count as usize] {
        let history = find_history(full, *run)?;
        writer
            .append(5, *run, history.bytes)
            .map_err(|_| ExportError::Length)?;
    }
    writer.finish().map_err(|_| ExportError::Length)?;
    {
        let mut storage = writer.into_storage();
        let scan = scan_archive(&mut storage).map_err(|_| ExportError::Checksum)?;
        if !scan.complete || scan.valid_bytes as usize != capacity {
            return Err(ExportError::Length);
        }
    }
    Ok(report)
}

fn find_history<'a>(
    histories: &'a [HistoryRecord<'a>],
    run_index: u32,
) -> Result<HistoryRecord<'a>, ExportError> {
    histories
        .iter()
        .copied()
        .find(|history| history.run_index == run_index)
        .ok_or(ExportError::Manifest)
}
pub fn build_stock_report(
    ksc4: &[u8],
    ksr4: &[u8],
    kph4: &[u8],
) -> Result<(Vec<u8>, ExportManifest), ExportError> {
    if ksc4.len() != 512 || ksr4.len() < 1_024 * 128 {
        return Err(ExportError::Length);
    }
    let mut report = vec![0u8; STOCK_REPORT_CAPACITY];
    let storage = SliceStorage::new(&mut report);
    let mut writer =
        ArchiveWriter::create(storage, 0xa2e9_e9d5).map_err(|_| ExportError::Length)?;
    writer
        .append(1, u32::MAX, ksc4)
        .map_err(|_| ExportError::Length)?;
    let aggregate = stock_aggregate_payload();
    writer
        .append(2, u32::MAX, &aggregate)
        .map_err(|_| ExportError::Length)?;
    for run in STOCK_RETAINED_RUNS {
        let start = run as usize * 128;
        writer
            .append(3, run, &ksr4[start..start + 128])
            .map_err(|_| ExportError::Length)?;
    }
    writer.append(4, 0, kph4).map_err(|_| ExportError::Length)?;
    writer.finish().map_err(|_| ExportError::Length)?;
    {
        let mut storage = writer.into_storage();
        let scan = scan_archive(&mut storage).map_err(|_| ExportError::Checksum)?;
        if !scan.complete || scan.valid_bytes as usize != STOCK_REPORT_CAPACITY {
            return Err(ExportError::Length);
        }
    }
    let manifest = ExportManifest::stock_default();
    manifest.validate(1_024)?;
    Ok((report, manifest))
}

pub fn stock_aggregate_payload() -> [u8; 128] {
    let mut payload = [0u8; 128];
    payload[0..4].copy_from_slice(b"KAG4");
    payload[4..8].copy_from_slice(&4u32.to_le_bytes());
    payload[8..12].copy_from_slice(&1_024u32.to_le_bytes());
    for (index, value) in [857u32, 166, 1, 0, 0, 0].iter().enumerate() {
        let offset = 12 + index * 4;
        payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    payload[36..40].copy_from_slice(&0x813c_e420u32.to_le_bytes());
    payload[40..44].copy_from_slice(&180i32.to_le_bytes());
    payload[44..48].copy_from_slice(&194i32.to_le_bytes());
    payload[48..52].copy_from_slice(&39i32.to_le_bytes());
    payload[52..56].copy_from_slice(&43i32.to_le_bytes());
    payload[56..60].copy_from_slice(&0i32.to_le_bytes());
    payload[60..64].copy_from_slice(&62i32.to_le_bytes());
    let crc = crc32_ieee(&payload[..124]);
    payload[124..128].copy_from_slice(&crc.to_le_bytes());
    payload
}

pub fn validate_joined_archive(bytes: &mut [u8]) -> Result<(), ExportError> {
    let mut storage = SliceStorage::new(bytes);
    let scan = scan_archive(&mut storage).map_err(|_| ExportError::Checksum)?;
    if !scan.complete || scan.valid_bytes != storage.capacity() {
        return Err(ExportError::Order);
    }
    Ok(())
}
