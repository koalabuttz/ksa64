#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_flight::phase9_5_allocator::AllocatedAdvancedFlightComputer;
use ksa64_interface::phase9_5::*;
use ksa64_sim::phase8_5::LOCAL_SESSION;
use ksa64_sim::phase9_5::{reference_mixed_allocator_config, reference_mixed_flight_config};
const MAGIC: u32 = 0x3954_4c4b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BUDGET: u32 = 24_631;
unsafe fn p16(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn p32(o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(RESULT.add(o + i), b[i]);
        i += 1
    }
}
fn fail(c: u16) -> ! {
    unsafe {
        p16(4, 1);
        p16(6, c);
        core::ptr::write_volatile(BORDER, 2);
        p32(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
fn fast(e: u16, air: bool) -> AdvancedFastSensorCell {
    AdvancedFastSensorCell {
        session: LOCAL_SESSION,
        measurement_epoch: e,
        production_epoch: e,
        validity: ADVANCED_VALID_PLATFORM
            | ADVANCED_VALID_RATE
            | ADVANCED_VALID_DELTA_V
            | ADVANCED_VALID_ACTUATOR
            | if air { ADVANCED_VALID_AIR_DATA } else { 0 }
            | ADVANCED_VALID_SUPPLY,
        platform_angle: [100, -100, 50],
        angular_rate: [5, 0, 0],
        delta_velocity: [0, 0, 2500],
        dynamic_pressure_q10: 2000 << 10,
        mach_q12: 1024,
        gimbal_applied: [0; 2],
        canard_applied: [0; 4],
        valve_open_mask: 0,
        propellant_q21: 209715,
        supply_scale_q15: 32768,
        vehicle_status: 2,
        actuator_feedback: 0,
        flags: 0,
    }
}
fn aid() -> AdvancedAidCell {
    AdvancedAidCell {
        session: LOCAL_SESSION,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: ADVANCED_AID_ATTITUDE
            | ADVANCED_AID_BAROMETER
            | ADVANCED_AID_GPS
            | ADVANCED_AID_CONTINUITY,
        events: 0,
        onboard_time_q18: 0,
        barometer_q13: 0,
        gps_position_q13: [0; 3],
        gps_velocity_q19: [0; 3],
        attitude_vector: [0; 3],
        continuity: 1,
        deployment_feedback: 0,
        vehicle_status: 2,
        clock_flags: 0,
    }
}
fn computer() -> Option<AllocatedAdvancedFlightComputer> {
    let cfg = reference_mixed_flight_config()?;
    let base = AdvancedFlightComputer::new(cfg, [0; 3], [0; 3])?;
    AllocatedAdvancedFlightComputer::new(base, reference_mixed_allocator_config())
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        p32(0, 0);
        c64_timer::prepare_cia_timing()
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut f = computer().unwrap_or_else(|| fail(1));
    unsafe { c64_timer::start_cia_timer() }
    let a = f.tick(Some(fast(0, true)), Some(aid()));
    let aided = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe { c64_timer::start_cia_timer() }
    let b = f.tick(Some(fast(1, true)), None);
    let fastc = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe { c64_timer::start_cia_timer() }
    let c = f.tick(Some(fast(2, false)), None);
    let fallback = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    let worst = aided.max(fastc).max(fallback);
    let status = u16::from(worst > BUDGET)
        | (u16::from(a.base.local.safe) << 1)
        | (u16::from(b.base.local.safe) << 2)
        | (u16::from(c.base.local.safe) << 3);
    unsafe {
        p16(4, 1);
        p16(6, status);
        p32(8, overhead);
        p32(12, aided);
        p32(16, fastc);
        p32(20, fallback);
        p32(24, worst);
        p32(28, BUDGET);
        p32(32, c.allocator_checksum);
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        p32(0, MAGIC)
    }
    loop {}
}
