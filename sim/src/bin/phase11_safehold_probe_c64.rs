#![no_std]
#![no_main]

use core::panic::PanicInfo;

const MAGIC: u32 = 0x3148_534b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;

unsafe fn p16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}

unsafe fn p32(offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        core::ptr::write_volatile(RESULT.add(offset + index), bytes[index]);
        index += 1;
    }
}

fn finish(failures: u16) -> ! {
    unsafe {
        p16(4, 1);
        p16(6, failures);
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        p32(0, MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u16::MAX)
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let value = ksa64_sim::run_safehold_probe();
    unsafe {
        p32(0, 0);
        p16(8, value.releases);
        p32(10, value.flight_checksum);
        p32(14, value.navigation_checksum);
        p32(18, value.command_checksum);
        p32(22, value.journal_chain);
        p16(26, value.drogue_epoch);
        p16(28, value.main_epoch);
        core::ptr::write_volatile(RESULT.add(30), value.transition_count);
        core::ptr::write_volatile(RESULT.add(31), value.final_frame as u8);
        core::ptr::write_volatile(RESULT.add(32), u8::from(value.safe));
        p32(34, ksa64_sim::phase11_safehold_probe_signature());
    }
    finish(value.failures)
}
