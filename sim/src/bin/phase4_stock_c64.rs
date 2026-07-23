#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_sim::phase4::plot::parse_kph4;
use ksa64_sim::phase4::stock_ui::{InteractiveStockUi, UiKey, REFERENCE_STOCK_UI, SCREEN_BYTES};

const SCREEN: *mut u8 = 0x0400 as *mut u8;

const STATUS: *mut u8 = 0xcff0 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND: *mut u8 = 0xd021 as *mut u8;
const KEYBOARD_COUNT: *mut u8 = 0x00c6 as *mut u8;
const KEYBOARD_BUFFER: *const u8 = 0x0277 as *const u8;
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

unsafe fn poll_ui_key() -> Option<UiKey> {
    if core::ptr::read_volatile(KEYBOARD_COUNT) == 0 {
        return None;
    }
    let key = core::ptr::read_volatile(KEYBOARD_BUFFER);
    core::ptr::write_volatile(KEYBOARD_COUNT, 0);
    match key {
        133 => Some(UiKey::F1),
        134 => Some(UiKey::F3),
        135 => Some(UiKey::F5),
        136 => Some(UiKey::F7),
        145 | 157 => Some(UiKey::Previous),
        17 | 29 => Some(UiKey::Next),
        13 => Some(UiKey::Return),
        _ => None,
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
    let mut ui = InteractiveStockUi::new();
    unsafe {
        ui.render(
            &REFERENCE_STOCK_UI,
            &plot,
            &mut *core::ptr::addr_of_mut!(BUFFER),
        );
        copy_page(SCREEN);
    }
    unsafe {
        core::ptr::write_volatile(STATUS, b'K');
        core::ptr::write_volatile(STATUS.add(1), b'S');
        core::ptr::write_volatile(STATUS.add(2), b'A');
        core::ptr::write_volatile(STATUS.add(3), b'4');
        core::ptr::write_volatile(BORDER, 5);
        core::ptr::write_volatile(BACKGROUND, 0);
    }
    loop {
        if let Some(key) = unsafe { poll_ui_key() } {
            ui.handle(key);
            unsafe {
                ui.render(
                    &REFERENCE_STOCK_UI,
                    &plot,
                    &mut *core::ptr::addr_of_mut!(BUFFER),
                );
                copy_page(SCREEN);
            }
        }
    }
}
