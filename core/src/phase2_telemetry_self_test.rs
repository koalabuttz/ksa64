//! Target-executable KST2 serializer/decoder fixtures.

use crate::phase2_scenario::ksa2a_fixture;
use crate::phase2_telemetry::{
    parse_phase2_telemetry_frame, parse_phase2_telemetry_header_for_scenario,
    write_phase2_telemetry_frame, write_phase2_telemetry_header, PHASE2_TELEMETRY_FRAME_LENGTH,
    PHASE2_TELEMETRY_HEADER_LENGTH,
};

mod golden {
    include!("../../phase2/generated/telemetry_v2.rs");
}

pub fn run_phase2_telemetry_self_tests() -> u8 {
    let scenario = ksa2a_fixture(false);
    let header = match parse_phase2_telemetry_header_for_scenario(&golden::HEADER, scenario) {
        Ok(value) => value,
        Err(_) => return 1,
    };
    if header.mission_steps() != 7_200
        || golden::FRAME_COUNT != 901
        || golden::STREAM_LENGTH != 57_704
        || golden::STREAM_CRC32 != 0x7d13_b2bf
    {
        return 2;
    }
    let initial = match parse_phase2_telemetry_frame(&golden::INITIAL_FRAME) {
        Ok(value) => value,
        Err(_) => return 3,
    };
    let final_frame = match parse_phase2_telemetry_frame(&golden::FINAL_FRAME) {
        Ok(value) => value,
        Err(_) => return 4,
    };
    if initial.step() != 0
        || final_frame.step() != 7_200
        || final_frame.state_checksum() != golden::FINAL_STATE_CHECKSUM
    {
        return 5;
    }
    let mut header_output = [0u8; PHASE2_TELEMETRY_HEADER_LENGTH];
    if write_phase2_telemetry_header(scenario, &mut header_output).is_err()
        || header_output != golden::HEADER
    {
        return 6;
    }
    let mut initial_output = [0u8; PHASE2_TELEMETRY_FRAME_LENGTH];
    let mut final_output = [0u8; PHASE2_TELEMETRY_FRAME_LENGTH];
    if write_phase2_telemetry_frame(&initial, &mut initial_output).is_err()
        || write_phase2_telemetry_frame(&final_frame, &mut final_output).is_err()
        || initial_output != golden::INITIAL_FRAME
        || final_output != golden::FINAL_FRAME
    {
        return 7;
    }
    0
}
