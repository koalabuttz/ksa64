#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::run_arithmetic_vectors;

const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;
const RESULT_ADDRESS: *mut u16 = 0xc000 as *mut u16;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER_COLOR, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let failures = run_arithmetic_vectors();
    unsafe {
        core::ptr::write_volatile(RESULT_ADDRESS, failures);
        core::ptr::write_volatile(BORDER_COLOR, if failures == 0 { 5 } else { 2 });
    }
    failures as isize
}
