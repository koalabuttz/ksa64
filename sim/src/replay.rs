//! Strict allocation-free Phase 3 replay tape (`KRP3`) reader.

use ksa64_interface::{
    crc32_ieee, EVENT_ABORT, EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_SEPARATION,
};

pub const REPLAY_HEADER_LENGTH: usize = 32;
pub const REPLAY_FRAME_LENGTH: usize = 24;
const MAGIC: [u8; 4] = *b"KRP3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    Length,
    Magic,
    Version,
    Reserved,
    Identity,
    Frame { index: u32 },
    Order { index: u32 },
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplaySummary {
    pub source_stream_crc32: u32,
    pub scenario_id: u32,
    pub config_crc32: u32,
    pub frames: u32,
    pub final_step: u32,
    pub final_altitude_q12: i32,
    pub final_downrange_q32: i32,
    pub final_pitch: u16,
    pub final_mode: u8,
    pub final_stage: u8,
    pub observed_events: u16,
    pub observed_alarms: u16,
    pub cue_counts: [u16; 5],
    pub cue_hash: u32,
}

fn u16_at(input: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([input[at], input[at + 1]])
}
fn u32_at(input: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([input[at], input[at + 1], input[at + 2], input[at + 3]])
}
fn i32_at(input: &[u8], at: usize) -> i32 {
    u32_at(input, at) as i32
}
fn hash_word(mut hash: u32, word: u32) -> u32 {
    let mut shift = 0;
    while shift < 32 {
        hash ^= (word >> shift) & 0xff;
        hash = hash.wrapping_mul(16_777_619);
        shift += 8;
    }
    hash
}

pub fn replay_phase3_tape(
    tape: &[u8],
    expected_scenario_id: u32,
    expected_config_crc32: u32,
) -> Result<ReplaySummary, ReplayError> {
    if tape.len() < REPLAY_HEADER_LENGTH {
        return Err(ReplayError::Length);
    }
    if tape[..4] != MAGIC {
        return Err(ReplayError::Magic);
    }
    if u16_at(tape, 4) != 3
        || u16_at(tape, 6) as usize != REPLAY_HEADER_LENGTH
        || u16_at(tape, 8) as usize != REPLAY_FRAME_LENGTH
    {
        return Err(ReplayError::Version);
    }
    if u16_at(tape, 10) != 0 || crc32_ieee(&tape[..28]) != u32_at(tape, 28) {
        return Err(ReplayError::Reserved);
    }
    let frames = u32_at(tape, 24);
    if tape.len() != REPLAY_HEADER_LENGTH + frames as usize * REPLAY_FRAME_LENGTH {
        return Err(ReplayError::Length);
    }
    if u32_at(tape, 16) != expected_scenario_id || u32_at(tape, 20) != expected_config_crc32 {
        return Err(ReplayError::Identity);
    }
    let mut summary = ReplaySummary {
        source_stream_crc32: u32_at(tape, 12),
        scenario_id: u32_at(tape, 16),
        config_crc32: u32_at(tape, 20),
        frames,
        final_step: 0,
        final_altitude_q12: 0,
        final_downrange_q32: 0,
        final_pitch: 0,
        final_mode: 0,
        final_stage: 0,
        observed_events: 0,
        observed_alarms: 0,
        cue_counts: [0; 5],
        cue_hash: 2_166_136_261,
    };
    let mut index = 0u32;
    while index < frames {
        let at = REPLAY_HEADER_LENGTH + index as usize * REPLAY_FRAME_LENGTH;
        let frame = &tape[at..at + REPLAY_FRAME_LENGTH];
        if crc32_ieee(&frame[..20]) != u32_at(frame, 20) || frame[14] > 7 || frame[15] > 3 {
            return Err(ReplayError::Frame { index });
        }
        let step = u32_at(frame, 0);
        if index > 0 && step <= summary.final_step {
            return Err(ReplayError::Order { index });
        }
        let events = u16_at(frame, 16);
        let alarms = u16_at(frame, 18);
        let cue_bits = [
            EVENT_IGNITION,
            EVENT_CUTOFF,
            EVENT_SEPARATION,
            EVENT_END,
            EVENT_ABORT,
        ];
        let mut cue = 0;
        while cue < cue_bits.len() {
            if events & cue_bits[cue] != 0 {
                summary.cue_counts[cue] = summary.cue_counts[cue].saturating_add(1);
                summary.cue_hash = hash_word(summary.cue_hash, (index << 8) | cue as u32);
            }
            cue += 1;
        }
        summary.final_step = step;
        summary.final_altitude_q12 = i32_at(frame, 4);
        summary.final_downrange_q32 = i32_at(frame, 8);
        summary.final_pitch = u16_at(frame, 12);
        summary.final_mode = frame[14];
        summary.final_stage = frame[15];
        summary.observed_events |= events;
        summary.observed_alarms |= alarms;
        index += 1;
    }
    if frames == 0 || summary.observed_events & EVENT_END == 0 {
        return Err(ReplayError::Terminal);
    }
    Ok(summary)
}
