#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::mission::{run_vertical_dynamics, run_vertical_mission};
use ksa64_core::scenario::{parse_scenario_image, SCENARIO_IMAGE_LENGTH};

const TIMING_MAGIC: u32 = 0x3154_534b;
const TIMING_SCHEMA: u16 = 1;

const MAGIC_ADDRESS: *mut u32 = 0xc000 as *mut u32;
const SCHEMA_ADDRESS: *mut u16 = 0xc004 as *mut u16;
const STATUS_ADDRESS: *mut u16 = 0xc006 as *mut u16;
const DYNAMICS_ELAPSED_ADDRESS: *mut u32 = 0xc008 as *mut u32;
const DYNAMICS_NET_ADDRESS: *mut u32 = 0xc00c as *mut u32;
const MISSION_ELAPSED_ADDRESS: *mut u32 = 0xc010 as *mut u32;
const MISSION_NET_ADDRESS: *mut u32 = 0xc014 as *mut u32;
const OVERHEAD_ADDRESS: *mut u32 = 0xc018 as *mut u32;
const STEP_ADDRESS: *mut u32 = 0xc01c as *mut u32;
const TIME_ADDRESS: *mut i32 = 0xc020 as *mut i32;
const ALTITUDE_ADDRESS: *mut i32 = 0xc024 as *mut i32;
const VELOCITY_ADDRESS: *mut i32 = 0xc028 as *mut i32;
const ACCELERATION_ADDRESS: *mut i32 = 0xc02c as *mut i32;
const MASS_ADDRESS: *mut i32 = 0xc030 as *mut i32;
const PROPELLANT_ADDRESS: *mut i32 = 0xc034 as *mut i32;
const CHECKSUM_ADDRESS: *mut u32 = 0xc038 as *mut u32;
const CUTOFF_ADDRESS: *mut u16 = 0xc03c as *mut u16;
const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;

const SCENARIO_IMAGE: &[u8; SCENARIO_IMAGE_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase0/numeric/scenario-v1.bin"
));

#[allow(dead_code)]
mod expected {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/mission_v1.rs"
    ));
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER_COLOR, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        core::ptr::write_volatile(MAGIC_ADDRESS, 0);
    }
    let scenario = match parse_scenario_image(SCENARIO_IMAGE) {
        Ok(scenario) => scenario,
        Err(_) => {
            unsafe {
                core::ptr::write_volatile(STATUS_ADDRESS, 1);
                core::ptr::write_volatile(MAGIC_ADDRESS, TIMING_MAGIC);
            }
            loop {}
        }
    };

    unsafe {
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };

    unsafe {
        c64_timer::start_cia_timer();
    }
    let dynamics = run_vertical_dynamics(&scenario);
    let dynamics_elapsed = unsafe { c64_timer::stop_cia_timer() };

    unsafe {
        c64_timer::start_cia_timer();
    }
    let mission = run_vertical_mission(&scenario);
    let mission_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut status = 0u16;
    let (dynamics, mission) = match (dynamics, mission) {
        (Ok(dynamics), Ok(mission)) => (dynamics, mission),
        _ => {
            status |= 2;
            unsafe {
                core::ptr::write_volatile(STATUS_ADDRESS, status);
                core::ptr::write_volatile(MAGIC_ADDRESS, TIMING_MAGIC);
            }
            loop {}
        }
    };
    let truth = mission.final_truth();
    if dynamics.final_truth() != truth
        || dynamics.cutoff_events() != mission.cutoff_events()
        || truth.step() != expected::FINAL_STEP
        || truth.time().raw() != expected::FINAL_TIME_Q16
        || truth.altitude().raw() != expected::FINAL_ALTITUDE_Q12
        || truth.velocity().raw() != expected::FINAL_VELOCITY_Q24
        || truth.acceleration().raw() != expected::FINAL_ACCELERATION_Q28
        || truth.total_mass().raw() != expected::FINAL_MASS_Q12
        || truth.propellant().raw() != expected::FINAL_PROPELLANT_Q12
        || mission.checksum() != expected::FINAL_CHECKSUM
        || mission.cutoff_events() != expected::CUTOFF_EVENTS
    {
        status |= 4;
    }

    unsafe {
        core::ptr::write_volatile(SCHEMA_ADDRESS, TIMING_SCHEMA);
        core::ptr::write_volatile(STATUS_ADDRESS, status);
        core::ptr::write_volatile(DYNAMICS_ELAPSED_ADDRESS, dynamics_elapsed);
        core::ptr::write_volatile(
            DYNAMICS_NET_ADDRESS,
            dynamics_elapsed.wrapping_sub(overhead),
        );
        core::ptr::write_volatile(MISSION_ELAPSED_ADDRESS, mission_elapsed);
        core::ptr::write_volatile(MISSION_NET_ADDRESS, mission_elapsed.wrapping_sub(overhead));
        core::ptr::write_volatile(OVERHEAD_ADDRESS, overhead);
        core::ptr::write_volatile(STEP_ADDRESS, truth.step());
        core::ptr::write_volatile(TIME_ADDRESS, truth.time().raw());
        core::ptr::write_volatile(ALTITUDE_ADDRESS, truth.altitude().raw());
        core::ptr::write_volatile(VELOCITY_ADDRESS, truth.velocity().raw());
        core::ptr::write_volatile(ACCELERATION_ADDRESS, truth.acceleration().raw());
        core::ptr::write_volatile(MASS_ADDRESS, truth.total_mass().raw());
        core::ptr::write_volatile(PROPELLANT_ADDRESS, truth.propellant().raw());
        core::ptr::write_volatile(CHECKSUM_ADDRESS, mission.checksum());
        core::ptr::write_volatile(CUTOFF_ADDRESS, mission.cutoff_events());
        core::ptr::write_volatile(BORDER_COLOR, if status == 0 { 5 } else { 2 });
        core::ptr::write_volatile(MAGIC_ADDRESS, TIMING_MAGIC);
    }

    loop {}
}
