#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_interface::phase9_5::*;
use ksa64_sim::phase9_5::{reference_mixed_effectors, reference_mixed_vehicle};
use ksa64_sim::phase9_5_mission::{AdvancedMissionFaults, AdvancedWorldEndpoint};
const BOX: *mut u8 = 0xc800 as *mut u8;
const RESULT: *mut u8 = 0xc000 as *mut u8;
const BORDER: *mut u8 = 0xd020 as *mut u8;
const BOX_MAGIC: u32 = 0x3957_4d42;
const RESULT_MAGIC: u32 = 0x3957_4c4b;
const FAST_AT: usize = 16;
const AID_AT: usize = FAST_AT + ADVANCED_FAST_SENSOR_LENGTH;
const COMMAND_AT: usize = AID_AT + ADVANCED_AID_LENGTH;
unsafe fn r(o: usize) -> u8 {
    core::ptr::read_volatile(BOX.add(o))
}
unsafe fn w(o: usize, v: u8) {
    core::ptr::write_volatile(BOX.add(o), v)
}
unsafe fn w8(p: *mut u8, o: usize, v: u8) {
    core::ptr::write_volatile(p.add(o), v)
}
unsafe fn w16(p: *mut u8, o: usize, v: u16) {
    w8(p, o, v as u8);
    w8(p, o + 1, (v >> 8) as u8)
}
unsafe fn w32(p: *mut u8, o: usize, v: u32) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 4 {
        w8(p, o + i, b[i]);
        i += 1
    }
}
fn input<const N: usize>(o: usize) -> [u8; N] {
    let mut b = [0; N];
    let mut i = 0;
    while i < N {
        b[i] = unsafe { r(o + i) };
        i += 1
    }
    b
}
fn output(o: usize, b: &[u8]) {
    let mut i = 0;
    while i < b.len() {
        unsafe { w(o + i, b[i]) };
        i += 1
    }
}
fn stop(code: u16, releases: u16, truth: u32, edges: u32, depletion: u16) -> ! {
    unsafe {
        w16(RESULT, 4, 1);
        w16(RESULT, 6, code);
        w16(RESULT, 8, releases);
        w16(RESULT, 10, depletion);
        w32(RESULT, 12, truth);
        w32(RESULT, 16, edges);
        core::ptr::write_volatile(BORDER, if code == 0 { 5 } else { 2 });
        w32(RESULT, 0, RESULT_MAGIC)
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
        w32(RESULT, 0, 0);
        let mut i = 0;
        while i < COMMAND_AT + ADVANCED_COMMAND_LENGTH {
            w(i, 0);
            i += 1
        }
        w32(BOX, 0, BOX_MAGIC)
    }
    let vehicle = reference_mixed_vehicle();
    let effectors = reference_mixed_effectors();
    let mut mission = ksa64_core::phase8_fixtures::FIRESTORM_I211_SPATIAL_MISSION;
    mission.vehicle_identity = vehicle.identity;
    mission.identity ^= vehicle.identity;
    let capability = ksa64_sim::phase8_5::reference_gimbal_capability(vehicle.identity);
    let mut world = AdvancedWorldEndpoint::new(
        &vehicle,
        &ksa64_core::phase8_fixtures::I211W_SPATIAL_MOTOR,
        mission,
        &ksa64_core::phase8_fixtures::FIRESTORM_CALM_WIND,
        SpatialMissionVariation::NOMINAL,
        capability,
        &effectors,
        AdvancedMissionFaults::NOMINAL,
    )
    .unwrap_or_else(|_| stop(4, 0, 0, 0, 0));
    let (mut seq, mut releases, mut truth) = (0u8, 0u16, 0u32);
    loop {
        if unsafe { r(10) } != 0 {
            stop(
                0,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            )
        }
        let release = match world.release() {
            Ok(Some(v)) => v,
            Ok(None) => stop(
                0,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            ),
            Err(_) => stop(
                5,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            ),
        };
        truth = release.director.truth_checksum;
        let mut fast = [0; ADVANCED_FAST_SENSOR_LENGTH];
        write_advanced_fast_sensor(&release.fast, &mut fast).unwrap_or_else(|_| {
            stop(
                6,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            )
        });
        output(FAST_AT, &fast);
        unsafe { w(8, 0) };
        if let Some(aid) = release.aid {
            let mut bytes = [0; ADVANCED_AID_LENGTH];
            write_advanced_aid(&aid, &mut bytes).unwrap_or_else(|_| {
                stop(
                    7,
                    releases,
                    truth,
                    world.valve_edge_count(),
                    world.depletion_count(),
                )
            });
            output(AID_AT, &bytes);
            unsafe { w(8, 1) }
        }
        seq = seq.wrapping_add(1);
        if seq == 0 {
            seq = 1
        }
        unsafe {
            w(4, seq);
            w(6, seq)
        }
        while unsafe { r(5) } != seq && unsafe { r(10) } == 0 {}
        if unsafe { r(10) } != 0 {
            stop(
                0,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            )
        }
        let command = parse_advanced_command(&input::<ADVANCED_COMMAND_LENGTH>(COMMAND_AT))
            .unwrap_or_else(|_| {
                stop(
                    8,
                    releases,
                    truth,
                    world.valve_edge_count(),
                    world.depletion_count(),
                )
            });
        world.accept_command(command).unwrap_or_else(|_| {
            stop(
                9,
                releases,
                truth,
                world.valve_edge_count(),
                world.depletion_count(),
            )
        });
        releases = releases.wrapping_add(1)
    }
}
