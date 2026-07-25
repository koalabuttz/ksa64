#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_flight::phase8_5::LocalFlightComputer;
use ksa64_interface::phase8_5::LocalAidCell;
use ksa64_sim::phase8_5::{
    local_flight_config, reference_avionics_profile, reference_monitor_capability,
    LocalWorldEndpoint,
};
const MAGIC: u32 = 0x3843_4c4b;
const RESULT: *mut u8 = 0xc800 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn w16(at: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(at), value as u8);
    core::ptr::write_volatile(RESULT.add(at + 1), (value >> 8) as u8);
}
unsafe fn w32(at: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < 4 {
        core::ptr::write_volatile(RESULT.add(at + index), bytes[index]);
        index += 1;
    }
}
fn stop(code: u16, releases: u32, truth: u32, navigation: u32, flight: u32) -> ! {
    unsafe {
        w16(4, 1);
        w16(6, code);
        w32(8, releases);
        w32(12, truth);
        w32(16, navigation);
        w32(20, flight);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        w32(0, MAGIC);
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff, 0, 0, 0, 0)
}
fn missing_aid() -> LocalAidCell {
    LocalAidCell {
        session: 0,
        measurement_epoch: 0,
        production_epoch: 0,
        validity: 0,
        events: 0,
        onboard_time_q18: 0,
        barometer_q13: 0,
        gps_position_q13: [0; 3],
        gps_velocity_q19: [0; 3],
        attitude_vector: [0; 3],
        continuity: 0,
        deployment_feedback: 0,
        vehicle_status: 0,
        clock_flags: 0,
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { w32(0, 0) };
    let vehicle = &ksa64_core::phase8_fixtures::FIRESTORM_SPATIAL_VEHICLE;
    let motor = &ksa64_core::phase8_fixtures::I211W_SPATIAL_MOTOR;
    let mission = ksa64_core::phase8_fixtures::FIRESTORM_I211_SPATIAL_MISSION;
    let wind = &ksa64_core::phase8_fixtures::FIRESTORM_CALM_WIND;
    let capability = reference_monitor_capability(vehicle.identity);
    let flight_config = local_flight_config(reference_avionics_profile(false), capability, motor)
        .unwrap_or_else(|_| stop(1, 0, 0, 0, 0));
    let mut world = LocalWorldEndpoint::new(
        vehicle,
        motor,
        mission,
        wind,
        SpatialMissionVariation::NOMINAL,
        capability,
    )
    .unwrap_or_else(|_| stop(2, 0, 0, 0, 0));
    let initial = world.snapshot().state;
    let attitude = initial.attitude;
    let mut flight = LocalFlightComputer::new(
        flight_config,
        [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        [
            (attitude.x() >> 15) as i16,
            (attitude.y() >> 15) as i16,
            (attitude.z() >> 15) as i16,
        ],
    )
    .unwrap_or_else(|| stop(3, 0, 0, 0, 0));
    let absent = missing_aid();
    let mut releases = 0u32;
    while !world.is_complete() {
        let release = match world.release() {
            Ok(Some(value)) => value,
            Ok(None) if world.is_complete() => break,
            _ => stop(
                4,
                releases,
                0,
                flight.navigation().checksum,
                flight.evidence().flight_checksum,
            ),
        };
        let aid_present = release.aid.is_some();
        let aid = release.aid.as_ref().unwrap_or(&absent);
        flight.tick_in_place(&release.inertial, true, aid, aid_present);
        if world.accept_command(flight.command()).is_err() {
            stop(
                5,
                releases,
                release.director.truth_checksum,
                flight.navigation().checksum,
                flight.evidence().flight_checksum,
            )
        }
        releases = releases.wrapping_add(1);
        if releases & 31 == 0 {
            unsafe {
                core::ptr::write_volatile(BORDER, (release.director.snapshot.phase as u8) & 15)
            }
        }
    }
    let result = world.result().unwrap_or_else(|| {
        stop(
            6,
            releases,
            0,
            flight.navigation().checksum,
            flight.evidence().flight_checksum,
        )
    });
    stop(
        0,
        releases,
        result.checksum,
        flight.navigation().checksum,
        flight.evidence().flight_checksum,
    )
}
