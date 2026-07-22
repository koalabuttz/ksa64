#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

use ksa64_phase0_rust::{
    c64_timer, divide_fraction_q16, divide_scaled_manual, multiply_scaled_manual,
};

const RESULT_MAGIC: u32 = 0x5250_534b;
const RESULT_SCHEMA: u16 = 1;
const CANDIDATE_RUST: u16 = 1;
const ITERATIONS: u16 = 512;

const MULTIPLY_EXPECTED: u32 = 20_084 * ITERATIONS as u32;
const DIVIDE_EXPECTED: u32 = 1_449_525 * ITERATIONS as u32;
const FRACTION_EXPECTED: u32 = 32_768 * ITERATIONS as u32;

const MULTIPLY_A: *mut i32 = 0xc080 as *mut i32;
const MULTIPLY_B: *mut i32 = 0xc084 as *mut i32;
const DIVIDE_NUMERATOR: *mut i32 = 0xc08c as *mut i32;
const DIVIDE_DENOMINATOR: *mut i32 = 0xc090 as *mut i32;
const FRACTION_NUMERATOR: *mut i32 = 0xc098 as *mut i32;
const FRACTION_DENOMINATOR: *mut i32 = 0xc09c as *mut i32;

const RESULT_BASE: usize = 0xc100;
const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { write_volatile(BORDER_COLOR, 2) };
    loop {}
}

unsafe fn initialize_inputs() {
    write_volatile(MULTIPLY_A, 2_048_000);
    write_volatile(MULTIPLY_B, 2_632_453);
    write_volatile(DIVIDE_NUMERATOR, 11_059);
    write_volatile(DIVIDE_DENOMINATOR, 2_048_000);
    write_volatile(FRACTION_NUMERATOR, 4_096);
    write_volatile(FRACTION_DENOMINATOR, 8_192);
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        write_volatile(RESULT_BASE as *mut u32, 0);
        initialize_inputs();
        c64_timer::prepare_cia_timing();
    }

    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };

    let mut multiply_accumulator = 0u32;
    let mut iteration = 0u16;
    unsafe { c64_timer::start_cia_timer() };
    while iteration < ITERATIONS {
        let value = multiply_scaled_manual(
            unsafe { read_volatile(MULTIPLY_A) },
            unsafe { read_volatile(MULTIPLY_B) },
            28,
        )
        .unwrap_or(0);
        multiply_accumulator = multiply_accumulator.wrapping_add(value as u32);
        iteration += 1;
    }
    let multiply_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut divide_accumulator = 0u32;
    iteration = 0;
    unsafe { c64_timer::start_cia_timer() };
    while iteration < ITERATIONS {
        let value = divide_scaled_manual(
            unsafe { read_volatile(DIVIDE_NUMERATOR) },
            unsafe { read_volatile(DIVIDE_DENOMINATOR) },
            28,
        )
        .unwrap_or(0);
        divide_accumulator = divide_accumulator.wrapping_add(value as u32);
        iteration += 1;
    }
    let divide_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut fraction_accumulator = 0u32;
    iteration = 0;
    unsafe { c64_timer::start_cia_timer() };
    while iteration < ITERATIONS {
        let value = divide_fraction_q16(unsafe { read_volatile(FRACTION_NUMERATOR) }, unsafe {
            read_volatile(FRACTION_DENOMINATOR)
        });
        fraction_accumulator = fraction_accumulator.wrapping_add(value as u32);
        iteration += 1;
    }
    let fraction_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut status = 0u16;
    if multiply_accumulator != MULTIPLY_EXPECTED {
        status |= 1;
    }
    if divide_accumulator != DIVIDE_EXPECTED {
        status |= 2;
    }
    if fraction_accumulator != FRACTION_EXPECTED {
        status |= 4;
    }

    unsafe {
        write_volatile((RESULT_BASE + 4) as *mut u16, RESULT_SCHEMA);
        write_volatile((RESULT_BASE + 6) as *mut u16, CANDIDATE_RUST);
        write_volatile((RESULT_BASE + 8) as *mut u16, status);
        write_volatile((RESULT_BASE + 10) as *mut u16, ITERATIONS);
        write_volatile((RESULT_BASE + 12) as *mut u32, overhead);
        write_volatile((RESULT_BASE + 16) as *mut u32, multiply_elapsed);
        write_volatile((RESULT_BASE + 20) as *mut u32, divide_elapsed);
        write_volatile((RESULT_BASE + 24) as *mut u32, fraction_elapsed);
        write_volatile((RESULT_BASE + 28) as *mut u32, multiply_accumulator);
        write_volatile((RESULT_BASE + 32) as *mut u32, divide_accumulator);
        write_volatile((RESULT_BASE + 36) as *mut u32, fraction_accumulator);
        write_volatile(BORDER_COLOR, if status == 0 { 5 } else { 2 });
        write_volatile(RESULT_BASE as *mut u32, RESULT_MAGIC);
    }

    loop {}
}
