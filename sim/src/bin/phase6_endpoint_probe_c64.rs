#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase6_realtime::{
    reference_realtime_guidance_slice, RealtimeFlightComputer, REALTIME_GUIDANCE_SIGNATURE,
};
use ksa64_interface::phase6::*;
use ksa64_interface::phase6_transport::{
    ByteTransmitter, ByteTransport, RealtimeCellKind, RealtimeCellReceiver, TransportError,
};
const MAGIC: u32 = 0x3645_504b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
struct Feed {
    bytes: [u8; REALTIME_AID_LENGTH],
    length: u8,
    offset: u8,
}
impl Feed {
    const fn new() -> Self {
        Self {
            bytes: [0; REALTIME_AID_LENGTH],
            length: 0,
            offset: 0,
        }
    }
    fn load(&mut self, input: &[u8]) {
        self.bytes[..input.len()].copy_from_slice(input);
        self.length = input.len() as u8;
        self.offset = 0
    }
}
impl ByteTransport for Feed {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError> {
        if self.offset == self.length {
            return Ok(None);
        }
        let b = self.bytes[self.offset as usize];
        self.offset += 1;
        Ok(Some(b))
    }
    fn try_write(&mut self, _: u8) -> Result<bool, TransportError> {
        Ok(false)
    }
    fn is_connected(&self) -> bool {
        true
    }
}
struct Capture {
    bytes: [u8; REALTIME_STATUS_LENGTH],
    length: u8,
}
impl Capture {
    const fn new() -> Self {
        Self {
            bytes: [0; REALTIME_STATUS_LENGTH],
            length: 0,
        }
    }
}
impl ByteTransport for Capture {
    fn try_read(&mut self) -> Result<Option<u8>, TransportError> {
        Ok(None)
    }
    fn try_write(&mut self, b: u8) -> Result<bool, TransportError> {
        if self.length as usize == self.bytes.len() {
            return Ok(false);
        }
        self.bytes[self.length as usize] = b;
        self.length += 1;
        Ok(true)
    }
    fn is_connected(&self) -> bool {
        true
    }
}

unsafe fn u16o(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn u32o(o: usize, v: u32) {
    for n in 0..4 {
        core::ptr::write_volatile(RESULT.add(o + n), (v >> (n * 8)) as u8)
    }
}
fn fail(c: u16) -> ! {
    unsafe {
        u16o(4, 1);
        u16o(6, c);
        core::ptr::write_volatile(BORDER, 2);
        u32o(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    fail(0xffff)
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { u32o(0, 0) }
    let aid = RealtimeAidCell {
        session: 0x6a52,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: REALTIME_AID_GPS | REALTIME_AID_STAR,
        events: 0,
        onboard_time_q16: 0,
        barometer_q12: 0,
        gps_position_q12: [22_958_965, 0, 12_465_701],
        gps_velocity_q24: [0, 6_857_499, 0],
        star_angle: [0, -8066, 0],
        rcs_propellant_q12: 0,
        vehicle_status: 1,
    };
    let inertial = RealtimeInertialCell {
        session: 0x6a52,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: 0xff,
        flags: 0,
        platform_angle: [0, -8066, 0],
        angular_rate: [0; 3],
        delta_velocity: [1, 2, 3],
        gimbal_applied: [0; 2],
        stage_status: 1,
    };
    let mut ab = [0u8; REALTIME_AID_LENGTH];
    let mut ib = [0u8; REALTIME_INERTIAL_LENGTH];
    if write_realtime_aid(&aid, &mut ab).is_err()
        || write_realtime_inertial(&inertial, &mut ib).is_err()
    {
        fail(1)
    }
    let mut wire = Feed::new();
    let mut rx = RealtimeCellReceiver::new();
    wire.load(&ab);
    let pending = match rx.poll(&mut wire) {
        Ok(Some((RealtimeCellKind::Aid, b))) => parse_realtime_aid(b).ok(),
        _ => fail(4),
    };
    wire.load(&ib);
    let got = match rx.poll(&mut wire) {
        Ok(Some((RealtimeCellKind::Inertial, b))) => parse_realtime_inertial(b).ok(),
        _ => fail(5),
    };
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let s = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(s.start, s.end, s.rate);
    let out = flight.tick(got, pending);
    let mut cb = [0u8; REALTIME_COMMAND_LENGTH];
    if write_realtime_command(&out.command, &mut cb).is_err() {
        fail(7)
    }
    let mut tx = ByteTransmitter::<REALTIME_STATUS_LENGTH>::new();
    let mut capture = Capture::new();
    if tx.stage(&cb).is_err() || tx.poll(&mut capture).is_err() {
        fail(8)
    }
    let mut count = 0u16;
    let mut hash = 2_166_136_261u32;
    let mut at = 0;
    while at < capture.length {
        let b = capture.bytes[at as usize];
        count += 1;
        hash = hash.rotate_left(5) ^ b as u32;
        hash = hash.wrapping_add(0x9e37_79b9);
        at += 1
    }
    unsafe {
        u16o(4, 1);
        u16o(6, 0);
        u16o(8, count);
        u16o(10, 0);
        u32o(12, hash);
        u32o(16, flight.navigation().checksum);
        u32o(20, flight.flight_checksum());
        u32o(24, REALTIME_GUIDANCE_SIGNATURE);
        core::ptr::write_volatile(BORDER, 5);
        u32o(0, MAGIC)
    }
    loop {}
}
