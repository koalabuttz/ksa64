#![no_std]
#![no_main]

use core::panic::PanicInfo;
use ksa64_core::phase10_telemetry::{
    global_evaluation_identity, GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint,
    KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH,
};

const PLOT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase10/evidence/ksa-g10r-stock.kph10"
));
const SUMMARY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase10/evidence/ksa-g10r-nominal.ksr10"
));
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR: *mut u8 = 0xd800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const KEY_COUNT: *mut u8 = 0x00c6 as *mut u8;
const KEY_BUFFER: *mut u8 = 0x0277 as *mut u8;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const MAGIC: u32 = 0x3042_503a;

#[derive(Clone, Copy)]
struct ReplayEvidence {
    point_count: u16,
    evaluation_identity: u32,
    outcome: u8,
    transition_mask: u8,
    maximum_altitude_q12: i32,
    final_downrange_q12: i32,
    final_crossrange_q12: i32,
    final_time_q16: u32,
    event_mask: u16,
    cue_hash: u32,
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}

fn fail(code: u16) -> ! {
    unsafe {
        write16(4, 1);
        write16(6, code);
        core::ptr::write_volatile(BORDER, 2);
        write32(0, MAGIC);
    }
    loop {}
}

unsafe fn write16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}

unsafe fn write32(offset: usize, value: u32) {
    let mut index = 0;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(offset + index), (value >> (index * 8)) as u8);
        index += 1;
    }
}

fn hash_word(mut hash: u32, value: u32) -> u32 {
    for byte in value.to_le_bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    hash
}

fn inspect() -> Result<ReplayEvidence, u16> {
    let header = GlobalPlotHeader::decode(&PLOT[..KPH10_HEADER_LENGTH]).map_err(|_| 1u16)?;
    let expected = KPH10_HEADER_LENGTH + usize::from(header.point_count) * KPH10_POINT_LENGTH;
    if PLOT.len() != expected {
        return Err(2);
    }
    let summary = GlobalEvaluationSummary::decode(SUMMARY).map_err(|_| 3u16)?;
    if global_evaluation_identity(&summary) != header.evaluation_identity {
        return Err(4);
    }
    let mut maximum_altitude = i32::MIN;
    let mut final_point = None;
    let mut transition_mask = 0u8;
    let mut event_mask = 0u16;
    let mut cue_hash = 0x811c_9dc5;
    let mut previous_frame = 0u8;
    let mut index = 0usize;
    while index < usize::from(header.point_count) {
        let start = KPH10_HEADER_LENGTH + index * KPH10_POINT_LENGTH;
        let point =
            GlobalPlotPoint::decode(&PLOT[start..start + KPH10_POINT_LENGTH]).map_err(|_| 5u16)?;
        maximum_altitude = maximum_altitude.max(point.altitude_q12_km);
        let frame = point.frame as u8;
        if previous_frame != 0 && frame != previous_frame {
            transition_mask |= match (previous_frame, frame) {
                (1, 2) => 1,
                (2, 3) => 2,
                (3, 2) => 4,
                (2, 1) => 8,
                _ => 0x80,
            };
        }
        previous_frame = frame;
        event_mask |= point.events;
        cue_hash = hash_word(cue_hash, point.mission_time_q16);
        cue_hash = hash_word(cue_hash, point.truth_checksum);
        final_point = Some(point);
        index += 1;
    }
    let final_point = final_point.ok_or(6u16)?;
    Ok(ReplayEvidence {
        point_count: header.point_count,
        evaluation_identity: header.evaluation_identity,
        outcome: summary.common.outcome as u8,
        transition_mask,
        maximum_altitude_q12: maximum_altitude,
        final_downrange_q12: final_point.downrange_q12_km,
        final_crossrange_q12: final_point.crossrange_q12_km,
        final_time_q16: final_point.mission_time_q16,
        event_mask,
        cue_hash,
    })
}

fn screen_code(value: u8) -> u8 {
    if value.is_ascii_lowercase() {
        value - b'a' + 1
    } else if value.is_ascii_uppercase() {
        value - b'A' + 1
    } else {
        value
    }
}

unsafe fn clear() {
    let mut index = 0;
    while index < 1000 {
        core::ptr::write_volatile(SCREEN.add(index), b' ');
        core::ptr::write_volatile(COLOR.add(index), 1);
        index += 1;
    }
    core::ptr::write_volatile(BORDER, 6);
    core::ptr::write_volatile(BACKGROUND, 0);
}

unsafe fn text(row: usize, column: usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() && column + index < 40 {
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + column + index),
            screen_code(value[index]),
        );
        index += 1;
    }
}

unsafe fn hex(row: usize, column: usize, value: u32) {
    let digits = b"0123456789ABCDEF";
    let mut index = 0;
    while index < 8 {
        let shift = (7 - index) * 4;
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + column + index),
            digits[((value >> shift) & 15) as usize],
        );
        index += 1;
    }
}

unsafe fn number(row: usize, mut column: usize, mut value: i32) {
    if value < 0 {
        core::ptr::write_volatile(SCREEN.add(row * 40 + column), b'-');
        column += 1;
        value = value.saturating_abs();
    }
    let mut digits = [0u8; 10];
    let mut count = 0;
    loop {
        digits[count] = (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        core::ptr::write_volatile(SCREEN.add(row * 40 + column), b'0' + digits[count]);
        column += 1;
    }
}

unsafe fn render(page: u8, evidence: ReplayEvidence) {
    clear();
    text(0, 1, b"KSA64 PHASE 10 GLOBAL REPLAY");
    text(1, 1, b"KPH10 + KSR10 / STOCK C64");
    text(2, 1, b"PAGE");
    number(2, 7, i32::from(page + 1));
    match page {
        0 => {
            text(5, 2, b"MISSION EVIDENCE");
            text(7, 2, b"POINTS");
            number(7, 22, i32::from(evidence.point_count));
            text(9, 2, b"OUTCOME");
            number(9, 22, i32::from(evidence.outcome));
            text(11, 2, b"TRANSITIONS");
            hex(11, 22, u32::from(evidence.transition_mask));
            text(13, 2, b"EVENTS");
            hex(13, 22, u32::from(evidence.event_mask));
        }
        1 => {
            text(5, 2, b"TRAJECTORY EXTREMA / Q12 KM");
            text(7, 2, b"APOGEE");
            number(7, 22, evidence.maximum_altitude_q12);
            text(9, 2, b"DOWNRANGE");
            number(9, 22, evidence.final_downrange_q12);
            text(11, 2, b"CROSSRANGE");
            number(11, 22, evidence.final_crossrange_q12);
            text(13, 2, b"FINAL Q16 TIME");
            number(13, 22, evidence.final_time_q16 as i32);
        }
        2 => {
            text(5, 2, b"FRAME OWNERSHIP");
            text(7, 2, b"ENU > ECEF");
            text(
                7,
                28,
                if evidence.transition_mask & 1 != 0 {
                    b"OK"
                } else {
                    b"--"
                },
            );
            text(9, 2, b"ECEF > GCRF");
            text(
                9,
                28,
                if evidence.transition_mask & 2 != 0 {
                    b"OK"
                } else {
                    b"--"
                },
            );
            text(11, 2, b"GCRF > ECEF");
            text(
                11,
                28,
                if evidence.transition_mask & 4 != 0 {
                    b"OK"
                } else {
                    b"--"
                },
            );
            text(13, 2, b"ECEF > ENU");
            text(
                13,
                28,
                if evidence.transition_mask & 8 != 0 {
                    b"OK"
                } else {
                    b"--"
                },
            );
        }
        _ => {
            text(5, 2, b"INTEGRITY");
            text(7, 2, b"EVALUATION");
            hex(7, 22, evidence.evaluation_identity);
            text(9, 2, b"CUE HASH");
            hex(9, 22, evidence.cue_hash);
            text(12, 2, b"STRICT CRC + IDENTITY BINDING");
            text(14, 2, b"REPLAY NEVER CHANGES PHYSICS");
            text(16, 2, b"NO REU REQUIRED");
        }
    }
    text(24, 1, b"1-4 PAGE  CURSOR LEFT/RIGHT");
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    let evidence = inspect().unwrap_or_else(|code| fail(code));
    unsafe {
        write16(4, 0);
        write16(6, 0);
        write16(8, evidence.point_count);
        core::ptr::write_volatile(RESULT.add(10), evidence.outcome);
        core::ptr::write_volatile(RESULT.add(11), evidence.transition_mask);
        write32(12, evidence.evaluation_identity);
        write32(16, evidence.cue_hash);
        write32(0, MAGIC);
    }
    let mut page = 0u8;
    unsafe { render(page, evidence) };
    loop {
        unsafe {
            if core::ptr::read_volatile(KEY_COUNT) != 0 {
                let key = core::ptr::read_volatile(KEY_BUFFER);
                core::ptr::write_volatile(KEY_COUNT, 0);
                match key {
                    b'1'..=b'4' => page = key - b'1',
                    29 => page = (page + 1) % 4,
                    157 => page = (page + 3) % 4,
                    _ => {}
                }
                render(page, evidence);
            }
        }
    }
}
