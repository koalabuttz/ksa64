#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_core::phase8_fixtures::{
    FIRESTORM_CALM_WIND, FIRESTORM_I211_SPATIAL_MISSION, FIRESTORM_SPATIAL_VEHICLE,
    I211W_SPATIAL_MOTOR,
};
use ksa64_core::phase8_mission::{Phase8MissionMachine, Phase8MissionSnapshot};
const MAGIC: u32 = 0x3852_4b53;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const TRACE_COUNT: usize = 17;
const RECORD_BASE: usize = 16;
const LAST_BASE: usize = RECORD_BASE + TRACE_COUNT * 4;
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
fn fields(s: Phase8MissionSnapshot, c: u32) -> [i32; 20] {
    [
        s.state.time.raw(),
        s.state.position.x(),
        s.state.position.y(),
        s.state.position.z(),
        s.state.velocity.x(),
        s.state.velocity.y(),
        s.state.velocity.z(),
        s.state.attitude.w(),
        s.state.attitude.x(),
        s.state.attitude.y(),
        s.state.attitude.z(),
        s.state.angular_rate.x(),
        s.state.angular_rate.y(),
        s.state.angular_rate.z(),
        s.phase as i32,
        s.events as i32,
        s.mass.mass.raw(),
        s.thrust_q13,
        s.aero.dynamic_pressure_q13,
        c as i32,
    ]
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
    let mut checks = [0u32; TRACE_COUNT];
    let mut checksum = machine.trace_checksum();
    checks[0] = checksum;
    unsafe { c64_timer::start_cia_timer() };
    let mut i = 1;
    while i < TRACE_COUNT {
        machine.step().unwrap_or_else(|_| fail(6));
        checksum = machine.trace_checksum();
        checks[i] = checksum;
        i += 1
    }
    let elapsed = unsafe { c64_timer::stop_cia_timer() };
    let last = fields(machine.snapshot(), checksum);
    unsafe {
        w16(4, 1);
        w16(6, 0);
        w16(8, TRACE_COUNT as u16);
        w16(10, 0);
        w32(12, elapsed.wrapping_sub(overhead));
        i = 0;
        while i < TRACE_COUNT {
            w32(RECORD_BASE + i * 4, checks[i]);
            i += 1
        }
        i = 0;
        while i < last.len() {
            w32(LAST_BASE + i * 4, last[i] as u32);
            i += 1
        }
        core::ptr::write_volatile(BORDER, 5);
        w32(0, MAGIC)
    }
    loop {}
}
