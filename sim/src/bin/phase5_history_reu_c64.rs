#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_sim::phase4::reu::{detect_c64_reu_capacity, ByteReu, C64ReuStorage};
use ksa64_sim::phase4::storage::ReuPreference;
use ksa64_sim::phase5_history::Phase5StoragePlan;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn u16at(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn u32at(o: usize, v: u32) {
    for n in 0..4 {
        core::ptr::write_volatile(RESULT.add(o + n), (v >> (8 * n)) as u8)
    }
}
unsafe fn finish(
    status: u16,
    capacity: u32,
    second: u32,
    preserved: bool,
    plan: Option<Phase5StoragePlan>,
) -> ! {
    for n in 0..4 {
        core::ptr::write_volatile(RESULT.add(n), 0)
    }
    u16at(4, 1);
    u16at(6, status);
    u32at(8, capacity);
    u32at(12, second);
    core::ptr::write_volatile(RESULT.add(16), preserved as u8);
    if let Some(p) = plan {
        u16at(18, p.summary_slots as u16);
        u16at(20, p.full_histories as u16);
        u16at(22, p.compact_histories as u16);
        u32at(24, p.used_bytes);
        u32at(28, p.free_bytes)
    }
    for (n, b) in b"H5P0".iter().enumerate() {
        core::ptr::write_volatile(RESULT.add(n), *b)
    }
    core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    unsafe { finish(0xffff, 0, 0, false, None) }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    let capacity = match detect_c64_reu_capacity() {
        Ok(v) => v,
        Err(_) => unsafe { finish(1, 0, 0, false, None) },
    };
    if capacity == 0 {
        let p = Phase5StoragePlan::compute(0, ReuPreference::Auto, 256, 3134, 99).ok();
        unsafe { finish(0, 0, 0, true, p) }
    }
    let mut device = C64ReuStorage::new(16_384);
    let address = capacity * 512;
    let original = device.read_byte(address).unwrap_or(0);
    if device.write_byte(address, 0x6d).is_err() {
        unsafe { finish(2, capacity, 0, false, None) }
    }
    let second = detect_c64_reu_capacity().unwrap_or(0);
    let preserved = device.read_byte(address).ok() == Some(0x6d);
    let _ = device.write_byte(address, original);
    if second != capacity || !preserved {
        unsafe { finish(3, capacity, second, preserved, None) }
    }
    let p = Phase5StoragePlan::compute(capacity, ReuPreference::Auto, 256, 3134, 393).ok();
    unsafe { finish(u16::from(p.is_none()) * 4, capacity, second, preserved, p) }
}
