#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_flight::phase8_5::{LocalControlCapability, LocalFlightConfig};
use ksa64_flight::phase9_5::{AdvancedFlightComputer, AdvancedFlightConfig, AirDataSource};
use ksa64_interface::phase9_5::*;
const MAGIC: u32 = 0x3946_4441;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
unsafe fn put(at: usize, value: u32) {
    for (index, byte) in value.to_le_bytes().iter().copied().enumerate() {
        core::ptr::write_volatile(RESULT.add(at + index), byte)
    }
}
fn finish(failures: u32, signature: u32) -> ! {
    unsafe {
        put(4, failures);
        put(8, signature);
        core::ptr::write_volatile(BORDER, if failures == 0 { 5 } else { 2 });
        put(0, MAGIC)
    }
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    finish(u32::MAX, 0)
}
fn config() -> AdvancedFlightConfig {
    AdvancedFlightConfig {
        local: LocalFlightConfig {
            session: 0x9510,
            capability: LocalControlCapability::MonitorOnly,
            minimum_arming_time_q18: 1 << 18,
            minimum_arming_altitude_q13: 10 << 13,
            burnout_qualification_time_q18: 3 << 18,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            proportional_gain_q15: 8192,
            derivative_gain_q15: 4096,
            gimbal_limit_q15: 0,
        },
        roll_proportional_gain_q15: 8192,
        roll_derivative_gain_q15: 4096,
        torque_limit_q12: [4096; 3],
        fallback_density_upper_q10: 1255,
        maximum_wind_q19: 10 << 19,
        minimum_sound_speed_mps: 300,
        maximum_navigation_speed_mps: 400,
        propellant_wet_q21: 209715,
        reserve_q15: 6554,
    }
}
fn fast(epoch: u16) -> AdvancedFastSensorCell {
    AdvancedFastSensorCell {
        session: 0x9510,
        measurement_epoch: epoch,
        production_epoch: epoch,
        validity: ADVANCED_VALID_PLATFORM
            | ADVANCED_VALID_RATE
            | ADVANCED_VALID_DELTA_V
            | ADVANCED_VALID_ACTUATOR
            | ADVANCED_VALID_AIR_DATA
            | ADVANCED_VALID_SUPPLY,
        platform_angle: [100, -100, 50],
        angular_rate: [5, 0, 0],
        delta_velocity: [0, 0, 2500],
        dynamic_pressure_q10: 1200 << 10,
        mach_q12: 1024,
        gimbal_applied: [0; 2],
        canard_applied: [0; 4],
        valve_open_mask: 0,
        propellant_q21: 209715,
        supply_scale_q15: 32768,
        vehicle_status: 2,
        actuator_feedback: 0,
        flags: 0,
    }
}
#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe { put(0, 0) }
    let mut failures = 0u32;
    let mut flight = match AdvancedFlightComputer::new(config(), [0; 3], [0; 3]) {
        Some(v) => v,
        None => finish(1, 0),
    };
    let first = flight.tick(Some(fast(0)), None);
    failures |= u32::from(first.air_data.source != AirDataSource::Pitot);
    failures |= u32::from(first.command.torque_demand_q12 == [0; 3]) << 1;
    let one = flight.tick(None, None);
    let two = flight.tick(None, None);
    let three = flight.tick(None, None);
    failures |= u32::from(one.command.torque_demand_q12 != first.command.torque_demand_q12) << 2;
    failures |= u32::from(two.command.torque_demand_q12 != first.command.torque_demand_q12) << 3;
    failures |= u32::from(three.command.torque_demand_q12 != [0; 3]) << 4;
    failures |= u32::from(three.command.discrete != ADVANCED_COMMAND_SAFE) << 5;
    finish(failures, three.command_checksum)
}
