#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_mission::{
    execute_hobby_mission_observed, HobbyMissionExecutionError, HobbyMissionObservation,
    HobbyMissionObserver,
};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};

const MAGIC: u32 = 0x3752_4b53;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const TRACE_COUNT: usize = 129;
const RECORD_BASE: usize = 12;
const LAST_BASE: usize = RECORD_BASE + TRACE_COUNT * 4;
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

struct TraceObserver {
    checksums: [u32; TRACE_COUNT],
    count: usize,
    last: [i32; 12],
    last_events: u32,
    last_checksum: u32,
}

impl TraceObserver {
    const fn new() -> Self {
        Self {
            checksums: [0; TRACE_COUNT],
            count: 0,
            last: [0; 12],
            last_events: 0,
            last_checksum: 0,
        }
    }
}

impl HobbyMissionObserver for TraceObserver {
    type Error = ();

    fn observe(&mut self, observation: HobbyMissionObservation) -> Result<(), Self::Error> {
        let step = observation.state.step as usize;
        if step >= TRACE_COUNT {
            return Err(());
        }
        self.checksums[step] = observation.checksum;
        self.count = step + 1;
        self.last = [
            observation.state.step as i32,
            observation.state.time.raw(),
            observation.state.altitude.raw(),
            observation.state.velocity.raw(),
            observation.state.acceleration.raw(),
            observation.state.mass.raw(),
            observation.state.propellant.raw(),
            observation.state.impulse_consumed_q16,
            observation.state.phase as i32,
            observation.thrust_raw_q13,
            observation.dynamic_pressure.raw(),
            observation.mach.map_or(0, |value| value.raw()),
        ];
        self.last_events = observation.events;
        self.last_checksum = observation.checksum;
        if step + 1 == TRACE_COUNT {
            Err(())
        } else {
            Ok(())
        }
    }
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { write_u32(0, 0) };
    let vehicle = parse_vehicle_pack(VEHICLE_BYTES).unwrap_or_else(|_| fail(1));
    let motor = parse_motor_pack(MOTOR_BYTES).unwrap_or_else(|_| fail(2));
    let mission = parse_mission_pack(MISSION_BYTES).unwrap_or_else(|_| fail(3));
    let mut trace = TraceObserver::new();
    match execute_hobby_mission_observed(vehicle, &motor, mission, &mut trace) {
        Err(HobbyMissionExecutionError::Observer(())) if trace.count == TRACE_COUNT => {}
        Err(HobbyMissionExecutionError::Configuration) => fail(4),
        _ => fail(5),
    }
    unsafe {
        write_u16(4, 1);
        write_u16(6, 0);
        write_u16(8, trace.count as u16);
        write_u16(10, 0);
        let mut index = 0usize;
        while index < trace.count {
            write_u32(RECORD_BASE + index * 4, trace.checksums[index]);
            index += 1;
        }
        index = 0;
        while index < trace.last.len() {
            write_i32(LAST_BASE + index * 4, trace.last[index]);
            index += 1;
        }
        write_u32(LAST_BASE + 48, trace.last_events);
        write_u32(LAST_BASE + 52, trace.last_checksum);
        core::ptr::write_volatile(BORDER, 5);
        write_u32(0, MAGIC);
    }
    loop {}
}
