#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_interface::phase9_5::*;
use ksa64_sim::phase8_5::LOCAL_SESSION;
use ksa64_sim::phase9_5::reference_mixed_flight_config;
const M: u32 = 0x3942_544d;
const R: *mut u8 = 0xc000 as *mut u8;
const B: *mut u8 = 0xd020 as *mut u8;
unsafe fn p(o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 4 {
        core::ptr::write_volatile(R.add(o + i), b[i]);
        i += 1
    }
}
fn end(c: u32, v: u32) -> ! {
    unsafe {
        p(4, c);
        p(8, v);
        core::ptr::write_volatile(B, if c == 0 { 5 } else { 2 });
        p(0, M)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    end(u32::MAX, 0)
}
fn f() -> AdvancedFastSensorCell {
    AdvancedFastSensorCell {
        session: LOCAL_SESSION,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: ADVANCED_VALID_MASK,
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
fn a() -> AdvancedAidCell {
    AdvancedAidCell {
        session: LOCAL_SESSION,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: ADVANCED_AID_VALID_MASK,
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
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        p(0, 0);
        c64_timer::prepare_cia_timing()
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut c = AdvancedFlightComputer::new(
        reference_mixed_flight_config().unwrap_or_else(|| end(1, 0)),
        [0; 3],
        [0; 3],
    )
    .unwrap_or_else(|| end(2, 0));
    unsafe { c64_timer::start_cia_timer() }
    let e = c.tick(Some(f()), Some(a()));
    let cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    end(u32::from(e.local.safe), cycles)
}
