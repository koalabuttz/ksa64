#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase6_realtime::{
    reference_realtime_guidance_slice, RealtimeFlightComputer, PAL_TICK_BUDGET_CYCLES,
};
use ksa64_interface::phase6::{
    RealtimeAidCell, RealtimeInertialCell, REALTIME_AID_GPS, REALTIME_AID_STAR,
};
const MAGIC: u32 = 0x3652_544b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
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
        u16o(6, code);
        core::ptr::write_volatile(BORDER, 2);
        u32o(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
fn inertial(epoch: u16) -> RealtimeInertialCell {
    RealtimeInertialCell {
        session: 0x6a52,
        measurement_epoch: epoch,
        production_epoch: epoch,
        validity: 0xff,
        flags: 0,
        platform_angle: [0, -8066, 0],
        angular_rate: [0; 3],
        delta_velocity: [1, 2, 3],
        gimbal_applied: [0; 2],
        stage_status: 1,
    }
}
fn aid() -> RealtimeAidCell {
    RealtimeAidCell {
        session: 0x6a52,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: REALTIME_AID_GPS | REALTIME_AID_STAR,
        events: 0,
        onboard_time_q16: 0,
        barometer_q12: 0,
        gps_position_q12: [22_958_965, 0, 12_465_701],
        gps_velocity_q24: [0, 6_857_499, 0],
        star_angle: [0, -8066, 0],
        rcs_propellant_q12: 0,
        vehicle_status: 1,
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        u32o(0, 0);
        c64_timer::prepare_cia_timing()
    };
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let initial = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(initial.start, initial.end, initial.rate);
    unsafe { c64_timer::start_cia_timer() };
    let navigation = flight.tick(Some(inertial(0)), Some(aid()));
    let navigation_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe { c64_timer::start_cia_timer() };
    let fast = flight.tick(Some(inertial(1)), None);
    let fast_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe { c64_timer::start_cia_timer() };
    let slice = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(slice.start, slice.end, slice.rate);
    let guidance = flight.tick(Some(inertial(2)), None);
    let guidance_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    let mut status = 0;
    if navigation.safe || fast.safe || guidance.safe {
        status |= 1
    }
    if navigation.command.effective_epoch != 1
        || fast.command.effective_epoch != 2
        || guidance.command.effective_epoch != 3
    {
        status |= 2
    }
    unsafe {
        u16o(4, 1);
        u16o(6, status);
        u32o(8, overhead);
        u32o(12, navigation_cycles);
        u32o(16, fast_cycles);
        u32o(20, guidance_cycles);
        u32o(24, PAL_TICK_BUDGET_CYCLES);
        u32o(28, flight.navigation().checksum);
        u32o(32, flight.flight_checksum());
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        u32o(0, MAGIC)
    }
    loop {}
}
