#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_sim::replay::replay_phase3_tape;

const SCREEN: *mut u8 = 0x0400 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const SID_FREQ_LO: *mut u8 = 0xd400 as *mut u8;
const SID_FREQ_HI: *mut u8 = 0xd401 as *mut u8;
const SID_CONTROL: *mut u8 = 0xd404 as *mut u8;
const TAPE: &[u8; 21_776] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase3/examples/ksa3-nominal.krp3"
));
const SCENARIO_ID: u32 = 0x95bc_9413;
const CONFIG_CRC32: u32 = 0x2815_ea66;

unsafe fn screen_code(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte - b'A' + 1
    } else {
        byte
    }
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
unsafe fn hex_digit(value: u8) -> u8 {
    if value < 10 {
        b'0' + value
    } else {
        b'A' + value - 10
    }
}
unsafe fn hex16(row: usize, column: usize, value: u16) {
    let mut shift = 12;
    let mut index = 0;
    loop {
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + column + index),
            screen_code(hex_digit(((value >> shift) & 15) as u8)),
        );
        if shift == 0 {
            break;
        }
        shift -= 4;
        index += 1;
    }
}
unsafe fn hex32(row: usize, column: usize, value: u32) {
    hex16(row, column, (value >> 16) as u16);
    hex16(row, column + 4, value as u16);
}
unsafe fn decimal4(row: usize, column: usize, value: u32) {
    let divisors = [1000u32, 100, 10, 1];
    let mut index = 0;
    while index < 4 {
        let digit = (value / divisors[index]) % 10;
        core::ptr::write_volatile(SCREEN.add(row * 40 + column + index), b'0' + digit as u8);
        index += 1;
    }
}
unsafe fn clear() {
    let mut index = 0;
    while index < 1000 {
        core::ptr::write_volatile(SCREEN.add(index), b' ');
        index += 1;
    }
    core::ptr::write_volatile(BORDER, 0);
    core::ptr::write_volatile(BACKGROUND, 0);
}
unsafe fn fail(code: u16) -> ! {
    clear();
    text(24, 0, b"PHASE 3 REPLAY ERROR");
    hex16(24, 21, code);
    core::ptr::write_volatile(BORDER, 2);
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { fail(0xffff) }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let summary = match replay_phase3_tape(TAPE, SCENARIO_ID, CONFIG_CRC32) {
        Ok(summary) => summary,
        Err(_) => unsafe { fail(1) },
    };
    if summary.frames != 906
        || summary.final_step != 7200
        || summary.final_mode != 5
        || summary.final_stage != 1
        || summary.source_stream_crc32 != 0xaf79_b36e
        || summary.cue_counts != [2, 2, 1, 1, 0]
        || summary.observed_alarms != 0
    {
        unsafe { fail(2) }
    }
    unsafe {
        clear();
        text(0, 0, b"KSA64 PHASE 3 REPLAY");
        text(2, 0, b"FRAMES");
        decimal4(2, 7, summary.frames);
        text(2, 13, b"STEP");
        decimal4(2, 18, summary.final_step);
        text(3, 0, b"ALT Q12");
        hex32(3, 8, summary.final_altitude_q12 as u32);
        text(3, 18, b"DOWN");
        hex32(3, 23, summary.final_downrange_q32 as u32);
        text(4, 0, b"MODE");
        hex16(4, 5, summary.final_mode as u16);
        text(4, 10, b"STAGE");
        hex16(4, 16, summary.final_stage as u16);
        text(4, 21, b"PITCH");
        hex16(4, 27, summary.final_pitch);
        text(5, 0, b"EVENTS");
        hex16(5, 7, summary.observed_events);
        text(5, 13, b"ALARMS");
        hex16(5, 20, summary.observed_alarms);
        text(6, 0, b"SID I02 C02 S01 E01 A00");
        text(7, 0, b"SOURCE CRC");
        hex32(7, 11, summary.source_stream_crc32);
        text(8, 0, b"CONFIG CRC");
        hex32(8, 11, summary.config_crc32);
        text(9, 0, b"CUE HASH");
        hex32(9, 9, summary.cue_hash);
        text(24, 0, b"PHASE 3 REPLAY PASS");
        core::ptr::write_volatile(SID_FREQ_LO, summary.cue_hash as u8);
        core::ptr::write_volatile(SID_FREQ_HI, (summary.cue_hash >> 8) as u8);
        core::ptr::write_volatile(SID_CONTROL, 0x11);
        core::ptr::write_volatile(BORDER, 5);
    }
    loop {}
}
