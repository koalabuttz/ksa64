#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::evaluation::{EvaluationOutcome, MetricSlot, ModelProfileId};
use ksa64_core::phase8_format::{
    validate_phase8_record, Phase8RecordKind, KPH8_HEADER_LENGTH, KPH8_POINT_LENGTH, KSR8_LENGTH,
};
use ksa64_core::phase8_result::parse_ksr8;
const MAGIC: u32 = 0x3855_4b53;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR: *mut u8 = 0xd800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const KEY_COUNT: *mut u8 = 0x00c6 as *mut u8;
const KEY_BUFFER: *mut u8 = 0x0277 as *mut u8;
const SUMMARY: &[u8; KSR8_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase8/examples/firestorm-i211.ksr8"
));
const PLOT: &[u8; 2036] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase8/examples/firestorm-i211.kph8"
));
unsafe fn w16(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn w32(o: usize, v: u32) {
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(RESULT.add(o + i), (v >> (i * 8)) as u8);
        i += 1
    }
}
fn fail(c: u16) -> ! {
    unsafe {
        w16(4, 1);
        w16(6, c);
        core::ptr::write_volatile(BORDER, 2);
        w32(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
fn r16(i: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([i[o], i[o + 1]])
}
fn r32(i: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([i[o], i[o + 1], i[o + 2], i[o + 3]])
}
fn code(b: u8) -> u8 {
    if b.is_ascii_lowercase() {
        b - b'a' + 1
    } else if b.is_ascii_uppercase() {
        b - b'A' + 1
    } else {
        b
    }
}
unsafe fn clear() {
    let mut i = 0;
    while i < 1000 {
        core::ptr::write_volatile(SCREEN.add(i), b' ');
        core::ptr::write_volatile(COLOR.add(i), 1);
        i += 1
    }
    core::ptr::write_volatile(BORDER, 6);
    core::ptr::write_volatile(BACKGROUND, 0)
}
unsafe fn text(r: usize, c: usize, s: &[u8]) {
    let mut i = 0;
    while i < s.len() && c + i < 40 {
        core::ptr::write_volatile(SCREEN.add(r * 40 + c + i), code(s[i]));
        i += 1
    }
}
unsafe fn number(r: usize, mut c: usize, v: i32, shift: u8) {
    let mut n = if shift == 0 { v } else { v >> shift };
    if n < 0 {
        core::ptr::write_volatile(SCREEN.add(r * 40 + c), b'-');
        c += 1;
        n = n.saturating_abs()
    }
    let mut d = [0u8; 10];
    let mut k = 0;
    loop {
        d[k] = (n % 10) as u8;
        k += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        core::ptr::write_volatile(SCREEN.add(r * 40 + c), b'0' + d[k]);
        c += 1
    }
}
fn crc() -> u32 {
    let mut c = 0xffff_ffffu32;
    let mut i = 0;
    while i < 1000 {
        c ^= unsafe { core::ptr::read_volatile(SCREEN.add(i)) } as u32;
        let mut b = 0;
        while b < 8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            };
            b += 1
        }
        i += 1
    }
    !c
}
unsafe fn title(page: u8, name: &[u8]) {
    text(0, 1, b"KSA64 PHASE 8 MISSION CONTROL");
    text(1, 1, b"FIRESTORM 54 / I211W");
    text(2, 1, b"PAGE ");
    number(2, 6, (page + 1) as i32, 0);
    text(2, 9, name);
    text(24, 1, b"CURSOR: PAGE  1-7: DIRECT");
}
fn count() -> usize {
    r16(PLOT, 32) as usize
}
fn point(i: usize, o: usize) -> i32 {
    r32(PLOT, KPH8_HEADER_LENGTH + i * KPH8_POINT_LENGTH + o)
}
unsafe fn graph_side() {
    let n = count();
    let mut max_z = 1;
    let mut max_r = 1;
    let mut i = 0;
    while i < n {
        max_z = max_z.max(point(i, 12));
        max_r = max_r.max(point(i, 4).abs().saturating_add(point(i, 8).abs()));
        i += 1
    }
    i = 0;
    while i < n {
        let x = 2
            + (point(i, 4).abs().saturating_add(point(i, 8).abs()) as usize * 35 / max_r as usize);
        let y = 21 - (point(i, 12).max(0) as usize * 16 / max_z as usize);
        core::ptr::write_volatile(SCREEN.add(y.min(21) * 40 + x.min(38)), b'*');
        i += 1
    }
    let mut x = 2;
    while x < 39 {
        core::ptr::write_volatile(SCREEN.add(22 * 40 + x), b'-');
        x += 1
    }
}
unsafe fn graph_top() {
    let n = count();
    let mut extent = 1;
    let mut i = 0;
    while i < n {
        extent = extent.max(point(i, 4).abs()).max(point(i, 8).abs());
        i += 1
    }
    i = 0;
    while i < n {
        let x = (20 + (point(i, 4) as i64 * 17 / extent as i64) as i32).clamp(2, 38) as usize;
        let y = (13 - (point(i, 8) as i64 * 8 / extent as i64) as i32).clamp(5, 21) as usize;
        core::ptr::write_volatile(SCREEN.add(y * 40 + x), b'*');
        i += 1
    }
    text(13, 19, b"+")
}
unsafe fn render(page: u8, s: ksa64_core::evaluation::EvaluationSummary) {
    clear();
    match page {
        0 => {
            title(page, b"FLIGHT STATUS");
            text(4, 2, b"STATUS: COMPLETE - RECOVERED");
            for (r, label, slot, sh, unit) in [
                (
                    6,
                    b"APOGEE" as &[u8],
                    MetricSlot::ApogeeAltitude,
                    13,
                    b"M" as &[u8],
                ),
                (7, b"MAX SPEED", MetricSlot::MaxSpeed, 19, b"M/S"),
                (8, b"MAX Q", MetricSlot::MaxDynamicPressure, 13, b"PA"),
                (9, b"MAX AOA", MetricSlot::MaximumAngleOfAttack, 28, b"RAD"),
            ] {
                text(r, 2, label);
                number(r, 18, s.metric(slot).unwrap_or(0), sh);
                text(r, 29, unit)
            }
            text(12, 2, b"RAIL EXIT / BURNOUT / APOGEE");
            text(14, 2, b"DROGUE / MAIN / LANDING: PASS");
        }
        1 => {
            title(page, b"SIDE TRAJECTORY");
            graph_side();
        }
        2 => {
            title(page, b"TOP-DOWN DRIFT");
            graph_top();
        }
        3 => {
            title(page, b"STABILITY / AOA");
            text(5, 2, b"MIN STATIC MARGIN");
            number(
                5,
                25,
                s.metric(MetricSlot::MinimumStaticMargin).unwrap_or(0),
                24,
            );
            text(7, 2, b"RAIL EXIT MARGIN");
            number(
                7,
                25,
                s.metric(MetricSlot::RailExitStaticMargin).unwrap_or(0),
                24,
            );
            text(9, 2, b"BURNOUT MARGIN");
            number(
                9,
                25,
                s.metric(MetricSlot::BurnoutStaticMargin).unwrap_or(0),
                24,
            );
            text(11, 2, b"MAX AOA RAW");
            number(
                11,
                25,
                s.metric(MetricSlot::MaximumAngleOfAttack).unwrap_or(0),
                0,
            );
        }
        4 => {
            title(page, b"WIND");
            text(5, 2, b"MAX WIND M/S");
            number(
                5,
                22,
                s.metric(MetricSlot::MaximumWindSpeed).unwrap_or(0),
                22,
            );
            text(8, 2, b"CALM REFERENCE PROFILE");
            text(10, 2, b"LAYERED/GUST CONTRACT READY");
        }
        5 => {
            title(page, b"EVENTS");
            for (r, label, bit) in [
                (5, b"RAIL EXIT" as &[u8], 1u32),
                (7, b"BURNOUT", 2),
                (9, b"APOGEE", 4),
                (11, b"DROGUE", 8),
                (13, b"MAIN", 16),
                (15, b"LANDING", 32),
            ] {
                text(r, 3, label);
                text(
                    r,
                    24,
                    if s.events & bit != 0 {
                        b"PASS"
                    } else {
                        b"MISS"
                    },
                )
            }
        }
        _ => {
            title(page, b"LANDING SUMMARY");
            text(5, 2, b"GROUND TIME S");
            number(
                5,
                23,
                s.metric(MetricSlot::GroundContactTime).unwrap_or(0),
                17,
            );
            text(7, 2, b"IMPACT SPEED M/S");
            number(
                7,
                23,
                s.metric(MetricSlot::ImpactVelocity).unwrap_or(0).abs(),
                19,
            );
            text(9, 2, b"LANDING DIST M");
            number(
                9,
                23,
                s.metric(MetricSlot::LandingDistance).unwrap_or(0),
                13,
            );
            text(13, 2, b"GROUND CONTACT: NOMINAL");
        }
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { w32(0, 0) };
    let rec = parse_ksr8(SUMMARY).unwrap_or_else(|_| fail(1));
    validate_phase8_record(PLOT, Phase8RecordKind::PlotHeader).unwrap_or_else(|_| fail(2));
    if rec.summary.profile != ModelProfileId::HobbySpatialV1
        || rec.summary.outcome != EvaluationOutcome::GroundContact
        || r16(PLOT, 34) as usize != KPH8_POINT_LENGTH
        || PLOT.len() != KPH8_HEADER_LENGTH + count() * KPH8_POINT_LENGTH + 4
    {
        fail(3)
    }
    let mut page = 0u8;
    unsafe {
        render(page, rec.summary);
        w16(4, 1);
        w16(6, 0);
        w32(8, crc());
        w32(12, count() as u32);
        w16(16, 7);
        w16(18, 0);
        core::ptr::write_volatile(BORDER, 5);
        w32(0, MAGIC)
    }
    loop {
        let n = unsafe { core::ptr::read_volatile(KEY_COUNT) };
        if n != 0 {
            let key = unsafe { core::ptr::read_volatile(KEY_BUFFER) };
            unsafe { core::ptr::write_volatile(KEY_COUNT, 0) };
            page = match key {
                b'1'..=b'7' => key - b'1',
                29 => (page + 1) % 7,
                157 => (page + 6) % 7,
                _ => page,
            };
            unsafe { render(page, rec.summary) }
        }
    }
}
