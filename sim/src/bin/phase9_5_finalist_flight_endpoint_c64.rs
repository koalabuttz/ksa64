#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_flight::phase9_5_allocator::AllocatedAdvancedFlightComputer;
use ksa64_interface::phase9_5::*;
use ksa64_sim::phase9_5_bootstrap::{parse_flight_bootstrap, KFB9_LENGTH};

const BOX: *mut u8 = 0xc800 as *mut u8;
const CONFIG: *const u8 = 0xca00 as *const u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3942_4d4b;
const RESULT_MAGIC: u32 = 0x3946_4c4b;
const FAST_AT: usize = 16;
const AID_AT: usize = FAST_AT + ADVANCED_FAST_SENSOR_LENGTH;
const COMMAND_AT: usize = AID_AT + ADVANCED_AID_LENGTH;
const STATUS_AT: usize = COMMAND_AT + ADVANCED_COMMAND_LENGTH;

unsafe fn r(offset: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(offset))
}
unsafe fn w(offset: usize, value: u8) {
    core::ptr::write_volatile(BOX.add(offset), value)
}
unsafe fn w8(pointer: *mut u8, offset: usize, value: u8) {
    core::ptr::write_volatile(pointer.add(offset), value)
}
unsafe fn w16(pointer: *mut u8, offset: usize, value: u16) {
    w8(pointer, offset, value as u8);
    w8(pointer, offset + 1, (value >> 8) as u8)
}
unsafe fn w32(pointer: *mut u8, offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < 4 {
        w8(pointer, offset + index, bytes[index]);
        index += 1;
    }
}
fn input<const N: usize>(offset: usize) -> [u8; N] {
    let mut bytes = [0; N];
    let mut index = 0;
    while index < N {
        bytes[index] = unsafe { r(offset + index) };
        index += 1;
    }
    bytes
}
fn output(offset: usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        unsafe { w(offset + index, bytes[index]) };
        index += 1;
    }
}
fn stop(code: u16, epoch: u16, nav: u32, flight: u32, allocator: u32) -> ! {
    unsafe {
        w16(RESULT, 4, 1);
        w16(RESULT, 6, code);
        w16(RESULT, 8, epoch);
        w32(RESULT, 12, nav);
        w32(RESULT, 16, flight);
        w32(RESULT, 20, allocator);
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
        let mut index = 0;
        while index < STATUS_AT + ADVANCED_STATUS_LENGTH {
            w(index, 0);
            index += 1;
        }
        w32(BOX, 0, BOX_MAGIC);
    }
    while unsafe { r(11) } == 0 && unsafe { r(10) } == 0 {}
    if unsafe { r(10) } != 0 {
        stop(0, 0, 0, 0, 0)
    }
    let bytes = unsafe { core::slice::from_raw_parts(CONFIG, KFB9_LENGTH) };
    let bootstrap = parse_flight_bootstrap(bytes).unwrap_or_else(|_| stop(12, 0, 0, 0, 0));
    let base = AdvancedFlightComputer::new(
        bootstrap.flight,
        bootstrap.initial_position_q13,
        bootstrap.attitude_target,
    )
    .unwrap_or_else(|| stop(16, 0, 0, 0, 0));
    let mut computer = AllocatedAdvancedFlightComputer::new(base, bootstrap.allocator)
        .unwrap_or_else(|| stop(17, 0, 0, 0, 0));
    let (mut seen, mut epoch, mut nav, mut flight, mut allocator) = (0u8, 0u16, 0u32, 0u32, 0u32);
    loop {
        if unsafe { r(10) } != 0 {
            stop(0, epoch, nav, flight, allocator)
        }
        let sequence = unsafe { r(4) };
        if sequence == seen {
            continue;
        }
        let fast = parse_advanced_fast_sensor(&input::<ADVANCED_FAST_SENSOR_LENGTH>(FAST_AT))
            .unwrap_or_else(|_| stop(1, epoch, nav, flight, allocator));
        let aid = if unsafe { r(8) } != 0 {
            Some(
                parse_advanced_aid(&input::<ADVANCED_AID_LENGTH>(AID_AT))
                    .unwrap_or_else(|_| stop(2, epoch, nav, flight, allocator)),
            )
        } else {
            None
        };
        let evidence = computer.tick(Some(fast), aid);
        epoch = fast.measurement_epoch;
        nav = evidence.base.local.navigation.checksum;
        flight = evidence.base.local.flight_checksum;
        allocator = evidence.allocator_checksum;
        let mut command = [0; ADVANCED_COMMAND_LENGTH];
        write_advanced_command(&evidence.command, &mut command)
            .unwrap_or_else(|_| stop(3, epoch, nav, flight, allocator));
        output(COMMAND_AT, &command);
        unsafe { w(9, 0) };
        if let Some(status) = evidence.status {
            let mut status_bytes = [0; ADVANCED_STATUS_LENGTH];
            write_advanced_status(&status, &mut status_bytes)
                .unwrap_or_else(|_| stop(4, epoch, nav, flight, allocator));
            output(STATUS_AT, &status_bytes);
            unsafe { w(9, 1) };
        }
        seen = sequence;
        unsafe {
            w(5, seen);
            w(6, seen)
        };
    }
}
