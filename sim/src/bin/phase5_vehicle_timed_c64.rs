#![no_std]
#![no_main]
use core::panic::PanicInfo;
use ksa64_core::c64_timer;
use ksa64_interface::EngineAction;
use ksa64_sim::phase5_vehicle::{Phase5VehicleCommand, Phase5VehicleMachine};
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
unsafe fn i32o(o: usize, v: i32) {
    u32o(o, v as u32)
}
fn fail(code: u16) -> ! {
    unsafe {
        u16o(4, 1);
        u16o(6, 1);
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
    let mut machine = Phase5VehicleMachine::new_ksa5a().unwrap_or_else(|_| fail(1));
    unsafe { c64_timer::start_cia_timer() };
    let snapshot = machine.step(Phase5VehicleCommand {
        engine_action: EngineAction::Ignite,
        ..Phase5VehicleCommand::HOLD
    });
    let cycles = unsafe { c64_timer::stop_cia_timer() }.wrapping_sub(overhead);
    let snapshot = snapshot.unwrap_or_else(|_| fail(2));
    let p = snapshot.truth.spatial().position();
    unsafe {
        u16o(4, 1);
        u16o(6, 1);
        u16o(8, 0);
        u16o(10, 0);
        u32o(12, overhead);
        u32o(16, cycles);
        u32o(20, snapshot.truth.step());
        i32o(24, p.x());
        i32o(28, p.y());
        i32o(32, p.z());
        core::ptr::write_volatile(BORDER, 5);
        u32o(0, MAGIC)
    }
    loop {}
}
