#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase5_gnc::{AttitudeControllerGains, SpatialFlightComputer};
use ksa64_flight::phase5_guidance::reference_guidance_target;
use ksa64_interface::crc32_ieee;
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, write_spatial_actuator_command, write_spatial_sensor_frame,
    SPATIAL_ACTUATOR_COMMAND_LENGTH, SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_sim::phase5_sensors::{Phase5SensorFaults, Phase5SensorSuite};
use ksa64_sim::phase5_vehicle::Phase5VehicleMachine;
const MAGIC: u32 = 0x3550_544b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn u16o(o: usize, v: u16) {
    core::ptr::write_volatile(RESULT.add(o), v as u8);
    core::ptr::write_volatile(RESULT.add(o + 1), (v >> 8) as u8)
}
unsafe fn u32o(o: usize, v: u32) {
    for n in 0..4 {
        core::ptr::write_volatile(RESULT.add(o + n), (v >> (n * 8)) as u8)
    }
}
fn fail(code: u16) -> ! {
    unsafe {
        u16o(4, 1);
        u16o(6, 2);
        u16o(8, code);
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
    unsafe {
        u32o(0, 0);
        c64_timer::prepare_cia_timing()
    };
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let snapshot = Phase5VehicleMachine::new_ksa5a()
        .and_then(|m| m.current_snapshot())
        .unwrap_or_else(|_| fail(1));
    let mut sensors = Phase5SensorSuite::new(0x5a00_0000, Phase5SensorFaults::default());
    let mut flight = SpatialFlightComputer::new();
    let mut sb = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
    let mut cb = [0u8; SPATIAL_ACTUATOR_COMMAND_LENGTH];
    unsafe { c64_timer::start_cia_timer() };
    let sensor = sensors.sample(snapshot);
    if write_spatial_sensor_frame(&sensor, &mut sb).is_err() {
        fail(2)
    }
    let output = flight.step_serialized_with_gains(
        &sb,
        reference_guidance_target(0),
        AttitudeControllerGains::REFERENCE_STAGE1,
    );
    if write_spatial_actuator_command(&output.command, &mut cb).is_err()
        || parse_spatial_actuator_command(&cb).is_err()
    {
        fail(3)
    }
    let cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    unsafe {
        u16o(4, 1);
        u16o(6, 2);
        u16o(8, 0);
        u16o(10, 0);
        u32o(12, overhead);
        u32o(16, cycles);
        u32o(20, sensors.checksum());
        u32o(24, output.navigation.checksum);
        u32o(28, output.flight_checksum);
        u32o(32, crc32_ieee(&cb[..SPATIAL_ACTUATOR_COMMAND_LENGTH - 4]));
        core::ptr::write_volatile(BORDER, 5);
        u32o(0, MAGIC)
    }
    loop {}
}
