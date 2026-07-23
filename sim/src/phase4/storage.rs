//! Capacity-scaled Phase 4 retention plans.

use super::contracts::{
    ARCHIVE_RECORD_HEADER_LENGTH, ARCHIVE_SUPERBLOCK_LENGTH, DETAIL_FRAME_LENGTH,
    DETAIL_HEADER_LENGTH, PLOT_HEADER_LENGTH, PLOT_POINT_LENGTH, RUN_SUMMARY_LENGTH,
    STOCK_INTERESTING_SUMMARIES,
};

pub const SUPPORTED_REU_KIB: [u32; 8] = [128, 256, 512, 1_024, 2_048, 4_096, 8_192, 16_384];
pub const AGGREGATE_RECORD_BYTES: u32 = 128;
pub const ARCHIVE_BASE_BYTES: u32 = ARCHIVE_SUPERBLOCK_LENGTH as u32
    + ARCHIVE_RECORD_HEADER_LENGTH as u32 * 3
    + AGGREGATE_RECORD_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReuPreference {
    Auto,
    Disabled,
    CapKiB(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageMode {
    Stock,
    Reu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoragePlan {
    pub mode: StorageMode,
    pub detected_kib: u32,
    pub effective_kib: u32,
    pub summary_slots: u32,
    pub full_histories: u32,
    pub compact_histories: u32,
    pub used_bytes: u32,
    pub free_bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoragePlanError {
    UnsupportedCapacity,
    SizeOverflow,
}

impl StoragePlan {
    pub fn compute(
        detected_kib: u32,
        preference: ReuPreference,
        run_count: u32,
        detail_frames: u32,
        compact_points: u32,
    ) -> Result<Self, StoragePlanError> {
        if detected_kib != 0 && !is_supported_capacity(detected_kib) {
            return Err(StoragePlanError::UnsupportedCapacity);
        }
        let effective_kib = match preference {
            ReuPreference::Disabled => 0,
            ReuPreference::Auto => detected_kib,
            ReuPreference::CapKiB(cap) => detected_kib.min(cap),
        };
        if effective_kib == 0 {
            let plot = checked_history_size(PLOT_HEADER_LENGTH, PLOT_POINT_LENGTH, compact_points)?;
            return Ok(Self {
                mode: StorageMode::Stock,
                detected_kib,
                effective_kib: 0,
                summary_slots: run_count.min(STOCK_INTERESTING_SUMMARIES as u32),
                full_histories: 0,
                compact_histories: u32::from(compact_points != 0),
                used_bytes: run_count.min(STOCK_INTERESTING_SUMMARIES as u32)
                    * RUN_SUMMARY_LENGTH as u32
                    + plot,
                free_bytes: 0,
            });
        }
        let capacity = effective_kib
            .checked_mul(1_024)
            .ok_or(StoragePlanError::SizeOverflow)?;
        let summary_budget = capacity / 4;
        let summary_slots = run_count.min(summary_budget / RUN_SUMMARY_LENGTH as u32);
        let summary_bytes = summary_slots
            .checked_mul(RUN_SUMMARY_LENGTH as u32)
            .ok_or(StoragePlanError::SizeOverflow)?;
        let full_payload =
            checked_history_size(DETAIL_HEADER_LENGTH, DETAIL_FRAME_LENGTH, detail_frames)?;
        let compact_payload =
            checked_history_size(PLOT_HEADER_LENGTH, PLOT_POINT_LENGTH, compact_points)?;
        let full_cost = full_payload + ARCHIVE_RECORD_HEADER_LENGTH as u32;
        let compact_cost = compact_payload + ARCHIVE_RECORD_HEADER_LENGTH as u32;
        let mut used = ARCHIVE_BASE_BYTES
            .checked_add(summary_bytes)
            .and_then(|value| value.checked_add(ARCHIVE_RECORD_HEADER_LENGTH as u32))
            .ok_or(StoragePlanError::SizeOverflow)?;
        if used > capacity {
            return Err(StoragePlanError::SizeOverflow);
        }
        let selectable = run_count;
        let full_histories = ((capacity - used) / full_cost).min(selectable);
        used += full_histories * full_cost;
        let compact_histories =
            ((capacity - used) / compact_cost).min(selectable.saturating_sub(full_histories));
        used += compact_histories * compact_cost;
        Ok(Self {
            mode: StorageMode::Reu,
            detected_kib,
            effective_kib,
            summary_slots,
            full_histories,
            compact_histories,
            used_bytes: used,
            free_bytes: capacity - used,
        })
    }
}

fn checked_history_size(header: usize, record: usize, count: u32) -> Result<u32, StoragePlanError> {
    (header as u32)
        .checked_add(
            (record as u32)
                .checked_mul(count)
                .ok_or(StoragePlanError::SizeOverflow)?,
        )
        .ok_or(StoragePlanError::SizeOverflow)
}

const fn is_supported_capacity(value: u32) -> bool {
    matches!(
        value,
        128 | 256 | 512 | 1_024 | 2_048 | 4_096 | 8_192 | 16_384
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionError {
    Empty,
    RunOrder,
}

pub fn select_detailed_runs(
    summaries: &[super::summary::RunSummary],
    output: &mut [u32],
) -> Result<usize, SelectionError> {
    if summaries.is_empty() {
        return Err(SelectionError::Empty);
    }
    let mut index = 0usize;
    while index < summaries.len() {
        if summaries[index].run_index != index as u32 {
            return Err(SelectionError::RunOrder);
        }
        index += 1;
    }
    let mut insertion = summaries[0];
    let mut load = summaries[0];
    let mut navigation = summaries[0];
    let mut first_failure = None;
    for summary in summaries {
        if summary.cutoff_radius_q12 < insertion.cutoff_radius_q12 {
            insertion = *summary;
        }
        if summary.max_dynamic_pressure_q16 > load.max_dynamic_pressure_q16 {
            load = *summary;
        }
        if summary.navigation_position_error_q12 > navigation.navigation_position_error_q12 {
            navigation = *summary;
        }
        if first_failure.is_none() && summary.outcome != super::summary::RunOutcome::StableOrbit {
            first_failure = Some(*summary);
        }
    }
    let candidates = [
        Some(summaries[0]),
        Some(insertion),
        Some(load),
        Some(navigation),
        first_failure,
    ];
    let mut count = 0usize;
    for candidate in candidates.into_iter().flatten() {
        push_run(output, &mut count, candidate.run_index);
    }
    for summary in summaries {
        push_run(output, &mut count, summary.run_index);
    }
    Ok(count)
}

fn push_run(output: &mut [u32], count: &mut usize, run: u32) {
    if *count == output.len() || output[..*count].contains(&run) {
        return;
    }
    output[*count] = run;
    *count += 1;
}
