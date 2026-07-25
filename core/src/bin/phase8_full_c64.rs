#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_core::phase8_fixtures::{
    FIRESTORM_CALM_WIND, FIRESTORM_I211_SPATIAL_MISSION, FIRESTORM_SPATIAL_VEHICLE,
    I211W_SPATIAL_MOTOR,
};
use ksa64_core::phase8_mission::{Phase8MissionError, Phase8MissionMachine};
const MAGIC: u32 = 0x3854_4b53;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn w16(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn w32(o: usize, v: u32) {
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(RESULT.add(o + i), (v >> (i * 8)) as u8);
        i += 1
    }
}
fn fail(c: u16) -> ! {
    unsafe {
        w16(4, 1);
        w16(6, c);
        core::ptr::write_volatile(BORDER, 2);
        w32(0, MAGIC)
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
        w32(0, 0);
        c64_timer::prepare_cia_timing()
    };
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut machine = Phase8MissionMachine::new(
        &FIRESTORM_SPATIAL_VEHICLE,
        &I211W_SPATIAL_MOTOR,
        FIRESTORM_I211_SPATIAL_MISSION,
        &FIRESTORM_CALM_WIND,
    )
    .unwrap_or_else(|_| fail(5));
    unsafe { c64_timer::start_cia_timer() };
    while !machine.is_complete() {
        match machine.step() {
            Ok(_) | Err(Phase8MissionError::Complete) => {}
            Err(Phase8MissionError::ModelEnvelopeExceeded) if machine.is_complete() => {}
            Err(_) => fail(6),
        }
    }
    let elapsed = unsafe { c64_timer::stop_cia_timer() };
    let r = machine.compact_result().unwrap_or_else(|| fail(7));
    unsafe {
        w16(4, 1);
        w16(6, 0);
        w32(8, elapsed.wrapping_sub(overhead));
        w32(12, r.steps);
        w32(16, r.max_altitude_raw_q13 as u32);
        w32(20, r.checksum);
        w16(24, r.event_history);
        core::ptr::write_volatile(BORDER, 5);
        w32(0, MAGIC)
    }
    loop {}
}
