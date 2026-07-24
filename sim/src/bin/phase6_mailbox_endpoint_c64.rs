#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase6_realtime::{reference_realtime_guidance_slice, RealtimeFlightComputer};
use ksa64_interface::phase6::*;
const BOX: *mut u8 = 0xc800 as *mut u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3642_4d4b;
const RESULT_MAGIC: u32 = 0x3646_4c4b;
unsafe fn r(o: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(o))
}
unsafe fn w(o: usize, v: u8) {
    core::ptr::write_volatile(BOX.add(o), v)
}
unsafe fn w16(p: *mut u8, o: usize, v: u16) {
    core::ptr::write_volatile(p.add(o), v as u8);
    core::ptr::write_volatile(p.add(o + 1), (v >> 8) as u8)
}
unsafe fn w32(p: *mut u8, o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut n = 0;
    while n < 4 {
        core::ptr::write_volatile(p.add(o + n), b[n]);
        n += 1
    }
}
fn stop(code: u16, epochs: u16, nav: u32, flight: u32) -> ! {
    unsafe {
        w16(RESULT, 4, 1);
        w16(RESULT, 6, code);
        w16(RESULT, 8, epochs);
        w16(RESULT, 10, 0);
        w32(RESULT, 12, nav);
        w32(RESULT, 16, flight);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        w32(RESULT, 0, RESULT_MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff, 0, 0, 0)
}
fn copy_in<const N: usize>(at: usize) -> [u8; N] {
    let mut b = [0; N];
    let mut n = 0;
    while n < N {
        b[n] = unsafe { r(at + n) };
        n += 1
    }
    b
}
fn copy_out(at: usize, b: &[u8]) {
    let mut n = 0;
    while n < b.len() {
        unsafe { w(at + n, b[n]) };
        n += 1
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        w32(RESULT, 0, 0);
        let mut n = 0;
        while n < 128 {
            w(n, 0);
            w(n + 128, 0);
            n += 1
        }
        w32(BOX, 0, BOX_MAGIC)
    }
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let g = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(g.start, g.end, g.rate);
    let mut seen = 0u8;
    loop {
        let seq = unsafe { r(4) };
        if seq == seen {
            continue;
        }
        let aid = if unsafe { r(8) } != 0 {
            let b = copy_in::<REALTIME_AID_LENGTH>(16);
            match parse_realtime_aid(&b) {
                Ok(v) => Some(v),
                Err(_) => stop(1, 0, flight.navigation().checksum, flight.flight_checksum()),
            }
        } else {
            None
        };
        let ib = copy_in::<REALTIME_INERTIAL_LENGTH>(80);
        let inertial = match parse_realtime_inertial(&ib) {
            Ok(v) => v,
            Err(_) => stop(2, 0, flight.navigation().checksum, flight.flight_checksum()),
        };
        let epoch = inertial.measurement_epoch;
        if epoch & 31 == 2 {
            let s = reference_realtime_guidance_slice(epoch >> 5);
            flight.set_guidance_segment(s.start, s.end, s.rate)
        }
        let out = flight.tick(Some(inertial), aid);
        let mut cb = [0; REALTIME_COMMAND_LENGTH];
        if write_realtime_command(&out.command, &mut cb).is_err() {
            stop(
                3,
                epoch,
                flight.navigation().checksum,
                flight.flight_checksum(),
            )
        }
        copy_out(128, &cb);
        unsafe { w(9, 0) }
        if let Some(status) = out.status {
            let mut sb = [0; REALTIME_STATUS_LENGTH];
            if write_realtime_status(&status, &mut sb).is_err() {
                stop(
                    4,
                    epoch,
                    flight.navigation().checksum,
                    flight.flight_checksum(),
                )
            }
            copy_out(152, &sb);
            unsafe { w(9, 1) }
        }
        seen = seq;
        unsafe {
            w(5, seen);
            w(6, seen)
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
}
