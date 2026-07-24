#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::evaluation::{EvaluationOutcome, MetricSlot, ModelProfileId};
use ksa64_core::phase7_format::{
    validate_phase7_record, Phase7RecordKind, KPH7_HEADER_LENGTH, KPH7_POINT_LENGTH, KSR7_LENGTH,
};
use ksa64_core::phase7_result::parse_ksr7;

const MAGIC: u32 = 0x3755_4b53;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR: *mut u8 = 0xd800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const SUMMARY_BYTES: &[u8; KSR7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm-i211.ksr7"
));
const PLOT_BYTES: &[u8; 2_052] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm-i211.kph7"
));

unsafe fn write_u16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}

unsafe fn write_u32(offset: usize, value: u32) {
    let mut index = 0usize;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(offset + index), (value >> (index * 8)) as u8);
        index += 1;
    }
}

fn fail(code: u16) -> ! {
    unsafe {
        write_u16(4, 1);
        write_u16(6, code);
        core::ptr::write_volatile(BORDER, 2);
        write_u32(0, MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}

fn screen_code(byte: u8) -> u8 {
    if byte.is_ascii_lowercase() {
        byte - b'a' + 1
    } else if byte.is_ascii_uppercase() {
        byte - b'A' + 1
    } else {
        byte
    }
}

unsafe fn clear_screen() {
    let mut index = 0usize;
    while index < 1_000 {
        core::ptr::write_volatile(SCREEN.add(index), b' ');
        core::ptr::write_volatile(COLOR.add(index), 1);
        index += 1;
    }
    core::ptr::write_volatile(BORDER, 6);
    core::ptr::write_volatile(BACKGROUND, 0);
}

unsafe fn text(row: usize, column: usize, value: &[u8]) {
    let mut index = 0usize;
    while index < value.len() && column + index < 40 {
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + column + index),
            screen_code(value[index]),
        );
        index += 1;
    }
}

unsafe fn unsigned(row: usize, mut column: usize, mut value: u32) {
    let mut digits = [0u8; 10];
    let mut count = 0usize;
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

fn read_i32(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    read_i32(input, offset) as u32
}

fn screen_crc32() -> u32 {
    let mut crc = 0xffff_ffffu32;
    let mut index = 0usize;
    while index < 1_000 {
        let byte = unsafe { core::ptr::read_volatile(SCREEN.add(index)) };
        crc ^= byte as u32;
        let mut bit = 0u8;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        index += 1;
    }
    !crc
}

unsafe fn plot_trajectory(point_count: usize, max_altitude_raw: i32) {
    let mut column = 2usize;
    while column <= 37 {
        core::ptr::write_volatile(SCREEN.add(21 * 40 + column), b'-');
        column += 1;
    }
    let mut index = 0usize;
    while index < point_count {
        let offset = KPH7_HEADER_LENGTH + index * KPH7_POINT_LENGTH;
        let altitude = read_i32(PLOT_BYTES, offset + 4).max(0);
        let x = if point_count <= 1 {
            2
        } else {
            2 + index * 35 / (point_count - 1)
        };
        let height = if max_altitude_raw <= 0 {
            0
        } else {
            (altitude * 12 / max_altitude_raw) as usize
        };
        let y = 20usize.saturating_sub(height.min(12));
        core::ptr::write_volatile(SCREEN.add(y * 40 + x), b'*');
        index += 1;
    }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        write_u32(0, 0);
        clear_screen();
    }
    let record = parse_ksr7(SUMMARY_BYTES).unwrap_or_else(|_| fail(1));
    let plot_header = validate_phase7_record(PLOT_BYTES, Phase7RecordKind::PlotHeader)
        .unwrap_or_else(|_| fail(2));
    let point_count = u16::from_le_bytes([PLOT_BYTES[32], PLOT_BYTES[33]]) as usize;
    let point_stride = u16::from_le_bytes([PLOT_BYTES[34], PLOT_BYTES[35]]) as usize;
    if point_stride != KPH7_POINT_LENGTH
        || PLOT_BYTES.len() != KPH7_HEADER_LENGTH + point_count * point_stride + 4
    {
        fail(3);
    }
    let summary = record.summary;
    if summary.profile != ModelProfileId::HobbyVerticalV1
        || summary.outcome != EvaluationOutcome::GroundContact
        || summary.numeric_faults != 0
        || summary.source_checksums[0] != 0xa61c_5720
    {
        fail(4);
    }
    if plot_header.identity != 0x27b6_ec02
        || read_u32(PLOT_BYTES, 36) != 0xaed5_5fae
        || read_u32(PLOT_BYTES, 40) != 0x9776_df16
        || read_u32(PLOT_BYTES, 44) != 0x6e10_5477
    {
        fail(5);
    }
    let apogee = summary
        .metric(MetricSlot::ApogeeAltitude)
        .unwrap_or_else(|| fail(6));
    let speed = summary
        .metric(MetricSlot::MaxSpeed)
        .unwrap_or_else(|| fail(7));
    let pressure = summary
        .metric(MetricSlot::MaxDynamicPressure)
        .unwrap_or_else(|| fail(8));
    let impact = summary
        .metric(MetricSlot::ImpactVelocity)
        .unwrap_or_else(|| fail(9));

    unsafe {
        text(0, 4, b"KSA64 PHASE 7 MISSION CONTROL");
        text(2, 2, b"FIRESTORM 54 / AEROTECH I211W");
        text(3, 2, b"STATUS: COMPLETE - RECOVERED");
        text(5, 2, b"APOGEE");
        unsigned(5, 13, (apogee >> 13) as u32);
        text(5, 19, b"M");
        text(6, 2, b"MAX SPEED");
        unsigned(6, 13, (speed >> 19) as u32);
        text(6, 19, b"M/S");
        text(7, 2, b"MAX Q");
        unsigned(7, 13, (pressure >> 7) as u32);
        text(7, 19, b"PA");
        text(8, 2, b"IMPACT");
        unsigned(8, 13, ((impact.saturating_abs()) >> 19) as u32);
        text(8, 19, b"M/S");
        plot_trajectory(point_count, apogee);
        text(22, 2, b"LAUNCH     APOGEE       RECOVERY");
        text(24, 8, b"PHASE 7 REPLAY PASS");
    }

    let crc = screen_crc32();
    unsafe {
        write_u16(4, 1);
        write_u16(6, 0);
        write_u32(8, crc);
        write_u32(12, summary.source_checksums[0]);
        write_u32(16, point_count as u32);
        write_u32(20, plot_header.identity);
        core::ptr::write_volatile(BORDER, 5);
        write_u32(0, MAGIC);
    }
    loop {}
}
