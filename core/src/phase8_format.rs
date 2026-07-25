//! Strict common framing for Phase 8 spatial-hobby records.

use crate::phase8_numeric::HOBBY_SPATIAL_NUMERIC_CONTRACT_ID;
use crate::scenario::crc32_ieee;

pub const PHASE8_FORMAT_VERSION: u16 = 8;
pub const PHASE8_COMMON_HEADER_LENGTH: usize = 32;
pub const KVP8_LENGTH: usize = 1_024;
pub const KMP8_LENGTH: usize = 1_024;
pub const KMC8_LENGTH: usize = 512;
pub const KWP8_LENGTH: usize = 512;
pub const KST8_HEADER_LENGTH: usize = 128;
pub const KST8_FRAME_LENGTH: usize = 160;
pub const KSR8_LENGTH: usize = 256;
pub const KSC8_LENGTH: usize = 512;
pub const KPH8_HEADER_LENGTH: usize = 64;
pub const KPH8_POINT_LENGTH: usize = 24;
pub const KRA8_HEADER_LENGTH: usize = 64;
pub const KMP8_MAX_KNOTS: usize = 64;
pub const KVP8_MAX_AERO_KNOTS: usize = 16;
pub const KWP8_MAX_WIND_KNOTS: usize = 16;
pub const KSC8_MAX_DISTRIBUTIONS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Phase8RecordKind {
    VehiclePack = 1,
    MotorPack = 2,
    MissionPack = 3,
    WindPack = 4,
    TelemetryHeader = 5,
    EvaluationSummary = 6,
    Campaign = 7,
    PlotHeader = 8,
    ArchiveHeader = 9,
}

impl Phase8RecordKind {
    pub const fn magic(self) -> [u8; 4] {
        match self {
            Self::VehiclePack => *b"KVP8",
            Self::MotorPack => *b"KMP8",
            Self::MissionPack => *b"KMC8",
            Self::WindPack => *b"KWP8",
            Self::TelemetryHeader => *b"KST8",
            Self::EvaluationSummary => *b"KSR8",
            Self::Campaign => *b"KSC8",
            Self::PlotHeader => *b"KPH8",
            Self::ArchiveHeader => *b"KRA8",
        }
    }

    pub const fn fixed_length(self) -> Option<usize> {
        match self {
            Self::VehiclePack => Some(KVP8_LENGTH),
            Self::MotorPack => Some(KMP8_LENGTH),
            Self::MissionPack => Some(KMC8_LENGTH),
            Self::WindPack => Some(KWP8_LENGTH),
            Self::TelemetryHeader => Some(KST8_HEADER_LENGTH),
            Self::EvaluationSummary => Some(KSR8_LENGTH),
            Self::Campaign => Some(KSC8_LENGTH),
            Self::PlotHeader | Self::ArchiveHeader => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase8RecordError {
    Length,
    Magic,
    Version,
    HeaderLength,
    Kind,
    NumericContract,
    Reserved,
    Checksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase8RecordHeader {
    pub kind: Phase8RecordKind,
    pub record_length: u16,
    pub identity: u32,
}

fn r16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn r32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn w16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn w32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub fn write_phase8_header(
    output: &mut [u8],
    kind: Phase8RecordKind,
    identity: u32,
) -> Result<(), Phase8RecordError> {
    if output.len() < PHASE8_COMMON_HEADER_LENGTH + 4 || output.len() > u16::MAX as usize {
        return Err(Phase8RecordError::Length);
    }
    if let Some(expected) = kind.fixed_length() {
        if output.len() != expected {
            return Err(Phase8RecordError::Length);
        }
    }
    output.fill(0);
    output[0..4].copy_from_slice(&kind.magic());
    w16(output, 4, PHASE8_FORMAT_VERSION);
    w16(output, 6, PHASE8_COMMON_HEADER_LENGTH as u16);
    w16(output, 8, output.len() as u16);
    w16(output, 10, kind as u16);
    w32(output, 12, HOBBY_SPATIAL_NUMERIC_CONTRACT_ID);
    w32(output, 16, identity);
    Ok(())
}

pub fn seal_phase8_record(output: &mut [u8]) -> Result<u32, Phase8RecordError> {
    if output.len() < PHASE8_COMMON_HEADER_LENGTH + 4 {
        return Err(Phase8RecordError::Length);
    }
    let offset = output.len() - 4;
    let checksum = crc32_ieee(&output[..offset]);
    w32(output, offset, checksum);
    Ok(checksum)
}

pub fn validate_phase8_record(
    input: &[u8],
    expected_kind: Phase8RecordKind,
) -> Result<Phase8RecordHeader, Phase8RecordError> {
    if input.len() < PHASE8_COMMON_HEADER_LENGTH + 4 || input.len() > u16::MAX as usize {
        return Err(Phase8RecordError::Length);
    }
    if input[0..4] != expected_kind.magic() {
        return Err(Phase8RecordError::Magic);
    }
    if r16(input, 4) != PHASE8_FORMAT_VERSION {
        return Err(Phase8RecordError::Version);
    }
    if r16(input, 6) as usize != PHASE8_COMMON_HEADER_LENGTH {
        return Err(Phase8RecordError::HeaderLength);
    }
    if r16(input, 8) as usize != input.len() {
        return Err(Phase8RecordError::Length);
    }
    if r16(input, 10) != expected_kind as u16 {
        return Err(Phase8RecordError::Kind);
    }
    if r32(input, 12) != HOBBY_SPATIAL_NUMERIC_CONTRACT_ID {
        return Err(Phase8RecordError::NumericContract);
    }
    if input[20..PHASE8_COMMON_HEADER_LENGTH]
        .iter()
        .any(|value| *value != 0)
    {
        return Err(Phase8RecordError::Reserved);
    }
    if let Some(expected) = expected_kind.fixed_length() {
        if input.len() != expected {
            return Err(Phase8RecordError::Length);
        }
    }
    let offset = input.len() - 4;
    if r32(input, offset) != crc32_ieee(&input[..offset]) {
        return Err(Phase8RecordError::Checksum);
    }
    Ok(Phase8RecordHeader {
        kind: expected_kind,
        record_length: input.len() as u16,
        identity: r32(input, 16),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixed_record_kind_round_trips() {
        for (kind, length) in [
            (Phase8RecordKind::VehiclePack, KVP8_LENGTH),
            (Phase8RecordKind::MotorPack, KMP8_LENGTH),
            (Phase8RecordKind::MissionPack, KMC8_LENGTH),
            (Phase8RecordKind::WindPack, KWP8_LENGTH),
            (Phase8RecordKind::TelemetryHeader, KST8_HEADER_LENGTH),
            (Phase8RecordKind::EvaluationSummary, KSR8_LENGTH),
            (Phase8RecordKind::Campaign, KSC8_LENGTH),
        ] {
            let mut bytes = [0u8; KVP8_LENGTH];
            let record = &mut bytes[..length];
            write_phase8_header(record, kind, 0x1234_5678).unwrap();
            seal_phase8_record(record).unwrap();
            let header = validate_phase8_record(record, kind).unwrap();
            assert_eq!(header.identity, 0x1234_5678);
            assert_eq!(header.record_length as usize, length);
        }
    }

    #[test]
    fn corruption_and_reserved_header_bytes_fail_closed() {
        let mut bytes = [0u8; KMC8_LENGTH];
        write_phase8_header(&mut bytes, Phase8RecordKind::MissionPack, 7).unwrap();
        seal_phase8_record(&mut bytes).unwrap();
        bytes[24] = 1;
        seal_phase8_record(&mut bytes).unwrap();
        assert_eq!(
            validate_phase8_record(&bytes, Phase8RecordKind::MissionPack),
            Err(Phase8RecordError::Reserved)
        );
        bytes[24] = 0;
        seal_phase8_record(&mut bytes).unwrap();
        bytes[40] ^= 1;
        assert_eq!(
            validate_phase8_record(&bytes, Phase8RecordKind::MissionPack),
            Err(Phase8RecordError::Checksum)
        );
    }
}
