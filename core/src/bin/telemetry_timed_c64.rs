#![no_std]
#![no_main]

use core::convert::Infallible;
use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::mission::{run_vertical_dynamics, run_vertical_mission};
use ksa64_core::scenario::{parse_scenario_image, SCENARIO_IMAGE_LENGTH};
use ksa64_core::telemetry::{
    run_vertical_mission_with_telemetry, TelemetrySink, TELEMETRY_FRAME_LENGTH,
    TELEMETRY_HEADER_LENGTH,
};

const TIMING_MAGIC: u32 = 0x3254_534b;
const TIMING_SCHEMA: u16 = 1;

const MAGIC_ADDRESS: *mut u32 = 0xc000 as *mut u32;
const SCHEMA_ADDRESS: *mut u16 = 0xc004 as *mut u16;
const STATUS_ADDRESS: *mut u16 = 0xc006 as *mut u16;
const DYNAMICS_ELAPSED_ADDRESS: *mut u32 = 0xc008 as *mut u32;
const DYNAMICS_NET_ADDRESS: *mut u32 = 0xc00c as *mut u32;
const MISSION_ELAPSED_ADDRESS: *mut u32 = 0xc010 as *mut u32;
const MISSION_NET_ADDRESS: *mut u32 = 0xc014 as *mut u32;
const TELEMETRY_ELAPSED_ADDRESS: *mut u32 = 0xc018 as *mut u32;
const TELEMETRY_NET_ADDRESS: *mut u32 = 0xc01c as *mut u32;
const OVERHEAD_ADDRESS: *mut u32 = 0xc020 as *mut u32;
const STEP_ADDRESS: *mut u32 = 0xc024 as *mut u32;
const TIME_ADDRESS: *mut i32 = 0xc028 as *mut i32;
const ALTITUDE_ADDRESS: *mut i32 = 0xc02c as *mut i32;
const VELOCITY_ADDRESS: *mut i32 = 0xc030 as *mut i32;
const ACCELERATION_ADDRESS: *mut i32 = 0xc034 as *mut i32;
const MASS_ADDRESS: *mut i32 = 0xc038 as *mut i32;
const PROPELLANT_ADDRESS: *mut i32 = 0xc03c as *mut i32;
const CHECKSUM_ADDRESS: *mut u32 = 0xc040 as *mut u32;
const CUTOFF_ADDRESS: *mut u16 = 0xc044 as *mut u16;
const FRAMES_ADDRESS: *mut u32 = 0xc048 as *mut u32;
const BYTES_ADDRESS: *mut u32 = 0xc04c as *mut u32;
const FINAL_EVENTS_ADDRESS: *mut u16 = 0xc050 as *mut u16;
const FINAL_FRAME_CRC_ADDRESS: *mut u32 = 0xc054 as *mut u32;

const HEADER_BUFFER: *mut u8 = 0xc100 as *mut u8;
const FRAME_BUFFER: *mut u8 = 0xc120 as *mut u8;
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

struct VolatileDiscardSink {
    frames: u32,
    bytes: u32,
}

impl VolatileDiscardSink {
    const fn new() -> Self {
        Self {
            frames: 0,
            bytes: 0,
        }
    }

    unsafe fn copy_to(address: *mut u8, source: &[u8]) {
        let mut index = 0usize;
        while index < source.len() {
            core::ptr::write_volatile(address.add(index), source[index]);
            index += 1;
        }
    }
}

impl TelemetrySink for VolatileDiscardSink {
    type Error = Infallible;

    fn write_header(&mut self, header: &[u8; TELEMETRY_HEADER_LENGTH]) -> Result<(), Self::Error> {
        unsafe {
            Self::copy_to(HEADER_BUFFER, header);
        }
        self.bytes += TELEMETRY_HEADER_LENGTH as u32;
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8; TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error> {
        unsafe {
            Self::copy_to(FRAME_BUFFER, frame);
        }
        self.frames += 1;
        self.bytes += TELEMETRY_FRAME_LENGTH as u32;
        Ok(())
    }
}

unsafe fn read_buffer_u16(address: *mut u8, offset: usize) -> u16 {
    core::ptr::read_volatile(address.add(offset)) as u16
        | ((core::ptr::read_volatile(address.add(offset + 1)) as u16) << 8)
}

unsafe fn read_buffer_u32(address: *mut u8, offset: usize) -> u32 {
    read_buffer_u16(address, offset) as u32 | ((read_buffer_u16(address, offset + 2) as u32) << 16)
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

    let mut sink = VolatileDiscardSink::new();
    unsafe {
        c64_timer::start_cia_timer();
    }
    let telemetry = run_vertical_mission_with_telemetry(&scenario, &mut sink);
    let telemetry_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut status = 0u16;
    let (dynamics, mission, telemetry) = match (dynamics, mission, telemetry) {
        (Ok(dynamics), Ok(mission), Ok(telemetry)) => (dynamics, mission, telemetry),
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
    let final_events = unsafe { read_buffer_u16(FRAME_BUFFER, 30) };
    let final_frame_crc = unsafe { read_buffer_u32(FRAME_BUFFER, 36) };
    if dynamics.final_truth() != truth
        || dynamics.cutoff_events() != mission.cutoff_events()
        || telemetry.mission() != mission
        || telemetry.frames_written() != expected::TELEMETRY_FRAME_COUNT
        || sink.frames != expected::TELEMETRY_FRAME_COUNT
        || sink.bytes != expected::TELEMETRY_STREAM_LENGTH as u32
        || unsafe { read_buffer_u32(FRAME_BUFFER, 0) } != expected::FINAL_STEP
        || final_events != expected::FINAL_FRAME_EVENTS
        || unsafe { read_buffer_u32(FRAME_BUFFER, 32) } != expected::FINAL_CHECKSUM
        || final_frame_crc != expected::FINAL_FRAME_CRC32
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
        core::ptr::write_volatile(TELEMETRY_ELAPSED_ADDRESS, telemetry_elapsed);
        core::ptr::write_volatile(
            TELEMETRY_NET_ADDRESS,
            telemetry_elapsed.wrapping_sub(overhead),
        );
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
        core::ptr::write_volatile(FRAMES_ADDRESS, sink.frames);
        core::ptr::write_volatile(BYTES_ADDRESS, sink.bytes);
        core::ptr::write_volatile(FINAL_EVENTS_ADDRESS, final_events);
        core::ptr::write_volatile(FINAL_FRAME_CRC_ADDRESS, final_frame_crc);
        core::ptr::write_volatile(BORDER_COLOR, if status == 0 { 5 } else { 2 });
        core::ptr::write_volatile(MAGIC_ADDRESS, TIMING_MAGIC);
    }

    loop {}
}
