#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_sim::phase4::plot::parse_kph4;
use ksa64_sim::phase4::stock_ui::{render_stock_page, StockPage, REFERENCE_STOCK_UI, SCREEN_BYTES};

const SCREEN: *mut u8 = 0x0400 as *mut u8;
const PAGE_BASES: [*mut u8; 4] = [
    0xc000 as *mut u8,
    0xc400 as *mut u8,
    0xc800 as *mut u8,
    0xcc00 as *mut u8,
];
const STATUS: *mut u8 = 0xcff0 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const PLOT: &[u8; 1_872] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase4/examples/ksa4-baseline.kph4"
));
static mut BUFFER: [u8; SCREEN_BYTES] = [b' '; SCREEN_BYTES];

fn screen_code(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte - b'A' + 1
    } else {
        byte
    }
}

unsafe fn copy_page(target: *mut u8) {
    let source = core::ptr::addr_of!(BUFFER).cast::<u8>();
    let mut index = 0usize;
    while index < SCREEN_BYTES {
        core::ptr::write_volatile(
            target.add(index),
            screen_code(core::ptr::read(source.add(index))),
        );
        index += 1;
    }
}

unsafe fn fail(code: u8) -> ! {
    core::ptr::write_volatile(STATUS, b'E');
    core::ptr::write_volatile(STATUS.add(1), code);
    core::ptr::write_volatile(BORDER, 2);
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { fail(0xff) }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let plot = match parse_kph4(PLOT) {
        Ok(plot) => plot,
        Err(_) => unsafe { fail(1) },
    };
    let pages = [
        StockPage::Campaign,
        StockPage::Histogram,
        StockPage::Trajectory,
        StockPage::Storage,
    ];
    for index in 0..pages.len() {
        unsafe {
            render_stock_page(
                pages[index],
                &REFERENCE_STOCK_UI,
                &plot,
                &mut *core::ptr::addr_of_mut!(BUFFER),
            );
            copy_page(PAGE_BASES[index]);
            if index == 0 {
                copy_page(SCREEN);
            }
        }
    }
    unsafe {
        core::ptr::write_volatile(STATUS, b'K');
        core::ptr::write_volatile(STATUS.add(1), b'S');
        core::ptr::write_volatile(STATUS.add(2), b'A');
        core::ptr::write_volatile(STATUS.add(3), b'4');
        core::ptr::write_volatile(BORDER, 5);
        core::ptr::write_volatile(BACKGROUND, 0);
    }
    loop {}
}
