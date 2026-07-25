#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase8_5::LocalFlightComputer;
use ksa64_interface::phase8_5::{
    LocalAidCell, LocalInertialCell, LOCAL_AID_ATTITUDE, LOCAL_AID_BAROMETER, LOCAL_AID_CONTINUITY,
    LOCAL_AID_GPS, LOCAL_INERTIAL_VALID_MASK,
};
use ksa64_sim::phase8_5::{
    local_flight_config, reference_avionics_profile, reference_monitor_capability,
};
const MAGIC: u32 = 0x3854_4c4b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const PAL_RELEASE_BUDGET_80_PERCENT: u32 = 24_631;
unsafe fn u16o(at: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(at), value as u8);
    core::ptr::write_volatile(RESULT.add(at + 1), (value >> 8) as u8);
}
unsafe fn u32o(at: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(at + index), bytes[index]);
        index += 1;
    }
}
fn fail(code: u16) -> ! {
    unsafe {
        u16o(4, 1);
        u16o(6, code);
        core::ptr::write_volatile(BORDER, 2);
        u32o(0, MAGIC);
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
fn inertial(epoch: u16) -> LocalInertialCell {
    LocalInertialCell {
        session: 0x8501,
        measurement_epoch: epoch,
        production_epoch: epoch,
        validity: LOCAL_INERTIAL_VALID_MASK,
        flags: 0,
        platform_angle: [0; 3],
        angular_rate: [0; 3],
        delta_velocity: [0, 0, 4],
        gimbal_applied: [0; 2],
        vehicle_status: 1,
        actuator_feedback: 0,
    }
}
fn aid() -> LocalAidCell {
    LocalAidCell {
        session: 0x8501,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: LOCAL_AID_ATTITUDE | LOCAL_AID_BAROMETER | LOCAL_AID_GPS | LOCAL_AID_CONTINUITY,
        events: 0,
        onboard_time_q18: 0,
        barometer_q13: 0,
        gps_position_q13: [0; 3],
        gps_velocity_q19: [0; 3],
        attitude_vector: [0; 3],
        continuity: 3,
        deployment_feedback: 0,
        vehicle_status: 1,
        clock_flags: 0,
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        u32o(0, 0);
        c64_timer::prepare_cia_timing()
    };
    let capability = reference_monitor_capability(
        ksa64_core::phase8_fixtures::FIRESTORM_SPATIAL_VEHICLE.identity,
    );
    let config = local_flight_config(
        reference_avionics_profile(false),
        capability,
        &ksa64_core::phase8_fixtures::I211W_SPATIAL_MOTOR,
    )
    .unwrap_or_else(|_| fail(1));
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut flight = LocalFlightComputer::new(config, [0; 3], [0; 3]).unwrap_or_else(|| fail(2));
    let inertial_zero = inertial(0);
    let aid_zero = aid();
    unsafe { c64_timer::start_cia_timer() };
    flight.tick_in_place(&inertial_zero, true, &aid_zero, true);
    let aided_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    let aided = flight.evidence();
    let inertial_one = inertial(1);
    unsafe { c64_timer::start_cia_timer() };
    flight.tick_in_place(&inertial_one, true, &aid_zero, false);
    let fast_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    let fast = flight.evidence();
    let status = u16::from(aided.safe || fast.safe)
        | (u16::from(aided_cycles > PAL_RELEASE_BUDGET_80_PERCENT) << 1)
        | (u16::from(fast_cycles > PAL_RELEASE_BUDGET_80_PERCENT) << 2);
    unsafe {
        u16o(4, 1);
        u16o(6, status);
        u32o(8, overhead);
        u32o(12, aided_cycles);
        u32o(16, fast_cycles);
        u32o(20, PAL_RELEASE_BUDGET_80_PERCENT);
        u32o(24, flight.navigation().checksum);
        u32o(28, fast.flight_checksum);
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        u32o(0, MAGIC);
    }
    loop {}
}
