//! Adaptive, observational Phase 5 history retention (`KPH5`).

use crate::phase4::storage::{ReuPreference, StorageMode, SUPPORTED_REU_KIB};
use crate::phase5_campaign::{
    Phase5CampaignAggregate, Phase5CampaignSink, Phase5RunSummary, KSR5_LENGTH,
};
use crate::phase5_closed_loop::Phase5ClosedLoopStep;
use crate::phase5_mission::{Phase5MissionCase, Phase5MissionObserver, Phase5MissionOutcome};
use crate::phase5_telemetry::{PHASE5_TELEMETRY_FRAME_LENGTH, PHASE5_TELEMETRY_HEADER_LENGTH};
use crate::phase5_vehicle::Phase5VehicleSnapshot;
use ksa64_interface::crc32_ieee;

pub const KPH5_VERSION: u16 = 5;
pub const KPH5_CONTRACT_ID: u32 = 0x050c_0001;
pub const KPH5_HEADER_LENGTH: usize = 80;
pub const KPH5_POINT_LENGTH: usize = 16;
pub const STOCK_HISTORY_POINTS: usize = 128;
pub const STOCK_HISTORY_STRIDE: u16 = 32;
pub const REU_HISTORY_STRIDE: u16 = 8;
pub const STOCK_INTERESTING_SUMMARIES: usize = 5;
pub const ARCHIVE_SUPERBLOCK_BYTES: u32 = 256;
pub const ARCHIVE_RECORD_HEADER_BYTES: u32 = 32;
pub const AGGREGATE_BYTES: u32 = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Phase5HistoryPoint {
    pub step: u16,
    pub position_quarter_km: [i16; 3],
    pub dynamic_pressure_sixteenth_kpa: u16,
    pub navigation_error_quarter_km: u16,
    pub events: u16,
    pub alarms: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5HistoryHeader {
    pub campaign_seed: u32,
    pub run_index: u32,
    pub sensor_seed: u32,
    pub variation_checksum: u32,
    pub stride: u16,
    pub point_count: u16,
    pub terminal_step: u32,
    pub points_crc32: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5HistoryError {
    Length,
    Identity,
    Reserved,
    Checksum,
    Capacity,
    Stride,
    Sequence,
}

pub fn write_kph5_header(
    header: Phase5HistoryHeader,
    out: &mut [u8],
) -> Result<(), Phase5HistoryError> {
    if out.len() != KPH5_HEADER_LENGTH {
        return Err(Phase5HistoryError::Length);
    }
    if header.stride == 0 || header.point_count == 0 {
        return Err(Phase5HistoryError::Stride);
    }
    out.fill(0);
    out[..4].copy_from_slice(b"KPH5");
    pu16(out, 4, KPH5_VERSION);
    pu16(out, 6, KPH5_HEADER_LENGTH as u16);
    pu16(out, 8, KPH5_POINT_LENGTH as u16);
    pu16(out, 10, header.stride);
    pu32(out, 12, KPH5_CONTRACT_ID);
    pu32(
        out,
        16,
        ksa64_core::phase5_contract::PHASE5_NUMERIC_CONTRACT_ID,
    );
    pu32(out, 20, ksa64_core::phase5_contract::PHASE5_SCENARIO_ID);
    pu32(out, 24, header.campaign_seed);
    pu32(out, 28, header.run_index);
    pu32(out, 32, header.sensor_seed);
    pu32(out, 36, header.variation_checksum);
    pu16(out, 40, header.point_count);
    pu32(out, 44, header.terminal_step);
    pu32(out, 72, header.points_crc32);
    pu32(out, 76, crc32_ieee(&out[..76]));
    Ok(())
}

pub fn parse_kph5_header(input: &[u8]) -> Result<Phase5HistoryHeader, Phase5HistoryError> {
    if input.len() != KPH5_HEADER_LENGTH {
        return Err(Phase5HistoryError::Length);
    }
    if &input[..4] != b"KPH5"
        || gu16(input, 4) != KPH5_VERSION
        || gu16(input, 6) != KPH5_HEADER_LENGTH as u16
        || gu16(input, 8) != KPH5_POINT_LENGTH as u16
        || gu32(input, 12) != KPH5_CONTRACT_ID
        || gu32(input, 16) != ksa64_core::phase5_contract::PHASE5_NUMERIC_CONTRACT_ID
        || gu32(input, 20) != ksa64_core::phase5_contract::PHASE5_SCENARIO_ID
    {
        return Err(Phase5HistoryError::Identity);
    }
    if input[42..44]
        .iter()
        .chain(input[48..72].iter())
        .any(|&v| v != 0)
    {
        return Err(Phase5HistoryError::Reserved);
    }
    if crc32_ieee(&input[..76]) != gu32(input, 76) {
        return Err(Phase5HistoryError::Checksum);
    }
    let header = Phase5HistoryHeader {
        campaign_seed: gu32(input, 24),
        run_index: gu32(input, 28),
        sensor_seed: gu32(input, 32),
        variation_checksum: gu32(input, 36),
        stride: gu16(input, 10),
        point_count: gu16(input, 40),
        terminal_step: gu32(input, 44),
        points_crc32: gu32(input, 72),
    };
    if header.stride == 0 || header.point_count == 0 {
        return Err(Phase5HistoryError::Stride);
    }
    Ok(header)
}

pub fn write_kph5_point(
    point: Phase5HistoryPoint,
    out: &mut [u8],
) -> Result<(), Phase5HistoryError> {
    if out.len() != KPH5_POINT_LENGTH {
        return Err(Phase5HistoryError::Length);
    }
    pu16(out, 0, point.step);
    for (i, value) in point.position_quarter_km.iter().enumerate() {
        pu16(out, 2 + i * 2, *value as u16);
    }
    pu16(out, 8, point.dynamic_pressure_sixteenth_kpa);
    pu16(out, 10, point.navigation_error_quarter_km);
    pu16(out, 12, point.events);
    pu16(out, 14, point.alarms);
    Ok(())
}

pub fn write_kph5(
    mut header: Phase5HistoryHeader,
    points: &[Phase5HistoryPoint],
    out: &mut [u8],
) -> Result<(), Phase5HistoryError> {
    let expected = KPH5_HEADER_LENGTH
        .checked_add(
            points
                .len()
                .checked_mul(KPH5_POINT_LENGTH)
                .ok_or(Phase5HistoryError::Capacity)?,
        )
        .ok_or(Phase5HistoryError::Capacity)?;
    if out.len() != expected || points.is_empty() || points.len() > u16::MAX as usize {
        return Err(Phase5HistoryError::Length);
    }
    if points[0].step != 0 || points.last().map(|p| p.step as u32) != Some(header.terminal_step) {
        return Err(Phase5HistoryError::Sequence);
    }
    for (index, point) in points.iter().enumerate() {
        if index != 0 && point.step <= points[index - 1].step {
            return Err(Phase5HistoryError::Sequence);
        }
        let start = KPH5_HEADER_LENGTH + index * KPH5_POINT_LENGTH;
        write_kph5_point(*point, &mut out[start..start + KPH5_POINT_LENGTH])?;
    }
    header.point_count = points.len() as u16;
    header.points_crc32 = crc32_ieee(&out[KPH5_HEADER_LENGTH..]);
    write_kph5_header(header, &mut out[..KPH5_HEADER_LENGTH])
}

pub fn validate_kph5(input: &[u8]) -> Result<Phase5HistoryHeader, Phase5HistoryError> {
    if input.len() < KPH5_HEADER_LENGTH {
        return Err(Phase5HistoryError::Length);
    }
    let header = parse_kph5_header(&input[..KPH5_HEADER_LENGTH])?;
    let expected = KPH5_HEADER_LENGTH
        .checked_add(header.point_count as usize * KPH5_POINT_LENGTH)
        .ok_or(Phase5HistoryError::Capacity)?;
    if input.len() != expected || crc32_ieee(&input[KPH5_HEADER_LENGTH..]) != header.points_crc32 {
        return Err(Phase5HistoryError::Checksum);
    }
    let mut previous = None;
    for index in 0..header.point_count as usize {
        let start = KPH5_HEADER_LENGTH + index * KPH5_POINT_LENGTH;
        let point = parse_kph5_point(&input[start..start + KPH5_POINT_LENGTH])?;
        if previous.is_none() && point.step != 0 {
            return Err(Phase5HistoryError::Sequence);
        }
        if previous.is_some_and(|value| point.step <= value) {
            return Err(Phase5HistoryError::Sequence);
        }
        previous = Some(point.step);
    }
    if previous.map(u32::from) != Some(header.terminal_step) {
        return Err(Phase5HistoryError::Sequence);
    }
    Ok(header)
}
pub fn parse_kph5_point(input: &[u8]) -> Result<Phase5HistoryPoint, Phase5HistoryError> {
    if input.len() != KPH5_POINT_LENGTH {
        return Err(Phase5HistoryError::Length);
    }
    Ok(Phase5HistoryPoint {
        step: gu16(input, 0),
        position_quarter_km: [
            gu16(input, 2) as i16,
            gu16(input, 4) as i16,
            gu16(input, 6) as i16,
        ],
        dynamic_pressure_sixteenth_kpa: gu16(input, 8),
        navigation_error_quarter_km: gu16(input, 10),
        events: gu16(input, 12),
        alarms: gu16(input, 14),
    })
}

pub struct Phase5HistoryRecorder<const N: usize> {
    stride: u16,
    points: [Phase5HistoryPoint; N],
    count: usize,
    events_since_sample: u16,
    alarms_since_sample: u16,
}
impl<const N: usize> Phase5HistoryRecorder<N> {
    pub const fn new(stride: u16) -> Self {
        Self {
            stride,
            points: [Phase5HistoryPoint {
                step: 0,
                position_quarter_km: [0; 3],
                dynamic_pressure_sixteenth_kpa: 0,
                navigation_error_quarter_km: 0,
                events: 0,
                alarms: 0,
            }; N],
            count: 0,
            events_since_sample: 0,
            alarms_since_sample: 0,
        }
    }
    pub fn points(&self) -> &[Phase5HistoryPoint] {
        &self.points[..self.count]
    }
    pub const fn count(&self) -> usize {
        self.count
    }
    pub const fn stride(&self) -> u16 {
        self.stride
    }
    fn push(&mut self, point: Phase5HistoryPoint) -> Result<(), Phase5HistoryError> {
        if self.stride == 0 {
            return Err(Phase5HistoryError::Stride);
        }
        if self.count == N {
            return Err(Phase5HistoryError::Capacity);
        }
        self.points[self.count] = point;
        self.count += 1;
        Ok(())
    }
}
impl<const N: usize> Phase5MissionObserver for Phase5HistoryRecorder<N> {
    type Error = Phase5HistoryError;
    fn observe_initial(
        &mut self,
        _case: Phase5MissionCase,
        _seed: u32,
        snapshot: Phase5VehicleSnapshot,
    ) -> Result<(), Self::Error> {
        self.push(point_from_snapshot(snapshot, 0, 0, 0))
    }
    fn observe_step(
        &mut self,
        _case: Phase5MissionCase,
        step: Phase5ClosedLoopStep,
        terminal: bool,
    ) -> Result<(), Self::Error> {
        let n = step.vehicle.truth.step();
        self.events_since_sample |= step.vehicle.events;
        self.alarms_since_sample |= step.flight.alarms;
        if terminal || n.checked_rem(self.stride as u32) == Some(0) {
            let point = point_from_snapshot(
                step.vehicle,
                self.events_since_sample,
                self.alarms_since_sample,
                nav_error_quarter_km(step),
            );
            self.events_since_sample = 0;
            self.alarms_since_sample = 0;
            if terminal && self.count != 0 && self.points[self.count - 1].step == n as u16 {
                self.points[self.count - 1] = point;
                Ok(())
            } else {
                self.push(point)
            }
        } else {
            Ok(())
        }
    }
}
fn point_from_snapshot(
    snapshot: Phase5VehicleSnapshot,
    events: u16,
    alarms: u16,
    navigation_error_quarter_km: u16,
) -> Phase5HistoryPoint {
    let p = snapshot.truth.spatial().position();
    Phase5HistoryPoint {
        step: snapshot.truth.step().min(u16::MAX as u32) as u16,
        position_quarter_km: [
            position_quarter_km(p.x()),
            position_quarter_km(p.y()),
            position_quarter_km(p.z()),
        ],
        dynamic_pressure_sixteenth_kpa: (snapshot.dynamic_pressure_q16.max(0) as u32 >> 12)
            .min(u16::MAX as u32) as u16,
        navigation_error_quarter_km,
        events,
        alarms,
    }
}
fn position_quarter_km(q12_metres: i32) -> i16 {
    (q12_metres / 1024).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn nav_error_quarter_km(step: Phase5ClosedLoopStep) -> u16 {
    let p = step.vehicle.truth.spatial().position();
    let nav = step.flight.navigation.position_q12;
    let mut l1 = 0u32;
    for (truth, estimate) in [(p.x(), nav[0]), (p.y(), nav[1]), (p.z(), nav[2])] {
        l1 = l1.saturating_add(truth.saturating_sub(estimate).saturating_abs() as u32);
    }
    (l1 / 1024).min(u16::MAX as u32) as u16
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5StockSnapshot {
    pub aggregate: Phase5CampaignAggregate,
    pub retained: [Phase5RunSummary; STOCK_INTERESTING_SUMMARIES],
}
pub struct Phase5StockRetention {
    next_index: u32,
    aggregate: Phase5CampaignAggregate,
    baseline: Option<Phase5RunSummary>,
    insertion: Option<Phase5RunSummary>,
    load: Option<Phase5RunSummary>,
    navigation: Option<Phase5RunSummary>,
    first_failure: Option<Phase5RunSummary>,
    lowest: [Option<Phase5RunSummary>; STOCK_INTERESTING_SUMMARIES],
}
impl Phase5StockRetention {
    pub const fn new() -> Self {
        Self {
            next_index: 0,
            aggregate: Phase5CampaignAggregate::new(),
            baseline: None,
            insertion: None,
            load: None,
            navigation: None,
            first_failure: None,
            lowest: [None; STOCK_INTERESTING_SUMMARIES],
        }
    }
    pub fn observe(&mut self, summary: Phase5RunSummary) -> Result<(), Phase5HistoryError> {
        if summary.run_index != self.next_index {
            return Err(Phase5HistoryError::Sequence);
        }
        self.next_index += 1;
        self.aggregate.update(&summary);
        if summary.run_index == 0 {
            self.baseline = Some(summary);
        }
        replace_if(&mut self.insertion, summary, |a, b| {
            a.mission.perigee_altitude_q12 < b.mission.perigee_altitude_q12
                || (a.mission.perigee_altitude_q12 == b.mission.perigee_altitude_q12
                    && a.run_index < b.run_index)
        });
        replace_if(&mut self.load, summary, |a, b| {
            a.mission.max_dynamic_pressure_q16 > b.mission.max_dynamic_pressure_q16
                || (a.mission.max_dynamic_pressure_q16 == b.mission.max_dynamic_pressure_q16
                    && a.run_index < b.run_index)
        });
        replace_if(&mut self.navigation, summary, |a, b| {
            a.mission.max_nav_position_error_q12 > b.mission.max_nav_position_error_q12
                || (a.mission.max_nav_position_error_q12 == b.mission.max_nav_position_error_q12
                    && a.run_index < b.run_index)
        });
        if self.first_failure.is_none()
            && summary.mission.outcome != Phase5MissionOutcome::StableOrbit
        {
            self.first_failure = Some(summary);
        }
        if (summary.run_index as usize) < self.lowest.len() {
            self.lowest[summary.run_index as usize] = Some(summary);
        }
        Ok(())
    }
    pub fn finish(self) -> Result<Phase5StockSnapshot, Phase5HistoryError> {
        let baseline = self.baseline.ok_or(Phase5HistoryError::Sequence)?;
        let mut retained = [baseline; STOCK_INTERESTING_SUMMARIES];
        let mut count = 0usize;
        for s in [
            self.baseline,
            self.insertion,
            self.load,
            self.navigation,
            self.first_failure,
        ]
        .into_iter()
        .flatten()
        {
            push_unique(&mut retained, &mut count, s);
        }
        for s in self.lowest.into_iter().flatten() {
            push_unique(&mut retained, &mut count, s);
        }
        if count != retained.len() {
            return Err(Phase5HistoryError::Sequence);
        }
        Ok(Phase5StockSnapshot {
            aggregate: self.aggregate,
            retained,
        })
    }
}
impl Phase5CampaignSink for Phase5StockRetention {
    type Error = Phase5HistoryError;
    fn observe(&mut self, summary: &Phase5RunSummary) -> Result<(), Self::Error> {
        Phase5StockRetention::observe(self, *summary)
    }
}
impl Default for Phase5StockRetention {
    fn default() -> Self {
        Self::new()
    }
}
fn replace_if<F: FnOnce(&Phase5RunSummary, &Phase5RunSummary) -> bool>(
    slot: &mut Option<Phase5RunSummary>,
    candidate: Phase5RunSummary,
    better: F,
) {
    match slot {
        Some(current) if !better(&candidate, current) => {}
        _ => *slot = Some(candidate),
    }
}
fn push_unique(
    output: &mut [Phase5RunSummary; STOCK_INTERESTING_SUMMARIES],
    count: &mut usize,
    candidate: Phase5RunSummary,
) {
    if *count < output.len()
        && !output[..*count]
            .iter()
            .any(|s| s.run_index == candidate.run_index)
    {
        output[*count] = candidate;
        *count += 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5StoragePlan {
    pub mode: StorageMode,
    pub detected_kib: u32,
    pub effective_kib: u32,
    pub summary_slots: u32,
    pub full_histories: u32,
    pub compact_histories: u32,
    pub used_bytes: u32,
    pub free_bytes: u32,
}
impl Phase5StoragePlan {
    pub fn compute(
        detected_kib: u32,
        preference: ReuPreference,
        run_count: u32,
        full_frame_count: u32,
        compact_point_count: u32,
    ) -> Result<Self, Phase5HistoryError> {
        if detected_kib != 0 && !SUPPORTED_REU_KIB.contains(&detected_kib) {
            return Err(Phase5HistoryError::Capacity);
        }
        let effective_kib = match preference {
            ReuPreference::Disabled => 0,
            ReuPreference::Auto => detected_kib,
            ReuPreference::CapKiB(cap) => detected_kib.min(cap),
        };
        let compact_bytes = history_bytes(compact_point_count)?;
        if effective_kib == 0 {
            return Ok(Self {
                mode: StorageMode::Stock,
                detected_kib,
                effective_kib,
                summary_slots: run_count.min(STOCK_INTERESTING_SUMMARIES as u32),
                full_histories: 0,
                compact_histories: u32::from(compact_point_count != 0),
                used_bytes: run_count.min(STOCK_INTERESTING_SUMMARIES as u32) * KSR5_LENGTH as u32
                    + compact_bytes,
                free_bytes: 0,
            });
        }
        let capacity = effective_kib
            .checked_shl(10)
            .ok_or(Phase5HistoryError::Capacity)?;
        let summary_budget = capacity >> 2;
        let mut summary_slots = 0u32;
        let mut summary_bytes = 0u32;
        while summary_slots < run_count {
            let next = summary_bytes
                .checked_add(KSR5_LENGTH as u32)
                .ok_or(Phase5HistoryError::Capacity)?;
            if next > summary_budget {
                break;
            }
            summary_bytes = next;
            summary_slots += 1;
        }
        let mut used = ARCHIVE_SUPERBLOCK_BYTES
            .checked_add(AGGREGATE_BYTES)
            .and_then(|v| v.checked_add(summary_bytes))
            .and_then(|v| v.checked_add(3 * ARCHIVE_RECORD_HEADER_BYTES))
            .ok_or(Phase5HistoryError::Capacity)?;
        if used > capacity {
            return Err(Phase5HistoryError::Capacity);
        }
        let full_payload = (PHASE5_TELEMETRY_HEADER_LENGTH as u32)
            .checked_add(
                full_frame_count
                    .checked_mul(PHASE5_TELEMETRY_FRAME_LENGTH as u32)
                    .ok_or(Phase5HistoryError::Capacity)?,
            )
            .ok_or(Phase5HistoryError::Capacity)?;
        let full_cost = full_payload
            .checked_add(ARCHIVE_RECORD_HEADER_BYTES)
            .ok_or(Phase5HistoryError::Capacity)?;
        let compact_cost = compact_bytes
            .checked_add(ARCHIVE_RECORD_HEADER_BYTES)
            .ok_or(Phase5HistoryError::Capacity)?;
        let mut full_histories = 0u32;
        while full_frame_count != 0 && full_histories < run_count {
            let next = used
                .checked_add(full_cost)
                .ok_or(Phase5HistoryError::Capacity)?;
            if next > capacity {
                break;
            }
            used = next;
            full_histories += 1;
        }
        let mut compact_histories = 0u32;
        while compact_point_count != 0 && full_histories + compact_histories < run_count {
            let next = used
                .checked_add(compact_cost)
                .ok_or(Phase5HistoryError::Capacity)?;
            if next > capacity {
                break;
            }
            used = next;
            compact_histories += 1;
        }
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
    pub fn compute_exact(
        detected_kib: u32,
        preference: ReuPreference,
        run_count: u32,
        full_history_bytes: &[u32],
        compact_point_count: u32,
    ) -> Result<Self, Phase5HistoryError> {
        let mut plan = Self::compute(detected_kib, preference, run_count, 0, compact_point_count)?;
        if plan.mode == StorageMode::Stock {
            return Ok(plan);
        }
        let capacity = plan
            .effective_kib
            .checked_mul(1024)
            .ok_or(Phase5HistoryError::Capacity)?;
        let summary_bytes = plan
            .summary_slots
            .checked_mul(KSR5_LENGTH as u32)
            .ok_or(Phase5HistoryError::Capacity)?;
        let mut used = ARCHIVE_SUPERBLOCK_BYTES
            .checked_add(AGGREGATE_BYTES)
            .and_then(|v| v.checked_add(summary_bytes))
            .and_then(|v| v.checked_add(3 * ARCHIVE_RECORD_HEADER_BYTES))
            .ok_or(Phase5HistoryError::Capacity)?;
        let mut full = 0u32;
        for &payload in full_history_bytes.iter().take(run_count as usize) {
            if payload < (PHASE5_TELEMETRY_HEADER_LENGTH + PHASE5_TELEMETRY_FRAME_LENGTH) as u32 {
                return Err(Phase5HistoryError::Length);
            }
            let cost = payload
                .checked_add(ARCHIVE_RECORD_HEADER_BYTES)
                .ok_or(Phase5HistoryError::Capacity)?;
            if cost > capacity - used {
                break;
            }
            used += cost;
            full += 1;
        }
        let compact_cost = history_bytes(compact_point_count)?
            .checked_add(ARCHIVE_RECORD_HEADER_BYTES)
            .ok_or(Phase5HistoryError::Capacity)?;
        let compact = if compact_point_count == 0 {
            0
        } else {
            ((capacity - used) / compact_cost).min(run_count.saturating_sub(full))
        };
        used += compact * compact_cost;
        plan.full_histories = full;
        plan.compact_histories = compact;
        plan.used_bytes = used;
        plan.free_bytes = capacity - used;
        Ok(plan)
    }
}
fn history_bytes(points: u32) -> Result<u32, Phase5HistoryError> {
    (KPH5_HEADER_LENGTH as u32)
        .checked_add(
            points
                .checked_mul(KPH5_POINT_LENGTH as u32)
                .ok_or(Phase5HistoryError::Capacity)?,
        )
        .ok_or(Phase5HistoryError::Capacity)
}

pub fn select_phase5_histories(
    summaries: &[Phase5RunSummary],
    output: &mut [u32],
) -> Result<usize, Phase5HistoryError> {
    if summaries.is_empty() {
        return Err(Phase5HistoryError::Sequence);
    }
    for (i, summary) in summaries.iter().enumerate() {
        if summary.run_index != i as u32 {
            return Err(Phase5HistoryError::Sequence);
        }
    }
    let mut insertion = summaries[0];
    let mut load = summaries[0];
    let mut navigation = summaries[0];
    let mut failure = None;
    for &summary in summaries {
        if summary.mission.perigee_altitude_q12 < insertion.mission.perigee_altitude_q12 {
            insertion = summary;
        }
        if summary.mission.max_dynamic_pressure_q16 > load.mission.max_dynamic_pressure_q16 {
            load = summary;
        }
        if summary.mission.max_nav_position_error_q12
            > navigation.mission.max_nav_position_error_q12
        {
            navigation = summary;
        }
        if failure.is_none() && summary.mission.outcome != Phase5MissionOutcome::StableOrbit {
            failure = Some(summary);
        }
    }
    let mut count = 0usize;
    for s in [
        Some(summaries[0]),
        Some(insertion),
        Some(load),
        Some(navigation),
        failure,
    ]
    .into_iter()
    .flatten()
    {
        push_run(output, &mut count, s.run_index);
    }
    for s in summaries {
        push_run(output, &mut count, s.run_index);
    }
    Ok(count)
}
fn push_run(output: &mut [u32], count: &mut usize, run: u32) {
    if *count < output.len() && !output[..*count].contains(&run) {
        output[*count] = run;
        *count += 1;
    }
}
fn pu16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes())
}
fn pu32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
}
fn gu16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn gu32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
