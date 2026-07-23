//! Strict allocation-free Phase 5 KPH5 mission-control replay.

use crate::phase5_history::{
    parse_kph5_point, validate_kph5, Phase5HistoryError, Phase5HistoryPoint, KPH5_HEADER_LENGTH,
    KPH5_POINT_LENGTH,
};
use ksa64_interface::phase5::{
    EVENT_ABORT, EVENT_CUTOFF, EVENT_GIMBAL_JAMMED, EVENT_IGNITION, EVENT_RCS_DEPLETED,
    EVENT_SEPARATION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5ReplayError {
    History(Phase5HistoryError),
    Identity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5ReplaySummary {
    pub points: u16,
    pub final_step: u32,
    pub final_position_quarter_km: [i16; 3],
    pub max_dynamic_pressure_sixteenth_kpa: u16,
    pub max_navigation_error_quarter_km: u16,
    pub observed_events: u16,
    pub observed_alarms: u16,
    pub cue_counts: [u16; 5],
    pub cue_hash: u32,
}

pub fn replay_phase5_history(
    tape: &[u8],
    expected_campaign_seed: u32,
    expected_run_index: u32,
) -> Result<Phase5ReplaySummary, Phase5ReplayError> {
    let header = validate_kph5(tape).map_err(Phase5ReplayError::History)?;
    if header.campaign_seed != expected_campaign_seed || header.run_index != expected_run_index {
        return Err(Phase5ReplayError::Identity);
    }
    let mut out = Phase5ReplaySummary {
        points: header.point_count,
        final_step: 0,
        final_position_quarter_km: [0; 3],
        max_dynamic_pressure_sixteenth_kpa: 0,
        max_navigation_error_quarter_km: 0,
        observed_events: 0,
        observed_alarms: 0,
        cue_counts: [0; 5],
        cue_hash: 2_166_136_261,
    };
    let bits = [
        EVENT_IGNITION,
        EVENT_CUTOFF,
        EVENT_SEPARATION,
        EVENT_ABORT,
        EVENT_RCS_DEPLETED | EVENT_GIMBAL_JAMMED,
    ];
    let mut index = 0usize;
    while index < header.point_count as usize {
        let at = KPH5_HEADER_LENGTH + index * KPH5_POINT_LENGTH;
        let p = parse_kph5_point(&tape[at..at + KPH5_POINT_LENGTH])
            .map_err(Phase5ReplayError::History)?;
        out.final_step = p.step as u32;
        out.final_position_quarter_km = p.position_quarter_km;
        out.max_dynamic_pressure_sixteenth_kpa = out
            .max_dynamic_pressure_sixteenth_kpa
            .max(p.dynamic_pressure_sixteenth_kpa);
        out.max_navigation_error_quarter_km = out
            .max_navigation_error_quarter_km
            .max(p.navigation_error_quarter_km);
        out.observed_events |= p.events;
        out.observed_alarms |= p.alarms;
        let mut cue = 0usize;
        while cue < bits.len() {
            if p.events & bits[cue] != 0 {
                out.cue_counts[cue] = out.cue_counts[cue].saturating_add(1);
                out.cue_hash = hash_word(out.cue_hash, ((index as u32) << 8) | cue as u32)
            }
            cue += 1
        }
        index += 1;
    }
    Ok(out)
}

pub fn phase5_plot_coordinate(point: Phase5HistoryPoint) -> (u8, u8) {
    let column = (i32::from(point.position_quarter_km[1]).clamp(0, 4095) >> 7).min(31) as u8 + 4;
    let height =
        ((i32::from(point.position_quarter_km[2]) - 12_000).clamp(0, 4095) >> 8).min(15) as u8;
    (column, 20 - height)
}
fn hash_word(mut hash: u32, word: u32) -> u32 {
    let mut shift = 0;
    while shift < 32 {
        hash ^= (word >> shift) & 0xff;
        hash = hash.wrapping_mul(16_777_619);
        shift += 8
    }
    hash
}
