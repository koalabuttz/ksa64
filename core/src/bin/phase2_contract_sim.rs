#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    (ksa64_core::run_phase2_contract_self_tests() + ksa64_core::run_phase2_atmosphere_self_tests())
        as isize
}
