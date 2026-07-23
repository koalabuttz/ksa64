#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::phase2_scenario::ksa2a_fixture;
use ksa64_sim::probe::{
    command_is_safe_after_fault, run_actuator_probe, run_coast_probe, run_composed_probe,
    run_guidance_probe,
};

const MAGIC: u32 = 0x3350_544b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;

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
unsafe fn put_i32(offset: usize, value: i32) {
    put_u32(offset, value as u32)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { core::ptr::write_volatile(BORDER, 2) };
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        put_u32(0, 0);
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let scenario = ksa2a_fixture(false);

    unsafe { c64_timer::start_cia_timer() };
    let composed = run_composed_probe(scenario);
    let composed_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);

    unsafe { c64_timer::start_cia_timer() };
    let guidance = run_guidance_probe(false);
    let guidance_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);

    unsafe { c64_timer::start_cia_timer() };
    let fault = run_guidance_probe(true);
    let fault_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);

    unsafe { c64_timer::start_cia_timer() };
    let coast = run_coast_probe();
    let coast_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);

    unsafe { c64_timer::start_cia_timer() };
    let actuator_hash = run_actuator_probe();
    let actuator_cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);

    let mut status = 0u16;
    let composed = match composed {
        Some(result) => result,
        None => {
            status |= 1;
            ksa64_sim::probe::ComposedProbeResult {
                step: 0,
                radius_q12: 0,
                truth_checksum: 0,
                sensor_checksum: 0,
                nav_checksum: 0,
                flight_checksum: 0,
            }
        }
    };
    let coast = match coast {
        Some(result) => result,
        None => {
            status |= 2;
            ksa64_sim::probe::CoastProbeResult {
                step: 0,
                radius_q12: 0,
                radial_velocity_q24: 0,
                angular_momentum_q14: 0,
            }
        }
    };
    if !command_is_safe_after_fault(fault) {
        status |= 4;
    }
    unsafe {
        put_u16(4, 1);
        put_u16(6, status);
        put_u32(8, overhead);
        put_u32(12, composed_cycles);
        put_u32(16, guidance_cycles);
        put_u32(20, fault_cycles);
        put_u32(24, coast_cycles);
        put_u32(28, actuator_cycles);
        put_u32(32, composed.truth_checksum);
        put_u32(36, composed.sensor_checksum);
        put_u32(40, composed.nav_checksum);
        put_u32(44, composed.flight_checksum);
        put_i32(48, composed.radius_q12);
        put_u32(52, guidance.nav_checksum);
        put_u32(56, guidance.flight_checksum);
        put_u32(60, fault.nav_checksum);
        put_u32(64, fault.flight_checksum);
        put_i32(68, coast.radius_q12);
        put_i32(72, coast.radial_velocity_q24);
        put_u32(76, actuator_hash);
        put_u16(80, guidance.mode as u16 | ((fault.mode as u16) << 8));
        put_u16(82, guidance.alarms | fault.alarms);
        put_u32(84, composed.step);
        put_u32(88, coast.step);
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        put_u32(0, MAGIC);
    }
    loop {}
}
