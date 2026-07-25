#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase8_5::{LocalControlCapability, LocalFlightComputer, LocalFlightConfig};
use ksa64_interface::phase8_5::*;
const BOX: *mut u8 = 0xc800 as *mut u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3842_4d4b;
const RESULT_MAGIC: u32 = 0x3846_4c4b;
unsafe fn r(offset: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(offset))
}
unsafe fn w(offset: usize, value: u8) {
    core::ptr::write_volatile(BOX.add(offset), value)
}
unsafe fn w16(pointer: *mut u8, offset: usize, value: u16) {
    core::ptr::write_volatile(pointer.add(offset), value as u8);
    core::ptr::write_volatile(pointer.add(offset + 1), (value >> 8) as u8);
}
unsafe fn w32(pointer: *mut u8, offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < 4 {
        core::ptr::write_volatile(pointer.add(offset + index), bytes[index]);
        index += 1;
    }
}
fn stop(code: u16, epochs: u16, navigation: u32, flight: u32) -> ! {
    unsafe {
        w16(RESULT, 4, 1);
        w16(RESULT, 6, code);
        w16(RESULT, 8, epochs);
        w16(RESULT, 10, 0);
        w32(RESULT, 12, navigation);
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
    let mut bytes = [0; N];
    let mut index = 0;
    while index < N {
        bytes[index] = unsafe { r(at + index) };
        index += 1;
    }
    bytes
}
fn copy_out(at: usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        unsafe { w(at + index, bytes[index]) };
        index += 1;
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        w32(RESULT, 0, 0);
        let mut index = 0;
        while index < 128 {
            w(index, 0);
            w(index + 128, 0);
            index += 1;
        }
        w32(BOX, 0, BOX_MAGIC)
    }
    while unsafe { r(4) } == 0 && unsafe { r(10) } == 0 {}
    let gimbal = unsafe { r(11) } != 0;
    let config = LocalFlightConfig {
        session: 0x8501,
        capability: if gimbal {
            LocalControlCapability::TwoAxisGimbal
        } else {
            LocalControlCapability::MonitorOnly
        },
        minimum_arming_time_q18: 1 << 18,
        minimum_arming_altitude_q13: 10 << 13,
        burnout_qualification_time_q18: ksa64_core::phase8_fixtures::I211W_SPATIAL_MOTOR
            .burn_time
            .raw(),
        drogue_backup_time_q18: 15 << 18,
        main_backup_time_q18: 65 << 18,
        main_altitude_q13: 200 << 13,
        minimum_deployment_separation_q18: 2 << 18,
        proportional_gain_q15: if gimbal { 8_192 } else { 0 },
        derivative_gain_q15: if gimbal { 4_096 } else { 0 },
        gimbal_limit_q15: if gimbal { 910 } else { 0 },
    };
    let mut numeric = ksa64_core::numeric::NumericStatus::CLEAR;
    let axis = match ksa64_core::phase8_world::rail_axis_from_mission(
        ksa64_core::phase8_fixtures::FIRESTORM_I211_SPATIAL_MISSION,
        &mut numeric,
    ) {
        Ok(value) => value,
        Err(_) => stop(10, 0, 0, 0),
    };
    let attitude = match ksa64_core::phase8_world::attitude_from_rail_axis(axis, &mut numeric) {
        Ok(value) => value,
        Err(_) => stop(11, 0, 0, 0),
    };
    let mut flight = match LocalFlightComputer::new(
        config,
        [
            0,
            0,
            ksa64_core::phase8_fixtures::FIRESTORM_I211_SPATIAL_MISSION
                .launch_altitude
                .raw(),
        ],
        [
            (attitude.x() >> 15) as i16,
            (attitude.y() >> 15) as i16,
            (attitude.z() >> 15) as i16,
        ],
    ) {
        Some(value) => value,
        None => stop(12, 0, 0, 0),
    };
    let absent_aid = LocalAidCell {
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
    };
    let mut seen = 0u8;
    let mut epochs = 0u16;
    loop {
        if unsafe { r(10) } != 0 {
            stop(
                0,
                epochs,
                flight.navigation().checksum,
                flight.evidence().flight_checksum,
            )
        }
        let sequence = unsafe { r(4) };
        if sequence == seen {
            continue;
        }
        let aid = if unsafe { r(8) } != 0 {
            let bytes = copy_in::<LOCAL_AID_LENGTH>(16);
            match parse_local_aid(&bytes) {
                Ok(value) => Some(value),
                Err(_) => stop(1, 0, flight.navigation().checksum, 0),
            }
        } else {
            None
        };
        let bytes = copy_in::<LOCAL_INERTIAL_LENGTH>(80);
        let inertial = match parse_local_inertial(&bytes) {
            Ok(value) => value,
            Err(_) => stop(2, 0, flight.navigation().checksum, 0),
        };
        let aid_present = aid.is_some();
        let aid_ref = aid.as_ref().unwrap_or(&absent_aid);
        flight.tick_in_place(&inertial, true, aid_ref, aid_present);
        epochs = inertial.measurement_epoch;
        let command_cell = flight.command();
        let status_cell = flight.status();
        let flight_checksum = flight.evidence().flight_checksum;
        let mut command = [0; LOCAL_COMMAND_LENGTH];
        if write_local_command(&command_cell, &mut command).is_err() {
            stop(
                3,
                inertial.measurement_epoch,
                flight.navigation().checksum,
                flight_checksum,
            )
        }
        copy_out(128, &command);
        unsafe { w(9, 0) };
        if inertial.measurement_epoch & 3 == 0 {
            let status = match status_cell {
                Some(value) => value,
                None => stop(
                    5,
                    inertial.measurement_epoch,
                    flight.navigation().checksum,
                    flight_checksum,
                ),
            };
            let mut bytes = [0; LOCAL_STATUS_LENGTH];
            if write_local_status(&status, &mut bytes).is_err() {
                stop(
                    4,
                    inertial.measurement_epoch,
                    flight.navigation().checksum,
                    flight_checksum,
                )
            }
            copy_out(152, &bytes);
            unsafe { w(9, 1) }
        }
        seen = sequence;
        unsafe {
            w(5, seen);
            w(6, seen)
        }
    }
}
