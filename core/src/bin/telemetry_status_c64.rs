#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::c64_status::{render_status, C64StatusSink};
use ksa64_core::scenario::{parse_scenario_image, SCENARIO_IMAGE_LENGTH};
use ksa64_core::telemetry::run_vertical_mission_with_telemetry;

const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;
const SCENARIO_IMAGE: &[u8; SCENARIO_IMAGE_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase0/numeric/scenario-v1.bin"
));

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        core::ptr::write_volatile(BORDER_COLOR, 2);
    }
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let scenario = match parse_scenario_image(SCENARIO_IMAGE) {
        Ok(scenario) => scenario,
        Err(_) => {
            unsafe {
                core::ptr::write_volatile(BORDER_COLOR, 2);
            }
            loop {}
        }
    };
    let mut sink = C64StatusSink::new();
    if run_vertical_mission_with_telemetry(&scenario, &mut sink).is_err()
        || render_status(&scenario, &sink).is_err()
    {
        unsafe {
            core::ptr::write_volatile(BORDER_COLOR, 2);
        }
        loop {}
    }

    loop {}
}
