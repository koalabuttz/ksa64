//! Bounded spatial KPH8 plots for stock-C64 retention.
use ksa64_core::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordKind,
    KPH8_HEADER_LENGTH, KPH8_POINT_LENGTH, KST8_FRAME_LENGTH, KST8_HEADER_LENGTH,
};
use ksa64_core::phase8_telemetry::{parse_kst8_frame, parse_kst8_header};
pub const STOCK_KPH8_MAX_POINTS: usize = 82;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kph8Point {
    pub time_raw: i32,
    pub east_raw: i32,
    pub north_raw: i32,
    pub altitude_raw: i32,
    pub vertical_velocity_raw: i32,
    pub phase_and_events: u32,
}
fn w16(o: &mut [u8], p: usize, v: u16) {
    o[p..p + 2].copy_from_slice(&v.to_le_bytes())
}
fn w32(o: &mut [u8], p: usize, v: i32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn wu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn r32(i: &[u8], p: usize) -> i32 {
    i32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
fn ru32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
pub fn build_stock_kph8(telemetry: &[u8]) -> Result<Vec<u8>, &'static str> {
    if telemetry.len() < KST8_HEADER_LENGTH
        || !(telemetry.len() - KST8_HEADER_LENGTH).is_multiple_of(KST8_FRAME_LENGTH)
    {
        return Err("framing");
    }
    let header = parse_kst8_header(&telemetry[..KST8_HEADER_LENGTH]).map_err(|_| "header")?;
    let frames = (telemetry.len() - KST8_HEADER_LENGTH) / KST8_FRAME_LENGTH;
    if frames == 0 {
        return Err("empty");
    }
    let count = frames.min(STOCK_KPH8_MAX_POINTS);
    let mut output = vec![0u8; KPH8_HEADER_LENGTH + count * KPH8_POINT_LENGTH + 4];
    write_phase8_header(
        &mut output,
        Phase8RecordKind::PlotHeader,
        header.stream_identity,
    )
    .map_err(|_| "record")?;
    w16(&mut output, 32, count as u16);
    w16(&mut output, 34, KPH8_POINT_LENGTH as u16);
    wu32(&mut output, 36, header.vehicle_identity);
    wu32(&mut output, 40, header.motor_identity);
    wu32(&mut output, 44, header.mission_identity);
    wu32(&mut output, 48, header.wind_identity);
    wu32(&mut output, 52, frames as u32);
    for point_index in 0..count {
        let frame_index = if count == 1 {
            0
        } else {
            point_index * (frames - 1) / (count - 1)
        };
        let offset = KST8_HEADER_LENGTH + frame_index * KST8_FRAME_LENGTH;
        let (frame, _, _) = parse_kst8_frame(&telemetry[offset..offset + KST8_FRAME_LENGTH])
            .map_err(|_| "frame")?;
        let point = KPH8_HEADER_LENGTH + point_index * KPH8_POINT_LENGTH;
        for (o, v) in [
            (0, frame.state.time.raw()),
            (4, frame.state.position.x()),
            (8, frame.state.position.y()),
            (12, frame.state.position.z()),
            (16, frame.state.velocity.z()),
        ] {
            w32(&mut output, point + o, v)
        }
        wu32(
            &mut output,
            point + 20,
            frame.events as u32 | ((frame.phase as u32) << 24),
        );
    }
    seal_phase8_record(&mut output).map_err(|_| "checksum")?;
    Ok(output)
}
pub fn parse_kph8(input: &[u8]) -> Result<Vec<Kph8Point>, &'static str> {
    validate_phase8_record(input, Phase8RecordKind::PlotHeader).map_err(|_| "record")?;
    if input.len() < KPH8_HEADER_LENGTH + 4 || input[56..KPH8_HEADER_LENGTH].iter().any(|v| *v != 0)
    {
        return Err("reserved");
    }
    let count = u16::from_le_bytes([input[32], input[33]]) as usize;
    let stride = u16::from_le_bytes([input[34], input[35]]) as usize;
    if stride != KPH8_POINT_LENGTH || input.len() != KPH8_HEADER_LENGTH + count * stride + 4 {
        return Err("length");
    }
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let o = KPH8_HEADER_LENGTH + index * stride;
        points.push(Kph8Point {
            time_raw: r32(input, o),
            east_raw: r32(input, o + 4),
            north_raw: r32(input, o + 8),
            altitude_raw: r32(input, o + 12),
            vertical_velocity_raw: r32(input, o + 16),
            phase_and_events: ru32(input, o + 20),
        })
    }
    Ok(points)
}
