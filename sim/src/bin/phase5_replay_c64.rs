#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_sim::phase5_history::{parse_kph5_point, KPH5_HEADER_LENGTH, KPH5_POINT_LENGTH};
use ksa64_sim::phase5_replay::{phase5_plot_coordinate, replay_phase5_history};
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const SID_LO: *mut u8 = 0xd400 as *mut u8;
const SID_HI: *mut u8 = 0xd401 as *mut u8;
const SID_CTL: *mut u8 = 0xd404 as *mut u8;
const TAPE: &[u8; 1664] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase5/examples/ksa5-baseline.kph5"
));
unsafe fn code(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b - b'A' + 1
    } else {
        b
    }
}
unsafe fn text(row: usize, col: usize, s: &[u8]) {
    let mut n = 0;
    while n < s.len() && col + n < 40 {
        core::ptr::write_volatile(SCREEN.add(row * 40 + col + n), code(s[n]));
        n += 1
    }
}
unsafe fn hex_digit(v: u8) -> u8 {
    if v < 10 {
        b'0' + v
    } else {
        b'A' + v - 10
    }
}
unsafe fn hex16(row: usize, col: usize, v: u16) {
    let mut n = 0;
    while n < 4 {
        let shift = 12 - 4 * n;
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + col + n),
            code(hex_digit(((v >> shift) & 15) as u8)),
        );
        n += 1
    }
}
unsafe fn hex32(row: usize, col: usize, v: u32) {
    hex16(row, col, (v >> 16) as u16);
    hex16(row, col + 4, v as u16)
}
unsafe fn dec4(row: usize, col: usize, v: u16) {
    let d = [1000u16, 100, 10, 1];
    let mut n = 0;
    while n < 4 {
        core::ptr::write_volatile(
            SCREEN.add(row * 40 + col + n),
            b'0' + ((v / d[n]) % 10) as u8,
        );
        n += 1
    }
}
unsafe fn clear() {
    let mut n = 0;
    while n < 1000 {
        core::ptr::write_volatile(SCREEN.add(n), b' ');
        n += 1
    }
    core::ptr::write_volatile(BORDER, 0);
    core::ptr::write_volatile(BACKGROUND, 0)
}
unsafe fn fail(v: u16) -> ! {
    clear();
    text(24, 0, b"PHASE 5 REPLAY ERROR");
    hex16(24, 21, v);
    core::ptr::write_volatile(BORDER, 2);
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    unsafe { fail(0xffff) }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    let s = match replay_phase5_history(TAPE, 0x4b53_4135, 0) {
        Ok(v) => v,
        Err(_) => unsafe { fail(1) },
    };
    if s.points != 99
        || s.final_step != 3133
        || s.final_position_quarter_km != [20965, 3780, 15330]
        || s.max_dynamic_pressure_sixteenth_kpa != 697
        || s.max_navigation_error_quarter_km != 6
        || s.observed_events != 7
        || s.observed_alarms != 0
        || s.cue_counts != [2, 2, 1, 0, 0]
        || s.cue_hash != 0x3b2f_b64b
    {
        unsafe { fail(2) }
    }
    unsafe {
        clear();
        text(0, 0, b"KSA64 PHASE 5 REPLAY");
        text(1, 0, b"RUN 0000 POINTS");
        dec4(1, 16, s.points);
        text(1, 22, b"STEP");
        dec4(1, 27, s.final_step as u16);
        text(2, 0, b"POS");
        hex16(2, 4, s.final_position_quarter_km[0] as u16);
        hex16(2, 9, s.final_position_quarter_km[1] as u16);
        hex16(2, 14, s.final_position_quarter_km[2] as u16);
        text(3, 0, b"MAXQ");
        hex16(3, 5, s.max_dynamic_pressure_sixteenth_kpa);
        text(3, 10, b"NAV");
        hex16(3, 14, s.max_navigation_error_quarter_km);
        text(3, 20, b"EVENTS");
        hex16(3, 27, s.observed_events);
        text(4, 0, b"SID I02 C02 S01 A00 R00");
        let mut r = 5;
        while r <= 20 {
            core::ptr::write_volatile(SCREEN.add(r * 40 + 3), b'.');
            r += 1
        }
        let mut c = 3;
        while c <= 36 {
            core::ptr::write_volatile(SCREEN.add(20 * 40 + c), b'.');
            c += 1
        }
        let mut i = 0usize;
        while i < s.points as usize {
            let at = KPH5_HEADER_LENGTH + i * KPH5_POINT_LENGTH;
            let p = match parse_kph5_point(&TAPE[at..at + KPH5_POINT_LENGTH]) {
                Ok(v) => v,
                Err(_) => fail(3),
            };
            let (x, y) = phase5_plot_coordinate(p);
            let ch = if i == 0 {
                code(b'S')
            } else if i + 1 == s.points as usize {
                code(b'X')
            } else if p.events != 0 {
                b'+'
            } else {
                b'*'
            };
            core::ptr::write_volatile(SCREEN.add(y as usize * 40 + x as usize), ch);
            i += 1
        }
        text(21, 0, b"Y-Z PROJECTION QUARTER KM");
        text(22, 0, b"CUE HASH");
        hex32(22, 9, s.cue_hash);
        text(23, 0, b"KPH5 CRC F2B3B81F");
        text(24, 0, b"PHASE 5 REPLAY PASS");
        core::ptr::write_volatile(SID_LO, s.cue_hash as u8);
        core::ptr::write_volatile(SID_HI, (s.cue_hash >> 8) as u8);
        core::ptr::write_volatile(SID_CTL, 0x11);
        core::ptr::write_volatile(BORDER, 5)
    }
    loop {}
}
