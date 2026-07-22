#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::run_manual_arithmetic_vectors;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    run_manual_arithmetic_vectors() as isize
}
