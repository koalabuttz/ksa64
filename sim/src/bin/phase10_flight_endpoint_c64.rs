#![no_std]
#![no_main]

use core::panic::PanicInfo;
use ksa64_flight::phase10::{ksa_g10r_reference_flight_config, GlobalFlightComputer};
use ksa64_interface::phase10::*;

const BOX: *mut u8 = 0xc800 as *mut u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3042_4d4b;
const RESULT_MAGIC: u32 = 0x3046_4c4b;
const FAST_AT: usize = 16;
const AID_AT: usize = FAST_AT + GLOBAL_FAST_SENSOR_LENGTH;
const TRANSITION_AT: usize = AID_AT + GLOBAL_AID_FRAME_LENGTH;
const COMMAND_AT: usize = TRANSITION_AT + GLOBAL_TRANSITION_LENGTH;
const STATUS_AT: usize = COMMAND_AT + GLOBAL_COMMAND_LENGTH;
const MAILBOX_LENGTH: usize = STATUS_AT + GLOBAL_STATUS_LENGTH;

unsafe fn read(offset: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(offset))
}

unsafe fn write(offset: usize, value: u8) {
    core::ptr::write_volatile(BOX.add(offset), value)
}

unsafe fn write8(pointer: *mut u8, offset: usize, value: u8) {
    core::ptr::write_volatile(pointer.add(offset), value)
}

unsafe fn write16(pointer: *mut u8, offset: usize, value: u16) {
    write8(pointer, offset, value as u8);
    write8(pointer, offset + 1, (value >> 8) as u8);
}

unsafe fn write32(pointer: *mut u8, offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        write8(pointer, offset + index, bytes[index]);
        index += 1;
    }
}

fn input<const N: usize>(offset: usize) -> [u8; N] {
    let mut bytes = [0; N];
    let mut index = 0;
    while index < N {
        bytes[index] = unsafe { read(offset + index) };
        index += 1;
    }
    bytes
}

fn output(offset: usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        unsafe { write(offset + index, bytes[index]) };
        index += 1;
    }
}

fn stop(code: u16, epoch: u16, navigation: u32, flight: u32, command: u32) -> ! {
    unsafe {
        write16(RESULT, 4, 1);
        write16(RESULT, 6, code);
        write16(RESULT, 8, epoch);
        write16(RESULT, 10, 0);
        write32(RESULT, 12, navigation);
        write32(RESULT, 16, flight);
        write32(RESULT, 20, command);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        write32(RESULT, 0, RESULT_MAGIC);
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
        write32(RESULT, 0, 0);
        let mut index = 0;
        while index < MAILBOX_LENGTH {
            write(index, 0);
            index += 1;
        }
        write32(BOX, 0, BOX_MAGIC);
    }
    while unsafe { read(4) } == 0 && unsafe { read(10) } == 0 {}
    let config = ksa_g10r_reference_flight_config();
    let mut computer = GlobalFlightComputer::new(config).unwrap_or_else(|| stop(10, 0, 0, 0, 0));
    let mut seen = 0u8;
    let mut epoch = 0u16;
    let mut navigation = 0u32;
    let mut flight_checksum = 0u32;
    let mut command_checksum = 0u32;
    loop {
        if unsafe { read(10) } != 0 {
            stop(0, epoch, navigation, flight_checksum, command_checksum)
        }
        let sequence = unsafe { read(4) };
        if sequence == seen {
            continue;
        }
        let fast = parse_global_fast_sensor(&input::<GLOBAL_FAST_SENSOR_LENGTH>(FAST_AT))
            .unwrap_or_else(|_| stop(1, epoch, navigation, flight_checksum, command_checksum));
        let aid = if unsafe { read(8) } != 0 {
            Some(
                parse_global_aid_frame(&input::<GLOBAL_AID_FRAME_LENGTH>(AID_AT)).unwrap_or_else(
                    |_| stop(2, epoch, navigation, flight_checksum, command_checksum),
                ),
            )
        } else {
            None
        };
        let transition = if unsafe { read(11) } != 0 {
            Some(
                parse_global_transition(&input::<GLOBAL_TRANSITION_LENGTH>(TRANSITION_AT))
                    .unwrap_or_else(|_| {
                        stop(3, epoch, navigation, flight_checksum, command_checksum)
                    }),
            )
        } else {
            None
        };
        let evidence = computer.tick(Some(fast), aid, transition);
        epoch = fast.measurement_epoch;
        navigation = evidence.navigation.checksum;
        flight_checksum = evidence.flight_checksum;
        command_checksum = evidence.command.command_checksum;
        let mut command = [0; GLOBAL_COMMAND_LENGTH];
        write_global_command(&evidence.command, &mut command)
            .unwrap_or_else(|_| stop(4, epoch, navigation, flight_checksum, command_checksum));
        output(COMMAND_AT, &command);
        unsafe { write(9, 0) };
        if let Some(status) = evidence.status {
            let mut bytes = [0; GLOBAL_STATUS_LENGTH];
            write_global_status(&status, &mut bytes)
                .unwrap_or_else(|_| stop(5, epoch, navigation, flight_checksum, command_checksum));
            output(STATUS_AT, &bytes);
            unsafe { write(9, 1) };
        }
        seen = sequence;
        unsafe {
            write(5, seen);
            write(6, seen);
        }
    }
}
