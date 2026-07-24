//! Strict run-bound KST4 detailed telemetry archives.

use ksa64_interface::crc32_ieee;

use crate::telemetry::{
    parse_phase3_telemetry_frame, Phase3TelemetryError, Phase3TelemetryFrame,
    PHASE3_TELEMETRY_CONTRACT_ID, PHASE3_TELEMETRY_STRIDE,
};

use super::contracts::{DETAIL_FRAME_LENGTH, DETAIL_HEADER_LENGTH, KST4_MAGIC};
use super::PHASE4_CONTRACT_ID;

pub const KST4_VERSION: u16 = 4;
const HEADER_CRC_OFFSET: usize = DETAIL_HEADER_LENGTH - 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetailHeader {
    pub campaign_crc32: u32,
    pub run_index: u32,
    pub derived_seed: u32,
    pub variation_crc32: u32,
    pub frame_count: u32,
    pub payload_crc32: u32,
    pub first_step: u32,
    pub final_step: u32,
    pub final_truth_checksum: u32,
    pub final_sensor_checksum: u32,
    pub final_navigation_checksum: u32,
    pub final_flight_checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailError {
    Length,
    Magic,
    Version,
    Contract,
    Stride,
    Reserved,
    Checksum,
    Frame {
        index: u32,
        cause: Phase3TelemetryError,
    },
    Identity,
}

pub struct DetailArchive<'a> {
    pub header: DetailHeader,
    pub frames: &'a [u8],
}

impl DetailArchive<'_> {
    pub fn frame(&self, index: u32) -> Result<Phase3TelemetryFrame, DetailError> {
        if index >= self.header.frame_count {
            return Err(DetailError::Length);
        }
        validate_frame(self.frames, index)
    }
}

pub fn write_kst4(
    campaign_crc32: u32,
    run_index: u32,
    derived_seed: u32,
    variation_crc32: u32,
    frames: &[u8],
    output: &mut [u8],
) -> Result<DetailHeader, DetailError> {
    if frames.is_empty()
        || frames.len().checked_rem(DETAIL_FRAME_LENGTH) != Some(0)
        || output.len() != DETAIL_HEADER_LENGTH + frames.len()
        || frames.len() > u32::MAX as usize
    {
        return Err(DetailError::Length);
    }
    let frame_count = (frames.len() / DETAIL_FRAME_LENGTH) as u32;
    let first = validate_frame(frames, 0)?;
    let final_frame = validate_frame(frames, frame_count - 1)?;
    let mut index = 1u32;
    while index + 1 < frame_count {
        validate_frame(frames, index)?;
        index += 1;
    }
    let header = DetailHeader {
        campaign_crc32,
        run_index,
        derived_seed,
        variation_crc32,
        frame_count,
        payload_crc32: crc32_ieee(frames),
        first_step: first.step,
        final_step: final_frame.step,
        final_truth_checksum: final_frame.truth_checksum,
        final_sensor_checksum: final_frame.sensor_checksum,
        final_navigation_checksum: final_frame.nav_checksum,
        final_flight_checksum: final_frame.flight_checksum,
    };
    output.fill(0);
    output[0..4].copy_from_slice(&KST4_MAGIC);
    put_u16(output, 4, KST4_VERSION);
    put_u16(output, 6, DETAIL_HEADER_LENGTH as u16);
    put_u16(output, 8, DETAIL_FRAME_LENGTH as u16);
    put_u16(output, 10, PHASE3_TELEMETRY_STRIDE);
    put_u32(output, 12, PHASE4_CONTRACT_ID);
    put_u32(output, 16, PHASE3_TELEMETRY_CONTRACT_ID);
    put_u32(output, 20, header.campaign_crc32);
    put_u32(output, 24, header.run_index);
    put_u32(output, 28, header.derived_seed);
    put_u32(output, 32, header.variation_crc32);
    put_u32(output, 36, header.frame_count);
    put_u32(output, 40, frames.len() as u32);
    put_u32(output, 44, header.payload_crc32);
    put_u32(output, 48, header.first_step);
    put_u32(output, 52, header.final_step);
    put_u32(output, 56, header.final_truth_checksum);
    put_u32(output, 60, header.final_sensor_checksum);
    put_u32(output, 64, header.final_navigation_checksum);
    put_u32(output, 68, header.final_flight_checksum);
    output[DETAIL_HEADER_LENGTH..].copy_from_slice(frames);
    let header_crc = crc32_ieee(&output[..HEADER_CRC_OFFSET]);
    put_u32(output, HEADER_CRC_OFFSET, header_crc);
    Ok(header)
}

pub fn parse_kst4(input: &[u8]) -> Result<DetailArchive<'_>, DetailError> {
    if input.len() < DETAIL_HEADER_LENGTH {
        return Err(DetailError::Length);
    }
    if input[0..4] != KST4_MAGIC {
        return Err(DetailError::Magic);
    }
    if get_u16(input, 4) != KST4_VERSION
        || get_u16(input, 6) != DETAIL_HEADER_LENGTH as u16
        || get_u16(input, 8) != DETAIL_FRAME_LENGTH as u16
    {
        return Err(DetailError::Version);
    }
    if get_u16(input, 10) != PHASE3_TELEMETRY_STRIDE {
        return Err(DetailError::Stride);
    }
    if get_u32(input, 12) != PHASE4_CONTRACT_ID
        || get_u32(input, 16) != PHASE3_TELEMETRY_CONTRACT_ID
    {
        return Err(DetailError::Contract);
    }
    if input[72..HEADER_CRC_OFFSET].iter().any(|&byte| byte != 0) {
        return Err(DetailError::Reserved);
    }
    if get_u32(input, HEADER_CRC_OFFSET) != crc32_ieee(&input[..HEADER_CRC_OFFSET]) {
        return Err(DetailError::Checksum);
    }
    let frame_count = get_u32(input, 36);
    let payload_length = get_u32(input, 40);
    if frame_count == 0
        || payload_length
            != frame_count
                .checked_mul(DETAIL_FRAME_LENGTH as u32)
                .ok_or(DetailError::Length)?
        || input.len() != DETAIL_HEADER_LENGTH + payload_length as usize
    {
        return Err(DetailError::Length);
    }
    let frames = &input[DETAIL_HEADER_LENGTH..];
    if get_u32(input, 44) != crc32_ieee(frames) {
        return Err(DetailError::Checksum);
    }
    let first = validate_frame(frames, 0)?;
    let final_frame = validate_frame(frames, frame_count - 1)?;
    let mut index = 1u32;
    while index + 1 < frame_count {
        validate_frame(frames, index)?;
        index += 1;
    }
    let header = DetailHeader {
        campaign_crc32: get_u32(input, 20),
        run_index: get_u32(input, 24),
        derived_seed: get_u32(input, 28),
        variation_crc32: get_u32(input, 32),
        frame_count,
        payload_crc32: get_u32(input, 44),
        first_step: get_u32(input, 48),
        final_step: get_u32(input, 52),
        final_truth_checksum: get_u32(input, 56),
        final_sensor_checksum: get_u32(input, 60),
        final_navigation_checksum: get_u32(input, 64),
        final_flight_checksum: get_u32(input, 68),
    };
    if header.first_step != first.step
        || header.final_step != final_frame.step
        || header.final_truth_checksum != final_frame.truth_checksum
        || header.final_sensor_checksum != final_frame.sensor_checksum
        || header.final_navigation_checksum != final_frame.nav_checksum
        || header.final_flight_checksum != final_frame.flight_checksum
    {
        return Err(DetailError::Identity);
    }
    Ok(DetailArchive { header, frames })
}

fn validate_frame(frames: &[u8], index: u32) -> Result<Phase3TelemetryFrame, DetailError> {
    let start = index as usize * DETAIL_FRAME_LENGTH;
    parse_phase3_telemetry_frame(&frames[start..start + DETAIL_FRAME_LENGTH])
        .map_err(|cause| DetailError::Frame { index, cause })
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
