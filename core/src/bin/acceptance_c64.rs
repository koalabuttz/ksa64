#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(0xd020 as *mut u8, 2);
        core::ptr::write_volatile(0xd021 as *mut u8, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let failures = ksa64_core::run_c64_acceptance_self_tests();
    unsafe {
        let border = if failures == 0 { 5 } else { 2 };
        let background = if failures == 0 { 0 } else { 2 };
        core::ptr::write_volatile(0xd020 as *mut u8, border);
        core::ptr::write_volatile(0xd021 as *mut u8, background);
    }
    loop {}
}
