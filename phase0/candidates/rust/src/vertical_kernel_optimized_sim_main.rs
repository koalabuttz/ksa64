#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::{
    run_vertical_kernel_optimized, vertical_state_matches_checkpoint, vertical_vectors,
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let state = run_vertical_kernel_optimized();
    let final_checkpoint =
        vertical_vectors::VERTICAL_CHECKPOINTS[vertical_vectors::VERTICAL_CHECKPOINTS.len() - 1];
    (!vertical_state_matches_checkpoint(&state, final_checkpoint)) as isize
}
