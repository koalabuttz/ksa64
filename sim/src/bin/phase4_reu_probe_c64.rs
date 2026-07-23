#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_sim::phase4::reu::{detect_c64_reu_capacity, ByteReu, C64ReuStorage};
use ksa64_sim::phase4::storage::{ReuPreference, StoragePlan};

const RESULT: *mut u8 = 0xc000 as *mut u8;
const CIA_TIMER_A_LO: *mut u8 = 0xdc04 as *mut u8;
const CIA_TIMER_A_HI: *mut u8 = 0xdc05 as *mut u8;
const CIA_CONTROL_A: *mut u8 = 0xdc0e as *mut u8;
const CIA1_IRQ_CONTROL: *mut u8 = 0xdc0d as *mut u8;
const CIA2_IRQ_CONTROL: *mut u8 = 0xdd0d as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const REPETITIONS: u16 = 32;
static mut DMA_DATA: [u8; 256] = [0x5a; 256];
static mut DMA_BACKUP: [u8; 256] = [0; 256];

unsafe fn put_u16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}
unsafe fn put_u32(offset: usize, value: u32) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
    core::ptr::write_volatile(RESULT.add(offset + 2), (value >> 16) as u8);
    core::ptr::write_volatile(RESULT.add(offset + 3), (value >> 24) as u8);
}
unsafe fn finish(
    status: u16,
    capacity_kib: u32,
    second_kib: u32,
    preserved: bool,
    plan: Option<StoragePlan>,
    timings: [u16; 3],
) -> ! {
    for offset in 0..4 {
        core::ptr::write_volatile(RESULT.add(offset), 0);
    }
    put_u16(4, 1);
    put_u16(6, status);
    put_u32(8, capacity_kib);
    put_u32(12, second_kib);
    core::ptr::write_volatile(RESULT.add(16), preserved as u8);
    if let Some(plan) = plan {
        put_u16(18, plan.summary_slots as u16);
        put_u16(20, plan.full_histories as u16);
        put_u16(22, plan.compact_histories as u16);
        put_u32(24, plan.used_bytes);
        put_u32(28, plan.free_bytes);
    }
    put_u16(32, timings[0]);
    put_u16(34, timings[1]);
    put_u16(36, timings[2]);
    put_u16(38, REPETITIONS);
    core::ptr::write_volatile(RESULT, b'R');
    core::ptr::write_volatile(RESULT.add(1), b'4');
    core::ptr::write_volatile(RESULT.add(2), b'P');
    core::ptr::write_volatile(RESULT.add(3), b'0');
    core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
    loop {}
}

unsafe fn timer_start() {
    core::ptr::write_volatile(CIA_CONTROL_A, 0);
    core::ptr::write_volatile(CIA_TIMER_A_LO, 0xff);
    core::ptr::write_volatile(CIA_TIMER_A_HI, 0xff);
    core::ptr::write_volatile(CIA_CONTROL_A, 0x11);
}
unsafe fn timer_stop() -> u16 {
    core::ptr::write_volatile(CIA_CONTROL_A, 0);
    let low = core::ptr::read_volatile(CIA_TIMER_A_LO) as u16;
    let high = core::ptr::read_volatile(CIA_TIMER_A_HI) as u16;
    0xffffu16.wrapping_sub(low | (high << 8))
}

fn measure(storage: &mut C64ReuStorage, length: usize) -> Result<u16, ()> {
    let data = unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(DMA_DATA).cast(), length) };
    unsafe { timer_start() };
    for _ in 0..REPETITIONS {
        storage.write_slice(0x1000, data).map_err(|_| ())?;
    }
    Ok(unsafe { timer_stop() })
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { finish(0xffff, 0, 0, false, None, [0; 3]) }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        core::ptr::write_volatile(CIA1_IRQ_CONTROL, 0x7f);
        core::ptr::write_volatile(CIA2_IRQ_CONTROL, 0x7f);
        let _ = core::ptr::read_volatile(CIA1_IRQ_CONTROL);
        let _ = core::ptr::read_volatile(CIA2_IRQ_CONTROL);
    }
    let capacity = match detect_c64_reu_capacity() {
        Ok(value) => value,
        Err(_) => unsafe { finish(1, 0, 0, false, None, [0; 3]) },
    };

    if capacity == 0 {
        let plan = StoragePlan::compute(0, ReuPreference::Auto, 1_024, 906, 226).ok();
        unsafe { finish(0, 0, 0, true, plan, [0; 3]) }
    }
    let mut device = C64ReuStorage::new(16_384);
    let midpoint = capacity * 512;
    let original_zero = device.read_byte(0).unwrap_or(0);
    let original_mid = device.read_byte(midpoint).unwrap_or(0);
    if device.write_byte(0, 0x27).is_err() || device.write_byte(midpoint, 0xd8).is_err() {
        unsafe { finish(2, capacity, 0, false, None, [0; 3]) }
    }
    let second = detect_c64_reu_capacity().unwrap_or(0);
    let preserved =
        device.read_byte(0).ok() == Some(0x27) && device.read_byte(midpoint).ok() == Some(0xd8);
    let _ = device.write_byte(midpoint, original_mid);
    let _ = device.write_byte(0, original_zero);
    if second != capacity || !preserved {
        unsafe { finish(3, capacity, second, preserved, None, [0; 3]) }
    }
    let plan = StoragePlan::compute(capacity, ReuPreference::Auto, 1_024, 906, 901).ok();
    let mut storage = C64ReuStorage::new(capacity);
    let backup = unsafe { &mut *core::ptr::addr_of_mut!(DMA_BACKUP) };
    if storage.read_slice(0x1000, backup).is_err() {
        unsafe { finish(4, capacity, second, preserved, plan, [0; 3]) }
    }
    let timings = match (
        measure(&mut storage, 64),
        measure(&mut storage, 160),
        measure(&mut storage, 256),
    ) {
        (Ok(a), Ok(b), Ok(c)) => [a, b, c],
        _ => unsafe { finish(5, capacity, second, preserved, plan, [0; 3]) },
    };
    let _ = storage.write_slice(0x1000, backup);
    unsafe { finish(0, capacity, second, preserved, plan, timings) }
}
