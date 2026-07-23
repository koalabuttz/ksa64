#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_core::phase2_c64_replay::{
    render_phase2_replay, replay_phase2_tape, Phase2C64ReplaySink, Phase2ReplayError,
};
use ksa64_core::phase2_mission::{EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_SEPARATION};
use ksa64_core::phase2_scenario::ksa2a_fixture;

const BORDER: *mut u8 = 0xd020 as *mut u8;
const SCREEN: *mut u8 = 0x0400 as *mut u8;
const REPLAY_TAPE: &[u8; 2_851] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase2/examples/ksa2a-200km.krp2"
));

unsafe fn error_screen(code: u8) {
    let text = b"REPLAY ERROR";
    let mut index = 0usize;
    while index < text.len() {
        let byte = text[index];
        let screen = if byte == b' ' { byte } else { byte - b'A' + 1 };
        core::ptr::write_volatile(SCREEN.add(24 * 40 + index), screen);
        index += 1;
    }
    core::ptr::write_volatile(SCREEN.add(24 * 40 + 12), b'0' + code / 10);
    core::ptr::write_volatile(SCREEN.add(24 * 40 + 13), b'0' + code % 10);
    core::ptr::write_volatile(BORDER, 2);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { error_screen(99) };
    loop {}
}

fn fail(code: u8) -> ! {
    unsafe { error_screen(code) };
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let scenario = ksa2a_fixture(false);
    let mut sink = Phase2C64ReplaySink::new();
    if let Err(error) = replay_phase2_tape(scenario, REPLAY_TAPE, &mut sink) {
        fail(match error {
            Phase2ReplayError::Length => 11,
            Phase2ReplayError::Magic => 12,
            Phase2ReplayError::Version => 13,
            Phase2ReplayError::Header => 14,
            Phase2ReplayError::Checksum => 15,
            Phase2ReplayError::Scenario => 16,
            Phase2ReplayError::Telemetry(_) => 17,
        });
    }
    if sink.latest_frame().map(|frame| frame.state_checksum()) != Ok(0xcc57_612b) {
        fail(22);
    }
    if sink.frames_replayed() != 901 {
        fail(23);
    }
    if sink.source_stream_crc32() != 0x7d13_b2bf {
        fail(27);
    }
    let required_events = EVENT_IGNITION | EVENT_CUTOFF | EVENT_SEPARATION | EVENT_END;
    if sink.observed_events() & required_events != required_events {
        fail(24);
    }
    if sink.cue_counts() != [1, 2, 1, 1, 0] {
        fail(25);
    }
    if render_phase2_replay(scenario, &sink).is_err() {
        fail(26);
    }
    unsafe { core::ptr::write_volatile(BORDER, 5) };
    loop {}
}
