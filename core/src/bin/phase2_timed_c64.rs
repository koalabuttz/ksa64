#![no_std]
#![no_main]

use core::convert::Infallible;
use core::panic::PanicInfo;

use ksa64_core::c64_timer;
use ksa64_core::phase2_mission::execute_phase2_mission;
use ksa64_core::phase2_scenario::ksa2a_timing_fixture;
use ksa64_core::phase2_telemetry::{
    run_phase2_mission_with_telemetry, Phase2TelemetrySink, PHASE2_TELEMETRY_FRAME_LENGTH,
    PHASE2_TELEMETRY_HEADER_LENGTH,
};

const MAGIC: u32 = 0x3250_544b;
const RESULT: *mut u32 = 0xc000 as *mut u32;
const HEADER: *mut u8 = 0xc100 as *mut u8;
const FRAME: *mut u8 = 0xc140 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;

struct VolatileSink {
    frames: u32,
    bytes: u32,
}

impl VolatileSink {
    const fn new() -> Self {
        Self {
            frames: 0,
            bytes: 0,
        }
    }

    unsafe fn copy(address: *mut u8, source: &[u8]) {
        let mut index = 0usize;
        while index < source.len() {
            core::ptr::write_volatile(address.add(index), source[index]);
            index += 1;
        }
    }
}

impl Phase2TelemetrySink for VolatileSink {
    type Error = Infallible;

    fn write_header(
        &mut self,
        header: &[u8; PHASE2_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        unsafe { Self::copy(HEADER, header) };
        self.bytes += PHASE2_TELEMETRY_HEADER_LENGTH as u32;
        Ok(())
    }

    fn write_frame(
        &mut self,
        frame: &[u8; PHASE2_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error> {
        unsafe { Self::copy(FRAME, frame) };
        self.frames += 1;
        self.bytes += PHASE2_TELEMETRY_FRAME_LENGTH as u32;
        Ok(())
    }
}

unsafe fn read_u32(address: *mut u8, offset: usize) -> u32 {
    (core::ptr::read_volatile(address.add(offset)) as u32)
        | ((core::ptr::read_volatile(address.add(offset + 1)) as u32) << 8)
        | ((core::ptr::read_volatile(address.add(offset + 2)) as u32) << 16)
        | ((core::ptr::read_volatile(address.add(offset + 3)) as u32) << 24)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { core::ptr::write_volatile(BORDER, 2) };
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        core::ptr::write_volatile(RESULT, 0);
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let scenario = ksa2a_timing_fixture();

    unsafe { c64_timer::start_cia_timer() };
    let raw = execute_phase2_mission(scenario);
    let raw_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut sink = VolatileSink::new();
    unsafe { c64_timer::start_cia_timer() };
    let recorded = run_phase2_mission_with_telemetry(scenario, &mut sink);
    let recorded_elapsed = unsafe { c64_timer::stop_cia_timer() };

    let mut status = 0u16;
    let (raw, recorded) = match (raw, recorded) {
        (Ok(raw), Ok(recorded)) => (raw, recorded),
        _ => {
            status = 1;
            unsafe {
                core::ptr::write_volatile(0xc006 as *mut u16, status);
                core::ptr::write_volatile(RESULT, MAGIC);
            }
            loop {}
        }
    };
    if raw.truth() != recorded.mission().truth()
        || raw.truth().step() != 8
        || sink.frames != 2
        || sink.bytes != 168
        || unsafe { read_u32(FRAME, 0) } != 8
    {
        status = 2;
    }
    let truth = raw.truth();
    unsafe {
        core::ptr::write_volatile(0xc004 as *mut u16, 1);
        core::ptr::write_volatile(0xc006 as *mut u16, status);
        core::ptr::write_volatile(0xc008 as *mut u32, raw_elapsed);
        core::ptr::write_volatile(0xc00c as *mut u32, raw_elapsed.wrapping_sub(overhead));
        core::ptr::write_volatile(0xc010 as *mut u32, recorded_elapsed);
        core::ptr::write_volatile(0xc014 as *mut u32, recorded_elapsed.wrapping_sub(overhead));
        core::ptr::write_volatile(0xc018 as *mut u32, overhead);
        core::ptr::write_volatile(0xc01c as *mut u32, truth.step());
        core::ptr::write_volatile(0xc020 as *mut u32, sink.frames);
        core::ptr::write_volatile(0xc024 as *mut u32, recorded.mission().state_checksum());
        core::ptr::write_volatile(0xc028 as *mut i32, truth.radius().raw());
        core::ptr::write_volatile(0xc02c as *mut i32, truth.downrange().raw());
        core::ptr::write_volatile(0xc030 as *mut i32, truth.radial_velocity().raw());
        core::ptr::write_volatile(0xc034 as *mut i32, truth.specific_angular_momentum().raw());
        core::ptr::write_volatile(0xc038 as *mut i32, truth.total_mass().raw());
        core::ptr::write_volatile(0xc03c as *mut i32, truth.active_propellant().raw());
        core::ptr::write_volatile(0xc040 as *mut u32, read_u32(FRAME, 60));
        core::ptr::write_volatile(0xc044 as *mut u32, sink.bytes);
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        core::ptr::write_volatile(RESULT, MAGIC);
    }
    loop {}
}
