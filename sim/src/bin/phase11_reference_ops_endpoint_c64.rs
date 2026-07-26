#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::slice;
use ksa64_flight::phase10::ksa_g10r_reference_flight_config;
use ksa64_flight::phase11::{
    ksa_g10r_reference_mission_plan, GlobalKlr10FlightPackage, KsaG10rReferenceOpsV1,
};
use ksa64_interface::phase10::*;
use ksa64_interface::phase11::*;

const BOX: *mut u8 = 0x0200 as *mut u8;
const RESULT: *mut u8 = 0x0410 as *mut u8;
const BOX_MAGIC: u32 = 0x3142_4d4b;
const RESULT_MAGIC: u32 = 0x3146_4c4b;
const PAYLOAD_AT: usize = 16;
const PAYLOAD_LENGTH: usize = 512;

const OP_RELEASE: u8 = 1;
const OP_STAGE: u8 = 2;
const OP_COMMIT: u8 = 3;
const OP_CANCEL: u8 = 4;
const OP_GROUND_COMMS: u8 = 5;
const OP_PREDICTION: u8 = 6;
const OP_JOURNAL: u8 = 7;

// SEI; disable CIA1/CIA2/VIC interrupts; acknowledge pending sources; map
// all RAM in at $A000-$FFFF. The endpoint performs no KERNAL or I/O calls.
#[used]
#[link_section = ".init.005"]
static STOCK_BANK_INIT: [u8; 24] = [
    0x78, 0xa9, 0x7f, 0x8d, 0x0d, 0xdc, 0x8d, 0x0d, 0xdd, 0xa9, 0x00, 0x8d, 0x1a, 0xd0, 0xad, 0x0d,
    0xdc, 0xad, 0x0d, 0xdd, 0xa9, 0x34, 0x85, 0x01,
];

static mut PACKAGE: MaybeUninit<KsaG10rReferenceOpsV1> = MaybeUninit::uninit();

unsafe fn read(offset: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(offset))
}

unsafe fn write(offset: usize, value: u8) {
    core::ptr::write_volatile(BOX.add(offset), value)
}

unsafe fn read32(offset: usize) -> u32 {
    u32::from_le_bytes([
        read(offset),
        read(offset + 1),
        read(offset + 2),
        read(offset + 3),
    ])
}

unsafe fn write16(pointer: *mut u8, offset: usize, value: u16) {
    core::ptr::write_volatile(pointer.add(offset), value as u8);
    core::ptr::write_volatile(pointer.add(offset + 1), (value >> 8) as u8);
}

unsafe fn write32(pointer: *mut u8, offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        core::ptr::write_volatile(pointer.add(offset + index), bytes[index]);
        index += 1;
    }
}

unsafe fn payload(length: usize) -> &'static [u8] {
    slice::from_raw_parts(BOX.add(PAYLOAD_AT), length)
}

unsafe fn payload_mut(length: usize) -> &'static mut [u8] {
    slice::from_raw_parts_mut(BOX.add(PAYLOAD_AT), length)
}

fn stop(code: u16) -> ! {
    unsafe {
        write16(RESULT, 4, 1);
        write16(RESULT, 6, code);
        write32(RESULT, 0, RESULT_MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff)
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        write32(RESULT, 0, 0);
        let mut index = 0;
        while index < PAYLOAD_AT + PAYLOAD_LENGTH {
            write(index, 0);
            index += 1;
        }
        write32(BOX, 0, BOX_MAGIC);
    }

    let config = ksa_g10r_reference_flight_config();
    let package_pointer = core::ptr::addr_of_mut!(PACKAGE).cast::<KsaG10rReferenceOpsV1>();
    unsafe {
        package_pointer.write(KsaG10rReferenceOpsV1::new(config).unwrap_or_else(|| stop(10)));
    }
    let package = unsafe { &mut *package_pointer };
    if !package.initialize_mission_plan(ksa_g10r_reference_mission_plan()) {
        stop(11);
    }

    unsafe { write(6, 1) };
    let mut seen = 0u8;
    loop {
        if unsafe { read(10) } != 0 {
            stop(0);
        }
        let sequence = unsafe { read(4) };
        if sequence == seen {
            continue;
        }
        unsafe { write(9, 0) };
        let operation = unsafe { read(7) };
        match operation {
            OP_RELEASE => {
                let input = unsafe { payload(PAYLOAD_LENGTH) };
                let fast = parse_global_fast_sensor(&input[..GLOBAL_FAST_SENSOR_LENGTH])
                    .unwrap_or_else(|_| stop(1));
                let aid = if unsafe { read(8) } & 1 != 0 {
                    Some(
                        parse_global_aid_frame(
                            &input[GLOBAL_FAST_SENSOR_LENGTH
                                ..GLOBAL_FAST_SENSOR_LENGTH + GLOBAL_AID_FRAME_LENGTH],
                        )
                        .unwrap_or_else(|_| stop(2)),
                    )
                } else {
                    None
                };
                let transition = if unsafe { read(8) } & 2 != 0 {
                    let start = GLOBAL_FAST_SENSOR_LENGTH + GLOBAL_AID_FRAME_LENGTH;
                    Some(
                        parse_global_transition(&input[start..start + GLOBAL_TRANSITION_LENGTH])
                            .unwrap_or_else(|_| stop(3)),
                    )
                } else {
                    None
                };
                let evidence = package.process_release(Some(fast), aid, transition);
                unsafe {
                    write16(RESULT, 8, fast.measurement_epoch);
                    write32(RESULT, 12, evidence.navigation.checksum);
                    write32(RESULT, 16, evidence.flight_checksum);
                    write32(RESULT, 20, evidence.command.command_checksum);
                }
                let output = unsafe { payload_mut(PAYLOAD_LENGTH) };
                write_global_command(&evidence.command, &mut output[..GLOBAL_COMMAND_LENGTH])
                    .unwrap_or_else(|_| stop(4));
                if let Some(status) = evidence.status {
                    write_global_status(
                        &status,
                        &mut output
                            [GLOBAL_COMMAND_LENGTH..GLOBAL_COMMAND_LENGTH + GLOBAL_STATUS_LENGTH],
                    )
                    .unwrap_or_else(|_| stop(5));
                    unsafe { write(9, 1) };
                }
            }
            OP_STAGE => {
                let load =
                    parse_kul11(unsafe { payload(KUL11_LENGTH) }).unwrap_or_else(|_| stop(20));
                let receipt = package
                    .stage_uplink(load, unsafe { read32(12) })
                    .unwrap_or_else(|| stop(21));
                write_kua11(&receipt, unsafe { payload_mut(KUA11_LENGTH) })
                    .unwrap_or_else(|_| stop(22));
                unsafe { write(9, 1) };
            }
            OP_COMMIT | OP_CANCEL => {
                let request =
                    parse_kua11(unsafe { payload(KUA11_LENGTH) }).unwrap_or_else(|_| stop(23));
                let receipt = if operation == OP_COMMIT {
                    package.commit_uplink(&request)
                } else {
                    package.cancel_uplink(&request)
                }
                .unwrap_or_else(|| stop(24));
                write_kua11(&receipt, unsafe { payload_mut(KUA11_LENGTH) })
                    .unwrap_or_else(|_| stop(25));
                unsafe { write(9, 1) };
            }
            OP_GROUND_COMMS => {
                package.record_ground_communications(unsafe { read(8) } != 0);
            }
            OP_PREDICTION => {
                let prediction = package.prediction_summary().unwrap_or_else(|| stop(26));
                write_kpd11(&prediction, unsafe { payload_mut(KPD11_LENGTH) })
                    .unwrap_or_else(|_| stop(27));
                unsafe { write(9, 1) };
            }
            OP_JOURNAL => {
                let mut record = [EventJournalRecord::EMPTY; 1];
                let count = package.recover_journal_after(unsafe { read32(12) }, &mut record);
                if count != 0 {
                    write_kej11(&record[0], unsafe { payload_mut(KEJ11_LENGTH) })
                        .unwrap_or_else(|_| stop(28));
                    unsafe { write(9, 1) };
                }
            }
            _ => stop(29),
        }
        seen = sequence;
        unsafe { write(5, seen) };
    }
}
