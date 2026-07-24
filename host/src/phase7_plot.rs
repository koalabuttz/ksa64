//! Bounded sparse KPH7 trajectory plots for stock-C64 retention.

use ksa64_core::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordKind,
    KPH7_HEADER_LENGTH, KPH7_POINT_LENGTH, KST7_FRAME_LENGTH, KST7_HEADER_LENGTH,
};
use ksa64_core::phase7_telemetry::{parse_kst7_frame, parse_kst7_header};

pub const STOCK_KPH7_MAX_POINTS: usize = 124;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kph7Point {
    pub time_raw: i32,
    pub altitude_raw: i32,
    pub velocity_raw: i32,
    pub phase_and_events: u32,
}

fn w16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn r32(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

pub fn build_stock_kph7(telemetry: &[u8]) -> Result<Vec<u8>, &'static str> {
    if telemetry.len() < KST7_HEADER_LENGTH
        || !(telemetry.len() - KST7_HEADER_LENGTH).is_multiple_of(KST7_FRAME_LENGTH)
    {
        return Err("framing");
    }
    let header = parse_kst7_header(&telemetry[..KST7_HEADER_LENGTH]).map_err(|_| "header")?;
    let frame_count = (telemetry.len() - KST7_HEADER_LENGTH) / KST7_FRAME_LENGTH;
    if frame_count == 0 {
        return Err("empty");
    }
    let point_count = frame_count.min(STOCK_KPH7_MAX_POINTS);
    let length = KPH7_HEADER_LENGTH + point_count * KPH7_POINT_LENGTH + 4;
    let mut output = vec![0u8; length];
    write_phase7_header(
        &mut output,
        Phase7RecordKind::PlotHeader,
        header.stream_identity,
    )
    .map_err(|_| "record")?;
    w16(&mut output, 32, point_count as u16);
    w16(&mut output, 34, KPH7_POINT_LENGTH as u16);
    wu32(&mut output, 36, header.vehicle_identity);
    wu32(&mut output, 40, header.motor_identity);
    wu32(&mut output, 44, header.mission_identity);
    wu32(&mut output, 48, frame_count as u32);
    for point_index in 0..point_count {
        let frame_index = if point_count == 1 {
            0
        } else {
            point_index * (frame_count - 1) / (point_count - 1)
        };
        let frame_offset = KST7_HEADER_LENGTH + frame_index * KST7_FRAME_LENGTH;
        let frame = parse_kst7_frame(&telemetry[frame_offset..frame_offset + KST7_FRAME_LENGTH])
            .map_err(|_| "frame")?;
        let point_offset = KPH7_HEADER_LENGTH + point_index * KPH7_POINT_LENGTH;
        w32(
            &mut output,
            point_offset,
            frame.observation.state.time.raw(),
        );
        w32(
            &mut output,
            point_offset + 4,
            frame.observation.state.altitude.raw(),
        );
        w32(
            &mut output,
            point_offset + 8,
            frame.observation.state.velocity.raw(),
        );
        wu32(
            &mut output,
            point_offset + 12,
            frame.observation.events | ((frame.observation.state.phase as u32) << 24),
        );
    }
    seal_phase7_record(&mut output).map_err(|_| "checksum")?;
    Ok(output)
}

pub fn parse_kph7(input: &[u8]) -> Result<Vec<Kph7Point>, &'static str> {
    validate_phase7_record(input, Phase7RecordKind::PlotHeader).map_err(|_| "record")?;
    if input.len() < KPH7_HEADER_LENGTH + 4
        || input[52..KPH7_HEADER_LENGTH]
            .iter()
            .any(|value| *value != 0)
    {
        return Err("reserved");
    }
    let count = u16::from_le_bytes([input[32], input[33]]) as usize;
    let stride = u16::from_le_bytes([input[34], input[35]]) as usize;
    if stride != KPH7_POINT_LENGTH
        || input.len() != KPH7_HEADER_LENGTH + count * KPH7_POINT_LENGTH + 4
    {
        return Err("length");
    }
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = KPH7_HEADER_LENGTH + index * KPH7_POINT_LENGTH;
        points.push(Kph7Point {
            time_raw: r32(input, offset),
            altitude_raw: r32(input, offset + 4),
            velocity_raw: r32(input, offset + 8),
            phase_and_events: ru32(input, offset + 12),
        });
    }
    Ok(points)
}
