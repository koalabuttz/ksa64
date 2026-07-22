#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::{run_vertical_manual, vertical_vectors};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let run = run_vertical_manual();
    let checksum_failure = (run.checksum != vertical_vectors::VERTICAL_FINAL_FNV1A32) as u16;
    (run.checkpoint_failures + checksum_failure) as isize
}
