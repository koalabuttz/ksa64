#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::phase9_5_canard::run_phase95_canard_case;
const MAGIC: u32 = 0x394e_4143;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn put(at: usize, value: u32) {
    for (index, byte) in value.to_le_bytes().iter().copied().enumerate() {
        core::ptr::write_volatile(RESULT.add(at + index), byte);
    }
}
fn finish(failures: u32) -> ! {
    unsafe {
        put(4, failures);
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        put(0, MAGIC);
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u32::MAX)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        put(8, 1);
    }
    let mut failures = run_phase95_canard_case(0);
    unsafe {
        put(8, 2);
    }
    failures = failures.saturating_add(run_phase95_canard_case(1));
    unsafe {
        put(8, 3);
    }
    failures = failures.saturating_add(run_phase95_canard_case(2));
    unsafe {
        put(8, 0x103);
    }
    finish(failures)
}
