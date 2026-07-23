#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_quantities::{
    DownrangeAngle, PlanarVelocity, Radius, SpecificAngularMomentum,
};
use ksa64_core::planar::{
    advance_vacuum_midpoint, advance_vacuum_semi_implicit, PlanarTruthState, PlanarWorld,
    StagePhase,
};
use ksa64_core::quantities::{Mass, Time};

const MAGIC: u32 = 0x3256_534b;
const STEPS: u16 = 256;
const RESULT: *mut u32 = 0xc000 as *mut u32;
const BORDER: *mut u8 = 0xd020 as *mut u8;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER, 2);
    }
    loop {}
}

fn initial() -> PlanarTruthState {
    PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(EARTH_RADIUS_Q12 + 737_280),
        DownrangeAngle::ZERO,
        PlanarVelocity::ZERO,
        SpecificAngularMomentum::from_raw(838_954_247),
        Mass::from_raw(4096),
        Mass::ZERO,
        0,
        StagePhase::Complete,
    )
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        core::ptr::write_volatile(RESULT, 0);
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let world = PlanarWorld::simple_earth(Time::from_raw(8192));

    let mut semi = initial();
    let mut semi_status = NumericStatus::CLEAR;
    unsafe {
        c64_timer::start_cia_timer();
    }
    let mut count = 0u16;
    while count < STEPS {
        semi = match advance_vacuum_semi_implicit(world, semi, &mut semi_status) {
            Ok(value) => value,
            Err(_) => break,
        };
        count += 1;
    }
    let semi_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut midpoint = initial();
    let mut midpoint_status = NumericStatus::CLEAR;
    unsafe {
        c64_timer::start_cia_timer();
    }
    count = 0;
    while count < STEPS {
        midpoint = match advance_vacuum_midpoint(world, midpoint, &mut midpoint_status) {
            Ok(value) => value,
            Err(_) => break,
        };
        count += 1;
    }
    let midpoint_elapsed = unsafe { c64_timer::stop_cia_timer() };
    let status = if semi_status.is_clear()
        && midpoint_status.is_clear()
        && semi.step() == STEPS as u32
        && midpoint.step() == STEPS as u32
    {
        0u16
    } else {
        1u16
    };

    unsafe {
        core::ptr::write_volatile(0xc004 as *mut u16, 1);
        core::ptr::write_volatile(0xc006 as *mut u16, status);
        core::ptr::write_volatile(0xc008 as *mut u32, semi_elapsed.wrapping_sub(overhead));
        core::ptr::write_volatile(0xc00c as *mut u32, midpoint_elapsed.wrapping_sub(overhead));
        core::ptr::write_volatile(0xc010 as *mut u32, overhead);
        core::ptr::write_volatile(0xc014 as *mut i32, semi.radius().raw());
        core::ptr::write_volatile(0xc018 as *mut i32, semi.radial_velocity().raw());
        core::ptr::write_volatile(0xc01c as *mut i32, midpoint.radius().raw());
        core::ptr::write_volatile(0xc020 as *mut i32, midpoint.radial_velocity().raw());
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        core::ptr::write_volatile(RESULT, MAGIC);
    }
    loop {}
}
