#![no_std]
#![no_main]
use core::panic::PanicInfo;
#[no_mangle]
pub extern "C" fn main() -> u8 {
    ksa64_sim::run_phase5_telemetry_self_tests()
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
