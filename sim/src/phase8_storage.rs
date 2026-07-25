//! Capacity-scaled Phase 8 summary and history retention plans.
use ksa64_core::phase8_format::{
    KPH8_HEADER_LENGTH, KPH8_POINT_LENGTH, KSR8_LENGTH, KST8_FRAME_LENGTH, KST8_HEADER_LENGTH,
};
pub const SUPPORTED_REU_KIB: [u32; 8] = [128, 256, 512, 1_024, 2_048, 4_096, 8_192, 16_384];
pub const STOCK_INTERESTING_SUMMARIES: u32 = 5;
const ARCHIVE_OVERHEAD: u32 = 512;
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
pub struct Phase8StoragePlan {
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
    Overflow,
}
fn supported(v: u32) -> bool {
    v == 0 || SUPPORTED_REU_KIB.contains(&v)
}
fn history(header: usize, stride: usize, count: u32) -> Result<u32, StoragePlanError> {
    (header as u32)
        .checked_add(
            (stride as u32)
                .checked_mul(count)
                .ok_or(StoragePlanError::Overflow)?,
        )
        .and_then(|v| v.checked_add(4))
        .ok_or(StoragePlanError::Overflow)
}
impl Phase8StoragePlan {
    pub fn compute(
        detected_kib: u32,
        preference: ReuPreference,
        run_count: u32,
        detail_frames: u32,
        compact_points: u32,
    ) -> Result<Self, StoragePlanError> {
        if !supported(detected_kib) {
            return Err(StoragePlanError::UnsupportedCapacity);
        }
        let effective = match preference {
            ReuPreference::Disabled => 0,
            ReuPreference::Auto => detected_kib,
            ReuPreference::CapKiB(cap) => detected_kib.min(cap),
        };
        let compact = history(KPH8_HEADER_LENGTH, KPH8_POINT_LENGTH, compact_points)?;
        if effective == 0 {
            return Ok(Self {
                mode: StorageMode::Stock,
                detected_kib,
                effective_kib: 0,
                summary_slots: run_count.min(STOCK_INTERESTING_SUMMARIES),
                full_histories: 0,
                compact_histories: u32::from(compact_points > 0),
                used_bytes: run_count.min(STOCK_INTERESTING_SUMMARIES) * KSR8_LENGTH as u32
                    + compact,
                free_bytes: 0,
            });
        }
        let capacity = effective
            .checked_mul(1024)
            .ok_or(StoragePlanError::Overflow)?;
        let summaries = run_count.min((capacity / 4) / (KSR8_LENGTH as u32));
        let mut used = ARCHIVE_OVERHEAD
            .checked_add(summaries * KSR8_LENGTH as u32)
            .ok_or(StoragePlanError::Overflow)?;
        if used > capacity {
            return Err(StoragePlanError::Overflow);
        }
        let full = history(KST8_HEADER_LENGTH, KST8_FRAME_LENGTH, detail_frames)? + 8;
        let compact = compact + 8;
        let full_count = ((capacity - used) / full).min(run_count);
        used += full_count * full;
        let compact_count = ((capacity - used) / compact).min(run_count.saturating_sub(full_count));
        used += compact_count * compact;
        Ok(Self {
            mode: StorageMode::Reu,
            detected_kib,
            effective_kib: effective,
            summary_slots: summaries,
            full_histories: full_count,
            compact_histories: compact_count,
            used_bytes: used,
            free_bytes: capacity - used,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stock_and_every_reu_tier_are_bounded() {
        let stock = Phase8StoragePlan::compute(0, ReuPreference::Auto, 1024, 1000, 82).unwrap();
        assert_eq!(stock.summary_slots, 5);
        for size in SUPPORTED_REU_KIB {
            let plan =
                Phase8StoragePlan::compute(size, ReuPreference::Auto, 1024, 1000, 82).unwrap();
            assert!(plan.used_bytes <= size * 1024);
            assert!(plan.summary_slots >= 128);
        }
        assert_eq!(
            Phase8StoragePlan::compute(128, ReuPreference::Disabled, 1024, 1000, 82)
                .unwrap()
                .mode,
            StorageMode::Stock
        );
        assert_eq!(
            Phase8StoragePlan::compute(1024, ReuPreference::CapKiB(256), 1024, 1000, 82)
                .unwrap()
                .effective_kib,
            256
        );
    }
}
