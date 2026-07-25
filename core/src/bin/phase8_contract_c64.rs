#![no_std]
#![no_main]

use core::panic::PanicInfo;
use ksa64_core::{phase8_contract_signature, run_phase8_contract_self_tests};

const MAGIC: u32 = 0x3850_4b53;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;

unsafe fn write_u32(offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        core::ptr::write_volatile(RESULT.add(offset + index), bytes[index]);
        index += 1;
    }
}

fn finish(failures: u32) -> ! {
    unsafe {
        write_u32(4, failures);
        write_u32(8, phase8_contract_signature());
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        write_u32(0, MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u32::MAX)
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    finish(run_phase8_contract_self_tests())
}
