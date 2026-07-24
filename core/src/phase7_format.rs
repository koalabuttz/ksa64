//! Common strict framing for Phase 7 records.

use crate::phase7_numeric::HOBBY_NUMERIC_CONTRACT_ID;
use crate::scenario::crc32_ieee;

pub const PHASE7_FORMAT_VERSION: u16 = 7;
pub const PHASE7_COMMON_HEADER_LENGTH: usize = 32;
pub const KVP7_LENGTH: usize = 512;
pub const KMP7_LENGTH: usize = 896;
pub const KMC7_LENGTH: usize = 256;
pub const KST7_HEADER_LENGTH: usize = 96;
pub const KST7_FRAME_LENGTH: usize = 96;
pub const KSR7_LENGTH: usize = 192;
pub const KSC7_LENGTH: usize = 512;
pub const KCL7_HEADER_LENGTH: usize = 64;
pub const KPH7_HEADER_LENGTH: usize = 64;
pub const KPH7_POINT_LENGTH: usize = 16;
pub const KRA7_HEADER_LENGTH: usize = 64;
pub const KMP7_MAX_KNOTS: usize = 64;
pub const KSC7_MAX_DISTRIBUTIONS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Phase7RecordKind {
    VehiclePack = 1,
    MotorPack = 2,
    MissionPack = 3,
    TelemetryHeader = 4,
    EvaluationSummary = 5,
    Campaign = 6,
    CandidateList = 7,
    PlotHeader = 8,
    ArchiveHeader = 9,
}

impl Phase7RecordKind {
    pub const fn magic(self) -> [u8; 4] {
        match self {
            Self::VehiclePack => *b"KVP7",
            Self::MotorPack => *b"KMP7",
            Self::MissionPack => *b"KMC7",
            Self::TelemetryHeader => *b"KST7",
            Self::EvaluationSummary => *b"KSR7",
            Self::Campaign => *b"KSC7",
            Self::CandidateList => *b"KCL7",
            Self::PlotHeader => *b"KPH7",
            Self::ArchiveHeader => *b"KRA7",
        }
    }

    pub const fn fixed_length(self) -> Option<usize> {
        match self {
            Self::VehiclePack => Some(KVP7_LENGTH),
            Self::MotorPack => Some(KMP7_LENGTH),
            Self::MissionPack => Some(KMC7_LENGTH),
            Self::TelemetryHeader => Some(KST7_HEADER_LENGTH),
            Self::EvaluationSummary => Some(KSR7_LENGTH),
            Self::Campaign => Some(KSC7_LENGTH),
            Self::CandidateList | Self::PlotHeader | Self::ArchiveHeader => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase7RecordError {
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
pub struct Phase7RecordHeader {
    pub kind: Phase7RecordKind,
    pub record_length: u16,
    pub identity: u32,
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub fn write_phase7_header(
    output: &mut [u8],
    kind: Phase7RecordKind,
    identity: u32,
) -> Result<(), Phase7RecordError> {
    if output.len() < PHASE7_COMMON_HEADER_LENGTH + 4 || output.len() > u16::MAX as usize {
        return Err(Phase7RecordError::Length);
    }
    if let Some(expected) = kind.fixed_length() {
        if output.len() != expected {
            return Err(Phase7RecordError::Length);
        }
    }
    output.fill(0);
    output[0..4].copy_from_slice(&kind.magic());
    write_u16(output, 4, PHASE7_FORMAT_VERSION);
    write_u16(output, 6, PHASE7_COMMON_HEADER_LENGTH as u16);
    write_u16(output, 8, output.len() as u16);
    write_u16(output, 10, kind as u16);
    write_u32(output, 12, HOBBY_NUMERIC_CONTRACT_ID);
    write_u32(output, 16, identity);
    Ok(())
}

pub fn seal_phase7_record(output: &mut [u8]) -> Result<u32, Phase7RecordError> {
    if output.len() < PHASE7_COMMON_HEADER_LENGTH + 4 {
        return Err(Phase7RecordError::Length);
    }
    let checksum_offset = output.len() - 4;
    let checksum = crc32_ieee(&output[..checksum_offset]);
    write_u32(output, checksum_offset, checksum);
    Ok(checksum)
}

pub fn validate_phase7_record(
    input: &[u8],
    expected_kind: Phase7RecordKind,
) -> Result<Phase7RecordHeader, Phase7RecordError> {
    if input.len() < PHASE7_COMMON_HEADER_LENGTH + 4 || input.len() > u16::MAX as usize {
        return Err(Phase7RecordError::Length);
    }
    if let Some(expected) = expected_kind.fixed_length() {
        if input.len() != expected {
            return Err(Phase7RecordError::Length);
        }
    }
    if input[0..4] != expected_kind.magic() {
        return Err(Phase7RecordError::Magic);
    }
    if read_u16(input, 4) != PHASE7_FORMAT_VERSION {
        return Err(Phase7RecordError::Version);
    }
    if read_u16(input, 6) as usize != PHASE7_COMMON_HEADER_LENGTH {
        return Err(Phase7RecordError::HeaderLength);
    }
    if read_u16(input, 8) as usize != input.len() {
        return Err(Phase7RecordError::Length);
    }
    if read_u16(input, 10) != expected_kind as u16 {
        return Err(Phase7RecordError::Kind);
    }
    if read_u32(input, 12) != HOBBY_NUMERIC_CONTRACT_ID {
        return Err(Phase7RecordError::NumericContract);
    }
    if input[20..PHASE7_COMMON_HEADER_LENGTH]
        .iter()
        .any(|value| *value != 0)
    {
        return Err(Phase7RecordError::Reserved);
    }
    let checksum_offset = input.len() - 4;
    if read_u32(input, checksum_offset) != crc32_ieee(&input[..checksum_offset]) {
        return Err(Phase7RecordError::Checksum);
    }
    Ok(Phase7RecordHeader {
        kind: expected_kind,
        record_length: input.len() as u16,
        identity: read_u32(input, 16),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixed_record_kinds_round_trip() {
        let mut bytes = [0u8; KSR7_LENGTH];
        write_phase7_header(&mut bytes, Phase7RecordKind::EvaluationSummary, 0x1234).unwrap();
        seal_phase7_record(&mut bytes).unwrap();
        let header = validate_phase7_record(&bytes, Phase7RecordKind::EvaluationSummary).unwrap();
        assert_eq!(header.identity, 0x1234);
        assert_eq!(header.record_length as usize, KSR7_LENGTH);
    }

    #[test]
    fn corruption_and_reserved_bytes_fail_closed() {
        let mut bytes = [0u8; KVP7_LENGTH];
        write_phase7_header(&mut bytes, Phase7RecordKind::VehiclePack, 7).unwrap();
        seal_phase7_record(&mut bytes).unwrap();
        bytes[24] = 1;
        seal_phase7_record(&mut bytes).unwrap();
        assert_eq!(
            validate_phase7_record(&bytes, Phase7RecordKind::VehiclePack),
            Err(Phase7RecordError::Reserved)
        );
        bytes[24] = 0;
        seal_phase7_record(&mut bytes).unwrap();
        bytes[100] ^= 1;
        assert_eq!(
            validate_phase7_record(&bytes, Phase7RecordKind::VehiclePack),
            Err(Phase7RecordError::Checksum)
        );
    }
}
