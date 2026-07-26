#![no_std]
#![no_main]

use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_flight::phase10::{ksa_g10r_reference_flight_config, GlobalFlightComputer};
use ksa64_interface::phase10::*;

const MAGIC: u32 = 0x3054_4c4b;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const PAL_RELEASE_CYCLES: u32 = 30_789;
const SESSION: u16 = 0x10a0;
const Q30_ONE: i32 = 1 << 30;

unsafe fn p16(offset: usize, value: u16) {
    core::ptr::write_volatile(RESULT.add(offset), value as u8);
    core::ptr::write_volatile(RESULT.add(offset + 1), (value >> 8) as u8);
}

unsafe fn p32(offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        core::ptr::write_volatile(RESULT.add(offset + index), bytes[index]);
        index += 1;
    }
}

fn stop(code: u16) -> ! {
    unsafe {
        p16(4, 1);
        p16(6, code);
        core::ptr::write_volatile(BORDER, 2);
        p32(0, MAGIC);
    }
    loop {}
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop(0xffff)
}

fn fast(epoch: u16, frame: GlobalFrameId) -> GlobalFastSensorCell {
    GlobalFastSensorCell {
        session: SESSION,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame,
        validity: GLOBAL_FAST_VALID_MASK,
        mission_time_q16: u32::from(epoch) * 2_048,
        delta_velocity_q24: [0, 0, 16],
        delta_angle_q24: [0; 3],
        attitude_vector_q15: [0; 3],
        angular_rate_q15: [0; 3],
        dynamic_pressure_q10: 5_000 << 10,
        mach_q12: 2_048,
        gimbal_applied_q15: [0; 2],
        rcs_propellant_q21: 5 << 21,
        actuator_feedback: 0,
        vehicle_status: 2,
        sensor_checksum: epoch,
    }
}

fn aid(epoch: u16, frame: GlobalFrameId, gnss: bool) -> GlobalAidFrameCell {
    GlobalAidFrameCell {
        session: SESSION,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame,
        validity: GLOBAL_AID_BAROMETER
            | GLOBAL_AID_ATTITUDE
            | GLOBAL_AID_CONTINUITY
            | if gnss { GLOBAL_AID_GNSS } else { 0 },
        mission_time_q16: u32::from(epoch) * 2_048,
        barometer_q12_km: 0,
        gnss_position_q12_km: [0; 3],
        gnss_velocity_q24_km_s: [0; 3],
        attitude_q30: [Q30_ONE, 0, 0, 0],
        frame_rotation_q30: [Q30_ONE, 0, 0, 0],
        frame_omega_q24: [0; 3],
        events: 0,
        continuity: 1,
        deployment_feedback: 0,
    }
}

fn transition(epoch: u16) -> GlobalTransitionCell {
    GlobalTransitionCell {
        session: SESSION,
        source_epoch: epoch,
        effective_epoch: epoch,
        from: GlobalFrameId::LocalEnuV1,
        to: GlobalFrameId::EarthFixedEcefV1,
        flags: 0,
        mission_time_q16: u32::from(epoch) * 2_048,
        transform_identity: 0x10f0_0001,
        rotation_q30: [Q30_ONE, 0, 0, 0],
        omega_q24: [0; 3],
        pre_position_q12: [0; 3],
        post_position_q12: [1, 2, 3],
        pre_velocity_q24: [0; 3],
        post_velocity_q24: [0; 3],
        pre_attitude_q30: [Q30_ONE, 0, 0, 0],
        post_attitude_q30: [Q30_ONE, 0, 0, 0],
        pre_rate_q24: [0; 3],
        post_rate_q24: [0; 3],
        translation_q12: [1, 2, 3],
        velocity_bias_q24: [0; 3],
        transition_checksum: 0x10f0_0001,
    }
}

fn measure(
    computer: &mut GlobalFlightComputer,
    overhead: u32,
    fast_cell: GlobalFastSensorCell,
    aid_cell: Option<GlobalAidFrameCell>,
    transition_cell: Option<GlobalTransitionCell>,
) -> (u32, u32) {
    unsafe { c64_timer::start_cia_timer() };
    let evidence = computer.tick(Some(fast_cell), aid_cell, transition_cell);
    let cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    (cycles, evidence.flight_checksum)
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    unsafe {
        p32(0, 0);
        c64_timer::prepare_cia_timing();
    }
    let overhead = unsafe { c64_timer::measure_cia_boundary_overhead() };
    let mut flight =
        GlobalFlightComputer::new(ksa_g10r_reference_flight_config()).unwrap_or_else(|| stop(1));

    let (gnss, _) = measure(
        &mut flight,
        overhead,
        fast(0, GlobalFrameId::LocalEnuV1),
        Some(aid(0, GlobalFrameId::LocalEnuV1, true)),
        None,
    );
    let (fast_cycles, _) = measure(
        &mut flight,
        overhead,
        fast(1, GlobalFrameId::LocalEnuV1),
        None,
        None,
    );
    let _ = flight.tick(Some(fast(2, GlobalFrameId::LocalEnuV1)), None, None);
    let _ = flight.tick(Some(fast(3, GlobalFrameId::LocalEnuV1)), None, None);
    let (aided, _) = measure(
        &mut flight,
        overhead,
        fast(4, GlobalFrameId::LocalEnuV1),
        Some(aid(4, GlobalFrameId::LocalEnuV1, false)),
        None,
    );
    let (transition_cycles, checksum) = measure(
        &mut flight,
        overhead,
        fast(5, GlobalFrameId::EarthFixedEcefV1),
        None,
        Some(transition(5)),
    );
    let worst = gnss.max(fast_cycles).max(aided).max(transition_cycles);
    let status = u16::from(flight.is_safe());

    unsafe {
        p16(4, 1);
        p16(6, status);
        p32(8, overhead);
        p32(12, fast_cycles);
        p32(16, aided);
        p32(20, gnss);
        p32(24, transition_cycles);
        p32(28, worst);
        p32(32, PAL_RELEASE_CYCLES);
        p32(36, checksum);
        core::ptr::write_volatile(BORDER, if status == 0 { 5 } else { 2 });
        p32(0, MAGIC);
    }
    loop {}
}
