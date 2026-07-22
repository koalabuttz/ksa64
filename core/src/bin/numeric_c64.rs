#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let failures = ksa64_core::run_numeric_self_tests();
    unsafe {
        let color = if failures == 0 { 0 } else { 2 };
        core::ptr::write_volatile(0xd020 as *mut u8, color);
        core::ptr::write_volatile(0xd021 as *mut u8, color);
    }
    failures as isize
}
