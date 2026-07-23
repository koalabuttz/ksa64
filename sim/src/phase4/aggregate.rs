//! Allocation-free streaming Phase 4 campaign statistics.

use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;

use super::contracts::RUN_SUMMARY_LENGTH;
use super::summary::{write_ksr4, RunOutcome, RunSummary};

pub const HISTOGRAM_BINS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamingMetric {
    pub count: u32,
    pub minimum: i32,
    pub maximum: i32,
    mean_q16: i64,
    m2_q16: i64,
}
impl StreamingMetric {
    pub const EMPTY: Self = Self {
        count: 0,
        minimum: i32::MAX,
        maximum: i32::MIN,
        mean_q16: 0,
        m2_q16: 0,
    };
    pub fn update(&mut self, value: i32) {
        self.count = self.count.saturating_add(1);
        self.minimum = self.minimum.min(value);
        self.maximum = self.maximum.max(value);
        let value_q16 = (value as i64) << 16;
        let delta = value_q16 - self.mean_q16;
        self.mean_q16 += delta / self.count as i64;
        let delta_after = value_q16 - self.mean_q16;
        self.m2_q16 = self
            .m2_q16
            .saturating_add((delta.saturating_mul(delta_after)) >> 16);
    }
    pub const fn mean_q16(self) -> i64 {
        self.mean_q16
    }
    pub fn mean(self) -> i32 {
        (self.mean_q16 >> 16).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
    pub fn sample_variance(self) -> i64 {
        if self.count < 2 {
            0
        } else {
            (self.m2_q16 / (self.count - 1) as i64) >> 16
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignAggregate {
    pub run_count: u32,
    pub outcome_counts: [u32; RunOutcome::COUNT],
    pub cutoff_altitude_km: StreamingMetric,
    pub max_dynamic_pressure_kpa: StreamingMetric,
    pub max_proper_acceleration_mps2: StreamingMetric,
    pub navigation_position_error_m: StreamingMetric,
    pub insertion_histogram: [u32; HISTOGRAM_BINS],
    pub summary_chain: u32,
}
impl CampaignAggregate {
    pub const fn new() -> Self {
        Self {
            run_count: 0,
            outcome_counts: [0; RunOutcome::COUNT],
            cutoff_altitude_km: StreamingMetric::EMPTY,
            max_dynamic_pressure_kpa: StreamingMetric::EMPTY,
            max_proper_acceleration_mps2: StreamingMetric::EMPTY,
            navigation_position_error_m: StreamingMetric::EMPTY,
            insertion_histogram: [0; HISTOGRAM_BINS],
            summary_chain: 2_166_136_261,
        }
    }
    pub fn update(&mut self, summary: &RunSummary) {
        self.run_count = self.run_count.saturating_add(1);
        self.outcome_counts[summary.outcome as usize] =
            self.outcome_counts[summary.outcome as usize].saturating_add(1);
        let altitude = (summary.cutoff_radius_q12 - EARTH_RADIUS_Q12) / 4096;
        let max_q = summary.max_dynamic_pressure_q16 / 65_536;
        let max_proper = ((summary.max_proper_acceleration_q28 as i64 * 1_000) >> 28)
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let nav_m = ((summary.navigation_position_error_q12 as i64 * 1_000) >> 12)
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        self.cutoff_altitude_km.update(altitude);
        self.max_dynamic_pressure_kpa.update(max_q);
        self.max_proper_acceleration_mps2.update(max_proper);
        self.navigation_position_error_m.update(nav_m);
        let bin = histogram_bin(altitude, 100_000, 300_000);
        self.insertion_histogram[bin] = self.insertion_histogram[bin].saturating_add(1);
        let mut bytes = [0u8; RUN_SUMMARY_LENGTH];
        if write_ksr4(summary, &mut bytes).is_ok() {
            self.summary_chain = rolling_hash(self.summary_chain, &bytes);
        }
    }
    pub const fn success_count(self) -> u32 {
        self.outcome_counts[RunOutcome::StableOrbit as usize]
    }
}
impl Default for CampaignAggregate {
    fn default() -> Self {
        Self::new()
    }
}

fn histogram_bin(value: i32, minimum: i32, maximum: i32) -> usize {
    if value <= minimum {
        return 0;
    }
    if value >= maximum {
        return HISTOGRAM_BINS - 1;
    }
    let span = (maximum - minimum) as i64;
    (((value - minimum) as i64 * HISTOGRAM_BINS as i64) / span) as usize
}
fn rolling_hash(mut hash: u32, bytes: &[u8]) -> u32 {
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}
