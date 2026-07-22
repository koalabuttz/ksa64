#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_phase0_rust::{
    c64_timer, vertical_state_matches_checkpoint, vertical_step_optimized, vertical_vectors,
    VerticalState,
};

const TIMING_MAGIC: u32 = 0x5441_534b;
const TIMING_SCHEMA: u16 = 1;
const CANDIDATE_RUST: u16 = 1;

const MAGIC_ADDRESS: *mut u32 = 0xc000 as *mut u32;
const SCHEMA_ADDRESS: *mut u16 = 0xc004 as *mut u16;
const CANDIDATE_ADDRESS: *mut u16 = 0xc006 as *mut u16;
const STATUS_ADDRESS: *mut u16 = 0xc008 as *mut u16;
const ELAPSED_ADDRESS: *mut u32 = 0xc00c as *mut u32;
const OVERHEAD_ADDRESS: *mut u32 = 0xc010 as *mut u32;
const NET_ADDRESS: *mut u32 = 0xc014 as *mut u32;
const ALTITUDE_ADDRESS: *mut i32 = 0xc018 as *mut i32;
const VELOCITY_ADDRESS: *mut i32 = 0xc01c as *mut i32;
const ACCELERATION_ADDRESS: *mut i32 = 0xc020 as *mut i32;
const MASS_ADDRESS: *mut i32 = 0xc024 as *mut i32;
const PROPELLANT_ADDRESS: *mut i32 = 0xc028 as *mut i32;
const CUTOFF_ADDRESS: *mut u8 = 0xc02c as *mut u8;
const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER_COLOR, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let mut state = VerticalState::initial();
    unsafe {
        core::ptr::write_volatile(MAGIC_ADDRESS, 0);
        c64_timer::prepare_cia_timing();
    }

    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    unsafe {
        c64_timer::start_cia_timer();
    }
    let mut step = 0u16;
    while step < vertical_vectors::VERTICAL_TOTAL_STEPS {
        vertical_step_optimized(&mut state);
        step += 1;
    }
    let elapsed = unsafe { c64_timer::stop_cia_timer() };

    let expected =
        vertical_vectors::VERTICAL_CHECKPOINTS[vertical_vectors::VERTICAL_CHECKPOINTS.len() - 1];
    let status = (!vertical_state_matches_checkpoint(&state, expected)) as u16;
    let net = elapsed.wrapping_sub(overhead);

    unsafe {
        core::ptr::write_volatile(SCHEMA_ADDRESS, TIMING_SCHEMA);
        core::ptr::write_volatile(CANDIDATE_ADDRESS, CANDIDATE_RUST);
        core::ptr::write_volatile(STATUS_ADDRESS, status);
        core::ptr::write_volatile(ELAPSED_ADDRESS, elapsed);
        core::ptr::write_volatile(OVERHEAD_ADDRESS, overhead);
        core::ptr::write_volatile(NET_ADDRESS, net);
        core::ptr::write_volatile(ALTITUDE_ADDRESS, state.altitude_q12);
        core::ptr::write_volatile(VELOCITY_ADDRESS, state.velocity_q24);
        core::ptr::write_volatile(ACCELERATION_ADDRESS, state.acceleration_q28);
        core::ptr::write_volatile(MASS_ADDRESS, state.mass_q12);
        core::ptr::write_volatile(PROPELLANT_ADDRESS, state.propellant_q12);
        core::ptr::write_volatile(CUTOFF_ADDRESS, state.cutoff_events);
        core::ptr::write_volatile(BORDER_COLOR, if status == 0 { 5 } else { 2 });
        core::ptr::write_volatile(MAGIC_ADDRESS, TIMING_MAGIC);
    }

    loop {}
}
