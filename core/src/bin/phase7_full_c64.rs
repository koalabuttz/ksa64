#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_mission::{
    execute_hobby_mission, HobbyMissionOutcome, HOBBY_EVENT_APOGEE, HOBBY_EVENT_BURNOUT,
    HOBBY_EVENT_DROGUE, HOBBY_EVENT_END, HOBBY_EVENT_GROUND, HOBBY_EVENT_IGNITION,
    HOBBY_EVENT_LIFTOFF, HOBBY_EVENT_MAIN, HOBBY_EVENT_RAIL_EXIT,
};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};

const MAGIC: u32 = 0x3754_4b53;
const EXPECTED_CHECKSUM: u32 = 0xa61c_5720;
const REQUIRED_EVENTS: u32 = HOBBY_EVENT_IGNITION
    | HOBBY_EVENT_LIFTOFF
    | HOBBY_EVENT_RAIL_EXIT
    | HOBBY_EVENT_BURNOUT
    | HOBBY_EVENT_APOGEE
    | HOBBY_EVENT_DROGUE
    | HOBBY_EVENT_MAIN
    | HOBBY_EVENT_GROUND
    | HOBBY_EVENT_END;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;

const VEHICLE_BYTES: &[u8; KVP7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm54.kvp7"
));
const MOTOR_BYTES: &[u8; KMP7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/aerotech-i211w.kmp7"
));
const MISSION_BYTES: &[u8; KMC7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm-i211.kmc7"
));

unsafe fn write_u16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}

unsafe fn write_u32(offset: usize, value: u32) {
    let mut index = 0usize;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(offset + index), (value >> (index * 8)) as u8);
        index += 1;
    }
}

unsafe fn write_i32(offset: usize, value: i32) {
    write_u32(offset, value as u32);
}

fn fail(code: u16) -> ! {
    unsafe {
        write_u16(4, 1);
        write_u16(6, code);
        core::ptr::write_volatile(BORDER, 2);
        write_u32(0, MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}

fn outcome_code(outcome: HobbyMissionOutcome) -> u8 {
    match outcome {
        HobbyMissionOutcome::Landed => 0,
        HobbyMissionOutcome::NoLiftoff => 1,
        HobbyMissionOutcome::RecoveryIncomplete => 2,
        HobbyMissionOutcome::NumericFault => 3,
        HobbyMissionOutcome::StepLimit => 4,
        HobbyMissionOutcome::ConfigurationFault => 5,
    }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        write_u32(0, 0);
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let vehicle = parse_vehicle_pack(VEHICLE_BYTES).unwrap_or_else(|_| fail(1));
    let motor = parse_motor_pack(MOTOR_BYTES).unwrap_or_else(|_| fail(2));
    let mission = parse_mission_pack(MISSION_BYTES).unwrap_or_else(|_| fail(3));

    unsafe { c64_timer::start_cia_timer() };
    let result = execute_hobby_mission(vehicle, &motor, mission).unwrap_or_else(|_| fail(4));
    let elapsed = unsafe { c64_timer::stop_cia_timer() };
    let net = elapsed.wrapping_sub(overhead);

    let mut status = 0u16;
    if result.outcome != HobbyMissionOutcome::Landed {
        status |= 1;
    }
    if result.numeric_faults != 0 {
        status |= 2;
    }
    if result.event_history & REQUIRED_EVENTS != REQUIRED_EVENTS {
        status |= 4;
    }
    if result.state_checksum != EXPECTED_CHECKSUM {
        status |= 8;
    }

    unsafe {
        write_u16(4, 1);
        write_u16(6, status);
        write_u32(8, elapsed);
        write_u32(12, overhead);
        write_u32(16, net);
        write_u32(20, result.terminal.step);
        write_i32(24, result.max_altitude.raw());
        write_i32(28, result.ground.velocity_raw);
        write_u32(32, result.state_checksum);
        write_u32(36, result.event_history);
        core::ptr::write_volatile(RESULT.add(40), result.numeric_faults);
        core::ptr::write_volatile(RESULT.add(41), outcome_code(result.outcome));
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        write_u32(0, MAGIC);
    }
    loop {}
}
