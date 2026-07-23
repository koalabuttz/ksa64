//! Compact, CRC-bound C64 presentation replay derived from canonical KST2.

use crate::phase2_mission::{
    EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_IMPACT, EVENT_SEPARATION,
};
use crate::phase2_numeric::EARTH_RADIUS_Q12;
use crate::phase2_scenario::Phase2Scenario;
use crate::phase2_telemetry::{
    parse_phase2_telemetry_frame, parse_phase2_telemetry_header_for_scenario, Phase2TelemetryFrame,
    Phase2TelemetryReadError, PHASE2_TELEMETRY_FRAME_LENGTH, PHASE2_TELEMETRY_HEADER_LENGTH,
};

const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR_RAM: *mut u8 = 0xd800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const SID: *mut u8 = 0xd400 as *mut u8;
const COLUMNS: usize = 40;
const CELLS: usize = 1_000;
const REPLAY_HEADER_LENGTH: usize = 40;
const REPLAY_RECORD_LENGTH: usize = 3;
const FNV_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

mod replay_crc {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase2/generated/replay_crc32_v1.rs"
    ));
}

fn replay_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    let mut index = 0usize;
    while index < bytes.len() {
        let table_index = ((crc ^ bytes[index] as u32) & 0xff) as usize;
        crc = (crc >> 8) ^ replay_crc::CRC32_TABLE[table_index];
        index += 1;
    }
    !crc
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2ReplayError {
    Length,
    Magic,
    Version,
    Header,
    Checksum,
    Scenario,
    Telemetry(Phase2TelemetryReadError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SidCue {
    Ignition = 1,
    Cutoff = 2,
    Separation = 3,
    End = 4,
    ImpactAlarm = 5,
}

pub struct Phase2C64ReplaySink {
    telemetry_header: [u8; PHASE2_TELEMETRY_HEADER_LENGTH],
    final_frame: [u8; PHASE2_TELEMETRY_FRAME_LENGTH],
    frames_replayed: u16,
    observed_events: u16,
    cue_count: u16,
    cue_hash: u32,
    cue_counts: [u8; 5],
    source_stream_crc32: u32,
    max_dynamic_pressure_q16: i32,
    perigee_q12: i32,
    apogee_q12: i32,
}

impl Phase2C64ReplaySink {
    pub const fn new() -> Self {
        Self {
            telemetry_header: [0; PHASE2_TELEMETRY_HEADER_LENGTH],
            final_frame: [0; PHASE2_TELEMETRY_FRAME_LENGTH],
            frames_replayed: 0,
            observed_events: 0,
            cue_count: 0,
            cue_hash: FNV_OFFSET,
            cue_counts: [0; 5],
            source_stream_crc32: 0,
            max_dynamic_pressure_q16: 0,
            perigee_q12: 0,
            apogee_q12: 0,
        }
    }

    pub const fn frames_replayed(&self) -> u16 {
        self.frames_replayed
    }

    pub const fn observed_events(&self) -> u16 {
        self.observed_events
    }

    pub const fn cue_count(&self) -> u16 {
        self.cue_count
    }

    pub const fn cue_hash(&self) -> u32 {
        self.cue_hash
    }
    pub const fn source_stream_crc32(&self) -> u32 {
        self.source_stream_crc32
    }

    pub const fn cue_counts(&self) -> [u8; 5] {
        self.cue_counts
    }

    pub fn latest_frame(&self) -> Result<Phase2TelemetryFrame, Phase2TelemetryReadError> {
        parse_phase2_telemetry_frame(&self.final_frame)
    }

    fn add_cue(&mut self, step: u32, cue: SidCue) {
        let mut shift = 0u8;
        while shift < 32 {
            self.cue_hash ^= (step >> shift) & 0xff;
            self.cue_hash = self.cue_hash.wrapping_mul(FNV_PRIME);
            shift += 8;
        }
        self.cue_hash ^= cue as u32;
        self.cue_hash = self.cue_hash.wrapping_mul(FNV_PRIME);
        self.cue_count += 1;
        let index = match cue {
            SidCue::Ignition => 0,
            SidCue::Cutoff => 1,
            SidCue::Separation => 2,
            SidCue::End => 3,
            SidCue::ImpactAlarm => 4,
        };
        self.cue_counts[index] += 1;
        unsafe { play_cue(cue) };
    }

    fn replay_record(&mut self, step: u32, x: u8, y: u8, events: u16) {
        unsafe {
            put(
                12usize.saturating_sub(y.min(10) as usize),
                x.min(39) as usize,
                b'*',
            )
        };
        self.frames_replayed += 1;
        self.observed_events |= events;
        if events & EVENT_IGNITION != 0 {
            self.add_cue(step, SidCue::Ignition);
        }
        if events & EVENT_CUTOFF != 0 {
            self.add_cue(step, SidCue::Cutoff);
        }
        if events & EVENT_SEPARATION != 0 {
            self.add_cue(step, SidCue::Separation);
        }
        if events & EVENT_IMPACT != 0 {
            self.add_cue(step, SidCue::ImpactAlarm);
        }
        if events & EVENT_END != 0 {
            self.add_cue(step, SidCue::End);
        }
    }
}

impl Default for Phase2C64ReplaySink {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

#[inline]
fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

#[inline]
fn read_i32(input: &[u8], offset: usize) -> i32 {
    read_u32(input, offset) as i32
}

pub fn replay_phase2_tape(
    scenario: &Phase2Scenario,
    tape: &[u8],
    sink: &mut Phase2C64ReplaySink,
) -> Result<(), Phase2ReplayError> {
    if tape.len()
        < REPLAY_HEADER_LENGTH + PHASE2_TELEMETRY_HEADER_LENGTH + PHASE2_TELEMETRY_FRAME_LENGTH + 4
    {
        return Err(Phase2ReplayError::Length);
    }
    if &tape[..4] != b"KRP2" {
        return Err(Phase2ReplayError::Magic);
    }
    if read_u16(tape, 4) != 1 {
        return Err(Phase2ReplayError::Version);
    }
    if read_u16(tape, 6) as usize != REPLAY_HEADER_LENGTH
        || read_u16(tape, 8) as usize != REPLAY_RECORD_LENGTH
    {
        return Err(Phase2ReplayError::Header);
    }
    if replay_crc32(&tape[..36]) != read_u32(tape, 36)
        || replay_crc32(&tape[..tape.len() - 4]) != read_u32(tape, tape.len() - 4)
    {
        return Err(Phase2ReplayError::Checksum);
    }
    let count = read_u16(tape, 10) as usize;
    let canonical_offset = REPLAY_HEADER_LENGTH + count * REPLAY_RECORD_LENGTH;
    let expected_length =
        canonical_offset + PHASE2_TELEMETRY_HEADER_LENGTH + PHASE2_TELEMETRY_FRAME_LENGTH + 4;
    if tape.len() != expected_length {
        return Err(Phase2ReplayError::Length);
    }
    if read_u32(tape, 16) != scenario.scenario_id() {
        return Err(Phase2ReplayError::Scenario);
    }
    let telemetry_header =
        &tape[canonical_offset..canonical_offset + PHASE2_TELEMETRY_HEADER_LENGTH];
    let frame_offset = canonical_offset + PHASE2_TELEMETRY_HEADER_LENGTH;
    let final_frame = &tape[frame_offset..frame_offset + PHASE2_TELEMETRY_FRAME_LENGTH];
    let header = parse_phase2_telemetry_header_for_scenario(telemetry_header, scenario)
        .map_err(Phase2ReplayError::Telemetry)?;
    let final_decoded =
        parse_phase2_telemetry_frame(final_frame).map_err(Phase2ReplayError::Telemetry)?;
    if final_decoded.state_checksum() != read_u32(tape, 20)
        || final_decoded.step() != scenario.steps()
        || final_decoded.events() & EVENT_END == 0
    {
        return Err(Phase2ReplayError::Telemetry(
            Phase2TelemetryReadError::MissionSteps,
        ));
    }
    sink.telemetry_header.copy_from_slice(telemetry_header);
    sink.final_frame.copy_from_slice(final_frame);
    sink.source_stream_crc32 = read_u32(tape, 12);
    sink.max_dynamic_pressure_q16 = read_i32(tape, 24);
    sink.perigee_q12 = read_i32(tape, 28);
    sink.apogee_q12 = read_i32(tape, 32);
    unsafe {
        clear_screen();
        write_text(0, 0, "KSA64 KSA-2A POST-RUN REPLAY");
        write_text(1, 0, "ALTITUDE / DOWNRANGE TRAJECTORY");
        write_text(13, 0, "----------------------------------------");
    }
    let records = &tape[REPLAY_HEADER_LENGTH..canonical_offset];
    let mut index = 0usize;
    while index < count {
        let offset = index * REPLAY_RECORD_LENGTH;
        sink.replay_record(
            index as u32 * header.telemetry_stride() as u32,
            records[offset],
            records[offset + 1],
            records[offset + 2] as u16,
        );
        index += 1;
    }
    Ok(())
}

#[inline]
fn ascii_to_screen(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte - b'A' + 1
    } else {
        byte
    }
}

#[inline]
unsafe fn put(row: usize, column: usize, byte: u8) {
    if row < 25 && column < COLUMNS {
        let offset = row * COLUMNS + column;
        core::ptr::write_volatile(SCREEN.add(offset), ascii_to_screen(byte));
        core::ptr::write_volatile(COLOR_RAM.add(offset), 1);
    }
}

unsafe fn clear_screen() {
    let mut offset = 0usize;
    while offset < CELLS {
        core::ptr::write_volatile(SCREEN.add(offset), 32);
        core::ptr::write_volatile(COLOR_RAM.add(offset), 1);
        offset += 1;
    }
    core::ptr::write_volatile(BORDER, 6);
    core::ptr::write_volatile(BACKGROUND, 0);
}

unsafe fn write_text(row: usize, column: usize, text: &str) {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && column + index < COLUMNS {
        put(row, column + index, bytes[index]);
        index += 1;
    }
}

unsafe fn clear_field(row: usize, column: usize, width: usize) {
    let mut index = 0usize;
    while index < width {
        put(row, column + index, b' ');
        index += 1;
    }
}

unsafe fn write_u32_right(row: usize, column: usize, width: usize, mut value: u32) {
    clear_field(row, column, width);
    let mut position = column + width;
    loop {
        if position == column {
            return;
        }
        position -= 1;
        put(row, position, b'0' + (value % 10) as u8);
        value /= 10;
        if value == 0 {
            return;
        }
    }
}

unsafe fn write_fixed_3(row: usize, column: usize, width: usize, raw: i32, bits: u8) {
    clear_field(row, column, width);
    let negative = raw < 0;
    let magnitude = raw.saturating_abs() as u32;
    let scale = 1u32 << bits;
    let mut integer = magnitude >> bits;
    let fraction = magnitude & (scale - 1);
    let fraction_q16 = if bits >= 16 {
        fraction >> (bits - 16)
    } else {
        fraction << (16 - bits)
    };
    let mut thousandths = (fraction_q16 * 1_000 + 32_768) >> 16;
    if thousandths >= 1_000 {
        integer += 1;
        thousandths -= 1_000;
    }
    let fraction_start = column + width - 3;
    put(row, fraction_start - 1, b'.');
    put(row, fraction_start, b'0' + (thousandths / 100) as u8);
    put(
        row,
        fraction_start + 1,
        b'0' + ((thousandths / 10) % 10) as u8,
    );
    put(row, fraction_start + 2, b'0' + (thousandths % 10) as u8);
    let mut position = fraction_start - 1;
    loop {
        if position == column {
            return;
        }
        position -= 1;
        put(row, position, b'0' + (integer % 10) as u8);
        integer /= 10;
        if integer == 0 {
            break;
        }
    }
    if negative && position > column {
        put(row, position - 1, b'-');
    }
}

unsafe fn write_hex_u32(row: usize, column: usize, value: u32) {
    let mut index = 0usize;
    while index < 8 {
        let digit = ((value >> (28 - index * 4)) & 15) as u8;
        put(
            row,
            column + index,
            if digit < 10 {
                b'0' + digit
            } else {
                b'A' + digit - 10
            },
        );
        index += 1;
    }
}

unsafe fn play_cue(cue: SidCue) {
    let frequency: u16 = match cue {
        SidCue::Ignition => 0x1168,
        SidCue::Cutoff => 0x0d00,
        SidCue::Separation => 0x1900,
        SidCue::End => 0x0800,
        SidCue::ImpactAlarm => 0x0400,
    };
    core::ptr::write_volatile(SID, frequency as u8);
    core::ptr::write_volatile(SID.add(1), (frequency >> 8) as u8);
    core::ptr::write_volatile(SID.add(5), 0x08);
    core::ptr::write_volatile(SID.add(6), 0xf0);
    core::ptr::write_volatile(SID.add(24), 0x0f);
    core::ptr::write_volatile(
        SID.add(4),
        if cue == SidCue::ImpactAlarm {
            0x81
        } else {
            0x11
        },
    );
    let mut delay = 0u16;
    while delay < 2_000 {
        core::hint::spin_loop();
        delay += 1;
    }
    core::ptr::write_volatile(SID.add(4), 0x10);
}

pub fn render_phase2_replay(
    scenario: &Phase2Scenario,
    sink: &Phase2C64ReplaySink,
) -> Result<(), Phase2TelemetryReadError> {
    let header = parse_phase2_telemetry_header_for_scenario(&sink.telemetry_header, scenario)?;
    let frame = sink.latest_frame()?;
    unsafe {
        write_text(0, 31, "ORBIT");
        write_text(14, 0, "T+");
        write_fixed_3(14, 2, 11, frame.time().raw(), 16);
        write_text(14, 14, "S STEP");
        write_u32_right(14, 26, 8, frame.step());
        write_text(15, 0, "ALT");
        write_fixed_3(15, 4, 11, frame.radius().raw() - EARTH_RADIUS_Q12, 12);
        write_text(15, 16, "KM VR");
        write_fixed_3(15, 22, 10, frame.radial_velocity().raw(), 24);
        write_text(16, 0, "MASS");
        write_fixed_3(16, 5, 10, frame.total_mass().raw(), 12);
        write_text(16, 16, "T PROP");
        write_fixed_3(16, 23, 9, frame.propellant().raw(), 12);
        write_text(17, 0, "STAGE");
        write_u32_right(17, 6, 2, frame.active_stage() as u32 + 1);
        write_text(17, 9, "COMPLETE FRAMES");
        write_u32_right(17, 31, 6, sink.frames_replayed() as u32);
        write_text(18, 0, "MAX Q");
        write_fixed_3(18, 7, 10, sink.max_dynamic_pressure_q16, 16);
        write_text(18, 18, "KPA STRIDE");
        write_u32_right(18, 33, 4, header.telemetry_stride() as u32);
        write_text(19, 0, "ORBIT");
        write_fixed_3(19, 6, 9, sink.perigee_q12 - EARTH_RADIUS_Q12, 12);
        write_text(19, 16, "X");
        write_fixed_3(19, 18, 9, sink.apogee_q12 - EARTH_RADIUS_Q12, 12);
        write_text(19, 28, "KM");
        write_text(20, 0, "CHECKSUM");
        write_hex_u32(20, 10, frame.state_checksum());
        write_text(20, 21, "CUE HASH");
        write_hex_u32(20, 30, sink.cue_hash());
        let counts = sink.cue_counts();
        write_text(21, 0, "SID IGN");
        write_u32_right(21, 8, 2, counts[0] as u32);
        write_text(21, 11, "CUT");
        write_u32_right(21, 15, 2, counts[1] as u32);
        write_text(21, 18, "SEP");
        write_u32_right(21, 22, 2, counts[2] as u32);
        write_text(21, 25, "END");
        write_u32_right(21, 29, 2, counts[3] as u32);
        write_text(21, 32, "ALM");
        write_u32_right(21, 36, 2, counts[4] as u32);
        write_text(22, 0, "EVENT MASK");
        write_hex_u32(22, 12, sink.observed_events() as u32);
        write_text(22, 22, "CUES");
        write_u32_right(22, 28, 4, sink.cue_count() as u32);
        write_text(23, 0, "REPLAY SINK");
        write_u32_right(
            23,
            16,
            6,
            core::mem::size_of::<Phase2C64ReplaySink>() as u32,
        );
        write_text(23, 23, "BYTES");
        write_text(24, 0, "POST-RUN REPLAY - TIMING EXCLUDED");
    }
    Ok(())
}
