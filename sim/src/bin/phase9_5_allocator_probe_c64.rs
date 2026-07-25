#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_sim::phase9_5::{phase95_allocator_probe_signature, run_phase95_allocator_self_tests};
const MAGIC: u32 = 0x3941_4c43;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn put(at: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(at + index), bytes[index]);
        index += 1;
    }
}
fn finish(failures: u32, signature: u32) -> ! {
    unsafe {
        put(4, failures);
        put(8, signature);
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        put(0, MAGIC);
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u32::MAX, 0)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    finish(
        run_phase95_allocator_self_tests(),
        phase95_allocator_probe_signature(),
    )
}
