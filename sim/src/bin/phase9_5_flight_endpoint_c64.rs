#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_flight::phase9_5_allocator::AllocatedAdvancedFlightComputer;
use ksa64_interface::phase9_5::*;
use ksa64_sim::phase9_5::{reference_mixed_allocator_config, reference_mixed_flight_config};
const BOX: *mut u8 = 0xc800 as *mut u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3942_4d4b;
const RESULT_MAGIC: u32 = 0x3946_4c4b;
const FAST_AT: usize = 16;
const AID_AT: usize = FAST_AT + ADVANCED_FAST_SENSOR_LENGTH;
const COMMAND_AT: usize = AID_AT + ADVANCED_AID_LENGTH;
const STATUS_AT: usize = COMMAND_AT + ADVANCED_COMMAND_LENGTH;
unsafe fn r(o: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(o))
}
unsafe fn w(o: usize, v: u8) {
    core::ptr::write_volatile(BOX.add(o), v)
}
unsafe fn w16(p: *mut u8, o: usize, v: u16) {
    w8(p, o, v as u8);
    w8(p, o + 1, (v >> 8) as u8)
}
unsafe fn w8(p: *mut u8, o: usize, v: u8) {
    core::ptr::write_volatile(p.add(o), v)
}
unsafe fn w32(p: *mut u8, o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 4 {
        w8(p, o + i, b[i]);
        i += 1;
    }
}
fn input<const N: usize>(o: usize) -> [u8; N] {
    let mut b = [0; N];
    let mut i = 0;
    while i < N {
        b[i] = unsafe { r(o + i) };
        i += 1;
    }
    b
}
fn output(o: usize, b: &[u8]) {
    let mut i = 0;
    while i < b.len() {
        unsafe { w(o + i, b[i]) };
        i += 1;
    }
}
fn stop(code: u16, epoch: u16, nav: u32, flight: u32, alloc: u32) -> ! {
    unsafe {
        w16(RESULT, 4, 1);
        w16(RESULT, 6, code);
        w16(RESULT, 8, epoch);
        w32(RESULT, 12, nav);
        w32(RESULT, 16, flight);
        w32(RESULT, 20, alloc);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        w32(RESULT, 0, RESULT_MAGIC);
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff, 0, 0, 0, 0)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        w32(RESULT, 0, 0);
        let mut i = 0;
        while i < STATUS_AT + ADVANCED_STATUS_LENGTH {
            w(i, 0);
            i += 1;
        }
        w32(BOX, 0, BOX_MAGIC);
    }
    while unsafe { r(4) } == 0 && unsafe { r(10) } == 0 {}
    let cfg = reference_mixed_flight_config().unwrap_or_else(|| stop(12, 0, 0, 0, 0));
    let acfg = reference_mixed_allocator_config();
    let mut numeric = ksa64_core::numeric::NumericStatus::CLEAR;
    let axis = ksa64_core::phase8_world::rail_axis_from_mission(
        ksa64_core::phase8_fixtures::FIRESTORM_I211_SPATIAL_MISSION,
        &mut numeric,
    )
    .unwrap_or_else(|_| stop(14, 0, 0, 0, 0));
    let attitude = ksa64_core::phase8_world::attitude_from_rail_axis(axis, &mut numeric)
        .unwrap_or_else(|_| stop(15, 0, 0, 0, 0));
    let target = [
        (attitude.x() >> 15) as i16,
        (attitude.y() >> 15) as i16,
        (attitude.z() >> 15) as i16,
    ];
    let base =
        AdvancedFlightComputer::new(cfg, [0; 3], target).unwrap_or_else(|| stop(16, 0, 0, 0, 0));
    let mut computer =
        AllocatedAdvancedFlightComputer::new(base, acfg).unwrap_or_else(|| stop(17, 0, 0, 0, 0));
    let (mut seen, mut epoch, mut nav, mut flight, mut alloc) = (0u8, 0u16, 0u32, 0u32, 0u32);
    loop {
        if unsafe { r(10) } != 0 {
            stop(0, epoch, nav, flight, alloc)
        }
        let seq = unsafe { r(4) };
        if seq == seen {
            continue;
        }
        let fast = parse_advanced_fast_sensor(&input::<ADVANCED_FAST_SENSOR_LENGTH>(FAST_AT))
            .unwrap_or_else(|_| stop(1, epoch, nav, flight, alloc));
        let aid = if unsafe { r(8) } != 0 {
            Some(
                parse_advanced_aid(&input::<ADVANCED_AID_LENGTH>(AID_AT))
                    .unwrap_or_else(|_| stop(2, epoch, nav, flight, alloc)),
            )
        } else {
            None
        };
        let evidence = computer.tick(Some(fast), aid);
        epoch = fast.measurement_epoch;
        nav = evidence.base.local.navigation.checksum;
        flight = evidence.base.local.flight_checksum;
        alloc = evidence.allocator_checksum;
        let mut command = [0; ADVANCED_COMMAND_LENGTH];
        write_advanced_command(&evidence.command, &mut command)
            .unwrap_or_else(|_| stop(3, epoch, nav, flight, alloc));
        output(COMMAND_AT, &command);
        unsafe { w(9, 0) };
        if let Some(status) = evidence.status {
            let mut bytes = [0; ADVANCED_STATUS_LENGTH];
            write_advanced_status(&status, &mut bytes)
                .unwrap_or_else(|_| stop(4, epoch, nav, flight, alloc));
            output(STATUS_AT, &bytes);
            unsafe { w(9, 1) };
        }
        seen = seq;
        unsafe {
            w(5, seen);
            w(6, seen)
        };
    }
}
