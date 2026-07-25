#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::phase9_finalist::FinalistPackage;
const PACK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase9/examples/phase9-finalists.kfp9"
));
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR: *mut u8 = 0xd800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const KEY_COUNT: *mut u8 = 0x00c6 as *mut u8;
const KEY_BUFFER: *mut u8 = 0x0277 as *mut u8;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const MAGIC: u32 = 0x3946_4b53;
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
fn fail(code: u16) -> ! {
    unsafe {
        write16(4, 1);
        write16(6, code);
        core::ptr::write_volatile(BORDER, 2);
        write32(0, MAGIC)
    }
    loop {}
}
unsafe fn write16(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn write32(o: usize, v: u32) {
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(RESULT.add(o + i), (v >> (i * 8)) as u8);
        i += 1
    }
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
unsafe fn hex(r: usize, c: usize, v: u32) {
    let h = b"0123456789ABCDEF";
    let mut i = 0;
    while i < 8 {
        let shift = (7 - i) * 4;
        core::ptr::write_volatile(SCREEN.add(r * 40 + c + i), h[((v >> shift) & 15) as usize]);
        i += 1
    }
}
unsafe fn number(r: usize, mut c: usize, mut v: i32) {
    if v < 0 {
        core::ptr::write_volatile(SCREEN.add(r * 40 + c), b'-');
        c += 1;
        v = v.saturating_abs()
    }
    let mut d = [0u8; 10];
    let mut n = 0;
    loop {
        d[n] = (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while n > 0 {
        n -= 1;
        core::ptr::write_volatile(SCREEN.add(r * 40 + c), b'0' + d[n]);
        c += 1
    }
}
unsafe fn title(page: u8, name: &[u8], pack: &FinalistPackage<'_>) {
    text(0, 1, b"KSA64 PHASE 9 DESIGN WORKBENCH");
    text(1, 1, b"FINALIST PACKAGE ");
    hex(1, 18, pack.manifest_identity);
    text(2, 1, b"PAGE ");
    number(2, 6, i32::from(page + 1));
    text(2, 9, name);
    text(24, 1, b"CURSOR PAGE   1-7 DIRECT")
}
unsafe fn render(page: u8, selected: u8, pack: &FinalistPackage<'_>) {
    clear();
    let names: [&[u8]; 7] = [
        b"STATUS",
        b"PARETO",
        b"CANDIDATES",
        b"OBJECTIVES",
        b"CONSTRAINTS",
        b"RERUN",
        b"INTEGRITY",
    ];
    title(page, names[page as usize], pack);
    if pack.count == 0 {
        text(6, 3, b"BOOTSTRAP PACKAGE - NO FINALISTS");
        text(8, 3, b"RUN HOST PHASE 9 SEARCH TO");
        text(9, 3, b"GENERATE PHASE9-FINALISTS.KFP9");
        return;
    }
    let index = (selected % pack.count) as usize;
    let (d, a) = match pack.record(index) {
        Ok(v) => v,
        Err(_) => {
            text(6, 3, b"CORRUPT FINALIST RECORD");
            return;
        }
    };
    match page {
        0 => {
            text(5, 2, b"STUDY");
            hex(5, 20, pack.study_identity);
            text(7, 2, b"FINALISTS");
            number(7, 20, i32::from(pack.count));
            text(9, 2, b"SELECTED");
            number(9, 20, index as i32 + 1);
            text(11, 2, b"CANDIDATE");
            hex(11, 20, d.identity)
        }
        1 => {
            text(5, 2, b"PARETO RANKED PACKAGE");
            let mut i = 0;
            while i < pack.count.min(12) {
                if let Ok((_, x)) = pack.record(i as usize) {
                    number(7 + i as usize, 2, i32::from(i + 1));
                    hex(7 + i as usize, 7, x.candidate_identity);
                    number(7 + i as usize, 20, x.objectives[0] >> 13)
                }
                i += 1
            }
        }
        2 => {
            text(5, 2, b"DESIGN VECTOR RAW VALUES");
            let mut i = 0;
            while i < d.value_count.min(14) {
                number(7 + i as usize, 2, i32::from(i));
                number(7 + i as usize, 8, d.values[i as usize]);
                i += 1
            }
        }
        3 => {
            text(5, 2, b"ROBUST OBJECTIVES");
            let mut i = 0;
            while i < a.objective_count.min(8) {
                number(7 + i as usize, 2, i32::from(i));
                number(7 + i as usize, 9, a.objectives[i as usize]);
                i += 1
            }
        }
        4 => {
            text(5, 2, b"HARD CONSTRAINT VALUES");
            text(
                6,
                2,
                if a.feasible {
                    b"STATUS FEASIBLE"
                } else {
                    b"STATUS INFEASIBLE"
                },
            );
            let mut i = 0;
            while i < a.constraint_count.min(12) {
                number(8 + i as usize, 2, i32::from(i));
                number(8 + i as usize, 9, a.constraint_values[i as usize]);
                i += 1
            }
        }
        5 => {
            text(6, 2, b"EXACT RERUN REQUEST");
            text(8, 2, b"CANDIDATE");
            hex(8, 20, a.candidate_identity);
            text(10, 2, b"HOST WORLD + C64 FLIGHT ENDPOINT");
            text(12, 2, b"IDENTITY EXCLUDES PLACEMENT")
        }
        _ => {
            text(5, 2, b"MANIFEST");
            hex(5, 20, pack.manifest_identity);
            text(7, 2, b"STUDY");
            hex(7, 20, pack.study_identity);
            text(9, 2, b"CANDIDATE");
            hex(9, 20, a.candidate_identity);
            text(11, 2, b"AGGREGATE");
            hex(11, 20, a.identity);
            text(13, 2, b"CASE CRC");
            hex(13, 20, a.case_crc)
        }
    }
}
#[no_mangle]
pub extern "C" fn main() -> ! {
    let pack = match FinalistPackage::parse(PACK) {
        Ok(v) => v,
        Err(_) => fail(1),
    };
    unsafe {
        write16(4, 0);
        write16(6, 0);
        write16(8, pack.count as u16);
        write32(12, pack.manifest_identity);
        write32(0, MAGIC)
    }
    let mut page = 0u8;
    let mut selected = 0u8;
    unsafe { render(page, selected, &pack) }
    loop {
        unsafe {
            if core::ptr::read_volatile(KEY_COUNT) != 0 {
                let key = core::ptr::read_volatile(KEY_BUFFER);
                core::ptr::write_volatile(KEY_COUNT, 0);
                match key {
                    b'1'..=b'7' => page = key - b'1',
                    29 => page = (page + 1) % 7,
                    157 => page = (page + 6) % 7,
                    17 => {
                        if pack.count > 0 {
                            selected = (selected + 1) % pack.count
                        }
                    }
                    145 => {
                        if pack.count > 0 {
                            selected = (selected + pack.count - 1) % pack.count
                        }
                    }
                    _ => {}
                }
                render(page, selected, &pack)
            }
        }
    }
}
