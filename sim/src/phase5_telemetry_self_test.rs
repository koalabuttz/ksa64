use crate::phase5_mission::Phase5MissionCase;
use crate::phase5_telemetry::{
    initial_frame, parse_phase5_telemetry_frame, parse_phase5_telemetry_header,
    write_phase5_telemetry_frame, write_phase5_telemetry_header, Phase5TelemetryHeader,
    PHASE5_AVIONICS_SIGNATURE, PHASE5_TELEMETRY_FRAME_LENGTH, PHASE5_TELEMETRY_HEADER_LENGTH,
};
use crate::phase5_vehicle::{Phase5VehicleMachine, PHASE5_VEHICLE_SIGNATURE};
use ksa64_flight::phase5_guidance::GUIDANCE_SIGNATURE;
use ksa64_interface::crc32_ieee;
pub fn phase5_telemetry_codec_signature() -> u32 {
    let header = Phase5TelemetryHeader {
        seed: 0x5a00_0000,
        case: Phase5MissionCase::Nominal,
        vehicle_signature: PHASE5_VEHICLE_SIGNATURE,
        avionics_signature: PHASE5_AVIONICS_SIGNATURE,
        guidance_signature: GUIDANCE_SIGNATURE,
    };
    let mut hb = [0u8; PHASE5_TELEMETRY_HEADER_LENGTH];
    if write_phase5_telemetry_header(header, &mut hb).is_err()
        || parse_phase5_telemetry_header(&hb) != Ok(header)
    {
        return 0;
    }
    let snapshot = match Phase5VehicleMachine::new_ksa5a().and_then(|v| v.current_snapshot()) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let mut frame = initial_frame(snapshot);
    frame.observation_checksum = 0x1357_9bdf;
    let mut fb = [0u8; PHASE5_TELEMETRY_FRAME_LENGTH];
    if write_phase5_telemetry_frame(&frame, &mut fb).is_err()
        || parse_phase5_telemetry_frame(&fb) != Ok(frame)
    {
        return 0;
    }
    let mut h = crc32_ieee(&hb);
    h ^= crc32_ieee(&fb).rotate_left(11);
    h
}
pub fn run_phase5_telemetry_self_tests() -> u8 {
    let signature = phase5_telemetry_codec_signature();
    if signature != 0 && signature == PHASE5_TELEMETRY_CODEC_SIGNATURE {
        0
    } else {
        1
    }
}
pub const PHASE5_TELEMETRY_CODEC_SIGNATURE: u32 = 0x07bc_3e16;
