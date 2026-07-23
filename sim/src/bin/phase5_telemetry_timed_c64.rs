#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase5_telemetry::{
    encode_phase5_telemetry_observation, initial_frame, PHASE5_TELEMETRY_FRAME_LENGTH,
};
use ksa64_sim::phase5_vehicle::Phase5VehicleMachine;
const MAGIC: u32 = 0x3550_544b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const DISCARD: *mut u8 = 0xc100 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn u16o(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn u32o(o: usize, v: u32) {
    for n in 0..4 {
        core::ptr::write_volatile(RESULT.add(o + n), (v >> (n * 8)) as u8)
    }
}
fn fail(code: u16) -> ! {
    unsafe {
        u16o(4, 1);
        u16o(6, 3);
        u16o(8, code);
        core::ptr::write_volatile(BORDER, 2);
        u32o(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        u32o(0, 0);
        c64_timer::prepare_cia_timing()
    };
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let snapshot = Phase5VehicleMachine::new_ksa5a()
        .and_then(|m| m.current_snapshot())
        .unwrap_or_else(|_| fail(1));
    let frame = initial_frame(snapshot);
    let mut bytes = [0u8; PHASE5_TELEMETRY_FRAME_LENGTH];
    unsafe { c64_timer::start_cia_timer() };
    let observation = encode_phase5_telemetry_observation(frame, 2_166_136_261, &mut bytes)
        .unwrap_or_else(|_| fail(2));
    for (n, b) in bytes.iter().enumerate() {
        unsafe { core::ptr::write_volatile(DISCARD.add(n), *b) }
    }
    let cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe {
        u16o(4, 1);
        u16o(6, 3);
        u16o(8, 0);
        u16o(10, 0);
        u32o(12, overhead);
        u32o(16, cycles);
        u32o(20, observation);
        u32o(24, crc32_ieee(&bytes[..PHASE5_TELEMETRY_FRAME_LENGTH - 4]));
        u32o(28, PHASE5_TELEMETRY_FRAME_LENGTH as u32);
        u32o(32, 0);
        core::ptr::write_volatile(BORDER, 5);
        u32o(0, MAGIC)
    }
    loop {}
}
