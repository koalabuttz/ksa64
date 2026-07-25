#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_sim::{phase95_contract_signature, run_phase95_contract_self_tests};
const MAGIC: u32 = 0x3943_4c4b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn put(at: usize, value: u32) {
    let b = value.to_le_bytes();
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(RESULT.add(at + i), b[i]);
        i += 1
    }
}
fn finish(failures: u32) -> ! {
    unsafe {
        put(4, failures);
        put(8, phase95_contract_signature());
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        put(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u32::MAX)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    finish(run_phase95_contract_self_tests())
}
