#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase6_realtime::{reference_realtime_guidance_slice, RealtimeFlightComputer};
use ksa64_interface::phase6::*;
use ksa64_interface::phase6_transport::{
    AciaTransport, ByteTransmitter, RealtimeCellKind, RealtimeCellReceiver,
};
use ksa64_sim::phase6_c64::{C64AciaRegisters, ACIA_BASE_IO1};
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const MAGIC: u32 = 0x3646_4c4b;
const READY: u32 = 0x3652_4459;
unsafe fn put16(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn put32(o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut n = 0;
    while n < 4 {
        core::ptr::write_volatile(RESULT.add(o + n), b[n]);
        n += 1
    }
}
fn stop(code: u16, epochs: u16, nav: u32, flight: u32) -> ! {
    unsafe {
        put16(4, 1);
        put16(6, code);
        put16(8, epochs);
        put16(10, 0);
        put32(12, nav);
        put32(16, flight);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        put32(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff, 0, 0, 0)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { put32(0, 0) };
    let mut regs = C64AciaRegisters::new(ACIA_BASE_IO1).unwrap();
    unsafe { regs.configure_turbo232_57k6() };
    let mut io = AciaTransport::new(regs);
    unsafe { put32(0, READY) };
    let mut ready_tx = ByteTransmitter::<4>::new();
    if ready_tx.stage(&KLR6_READY).is_err() {
        stop(9, 0, 0, 0)
    }
    loop {
        match ready_tx.poll(&mut io) {
            Ok(true) => break,
            Ok(false) => {}
            Err(_) => stop(10, 0, 0, 0),
        }
    }
    let mut rx = RealtimeCellReceiver::new();
    let mut tx = ByteTransmitter::<REALTIME_AID_LENGTH>::new();
    let mut pending_aid = None;
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let s = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(s.start, s.end, s.rate);
    loop {
        let cell = match rx.poll(&mut io) {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) => stop(1, 0, flight.navigation().checksum, flight.flight_checksum()),
        };
        match cell.0 {
            RealtimeCellKind::Aid => {
                pending_aid = parse_realtime_aid(cell.1).ok();
            }
            RealtimeCellKind::Inertial => {
                let inertial = match parse_realtime_inertial(cell.1) {
                    Ok(v) => v,
                    Err(_) => stop(2, 0, flight.navigation().checksum, flight.flight_checksum()),
                };
                let epoch = inertial.measurement_epoch;
                if epoch & 31 == 2 {
                    let g = reference_realtime_guidance_slice(epoch >> 5);
                    flight.set_guidance_segment(g.start, g.end, g.rate)
                }
                let out = flight.tick(Some(inertial), pending_aid.take());
                let mut b = [0u8; REALTIME_AID_LENGTH];
                if write_realtime_command(&out.command, &mut b[..REALTIME_COMMAND_LENGTH]).is_err()
                {
                    stop(
                        3,
                        epoch,
                        flight.navigation().checksum,
                        flight.flight_checksum(),
                    )
                }
                if tx.stage(&b[..REALTIME_COMMAND_LENGTH]).is_err() {
                    stop(
                        4,
                        epoch,
                        flight.navigation().checksum,
                        flight.flight_checksum(),
                    )
                }
                loop {
                    match tx.poll(&mut io) {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(_) => stop(
                            5,
                            epoch,
                            flight.navigation().checksum,
                            flight.flight_checksum(),
                        ),
                    }
                }
                if let Some(status) = out.status {
                    if write_realtime_status(&status, &mut b[..REALTIME_STATUS_LENGTH]).is_err() {
                        stop(
                            6,
                            epoch,
                            flight.navigation().checksum,
                            flight.flight_checksum(),
                        )
                    }
                    if tx.stage(&b[..REALTIME_STATUS_LENGTH]).is_err() {
                        stop(
                            7,
                            epoch,
                            flight.navigation().checksum,
                            flight.flight_checksum(),
                        )
                    }
                    loop {
                        match tx.poll(&mut io) {
                            Ok(true) => break,
                            Ok(false) => {}
                            Err(_) => stop(
                                8,
                                epoch,
                                flight.navigation().checksum,
                                flight.flight_checksum(),
                            ),
                        }
                    }
                }
                if inertial.flags & 1 != 0 {
                    stop(
                        0,
                        epoch.wrapping_add(1),
                        flight.navigation().checksum,
                        flight.flight_checksum(),
                    )
                }
            }
            _ => {}
        }
    }
}
