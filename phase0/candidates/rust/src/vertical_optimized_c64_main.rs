#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::{run_vertical_optimized, vertical_vectors};

const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;
const RESULT_ADDRESS: *mut u16 = 0xc000 as *mut u16;
const CHECKSUM_ADDRESS: *mut u32 = 0xc004 as *mut u32;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER_COLOR, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let run = run_vertical_optimized();
    let checksum_failure = (run.checksum != vertical_vectors::VERTICAL_FINAL_FNV1A32) as u16;
    let failures = run.checkpoint_failures + checksum_failure;
    unsafe {
        core::ptr::write_volatile(RESULT_ADDRESS, failures);
        core::ptr::write_volatile(CHECKSUM_ADDRESS, run.checksum);
        core::ptr::write_volatile(BORDER_COLOR, if failures == 0 { 5 } else { 2 });
    }
    failures as isize
}
