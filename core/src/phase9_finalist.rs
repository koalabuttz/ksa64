//! Allocation-free KFP9 finalist package reader shared with the stock-C64 browser.
use crate::phase9_contract::{CandidateAggregate, DesignVector, KDV9_LENGTH, KOE9_LENGTH};
use crate::scenario::crc32_ieee;
pub const KFP9_HEADER_LENGTH: usize = 64;
pub const KFP9_RECORD_LENGTH: usize = KDV9_LENGTH + KOE9_LENGTH;
pub const KFP9_MAX_FINALISTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kfp9Error {
    Length,
    Magic,
    Version,
    Reserved,
    Checksum,
    Record,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalistPackage<'a> {
    input: &'a [u8],
    pub manifest_identity: u32,
    pub study_identity: u32,
    pub count: u8,
}
impl<'a> FinalistPackage<'a> {
    pub fn parse(input: &'a [u8]) -> Result<Self, Kfp9Error> {
        if input.len() < KFP9_HEADER_LENGTH + 4 || &input[..4] != b"KFP9" {
            return Err(Kfp9Error::Magic);
        }
        if r16(input, 4) != 9
            || r16(input, 6) as usize != KFP9_HEADER_LENGTH
            || r32(input, 8) as usize != input.len()
        {
            return Err(Kfp9Error::Version);
        }
        let count = r16(input, 20) as usize;
        if count > KFP9_MAX_FINALISTS
            || input.len() != KFP9_HEADER_LENGTH + count * KFP9_RECORD_LENGTH + 4
        {
            return Err(Kfp9Error::Length);
        }
        if input[22..KFP9_HEADER_LENGTH].iter().any(|v| *v != 0) {
            return Err(Kfp9Error::Reserved);
        }
        if r32(input, input.len() - 4) != crc32_ieee(&input[..input.len() - 4]) {
            return Err(Kfp9Error::Checksum);
        }
        Ok(Self {
            input,
            manifest_identity: r32(input, 12),
            study_identity: r32(input, 16),
            count: count as u8,
        })
    }
    pub fn record(&self, index: usize) -> Result<(DesignVector, CandidateAggregate), Kfp9Error> {
        if index >= self.count as usize {
            return Err(Kfp9Error::Record);
        }
        let o = KFP9_HEADER_LENGTH + index * KFP9_RECORD_LENGTH;
        let design =
            DesignVector::parse(&self.input[o..o + KDV9_LENGTH]).map_err(|_| Kfp9Error::Record)?;
        let aggregate =
            CandidateAggregate::parse(&self.input[o + KDV9_LENGTH..o + KFP9_RECORD_LENGTH])
                .map_err(|_| Kfp9Error::Record)?;
        if design.identity != aggregate.candidate_identity
            || design.manifest_identity != self.manifest_identity
            || aggregate.manifest_identity != self.manifest_identity
        {
            return Err(Kfp9Error::Record);
        }
        Ok((design, aggregate))
    }
}
fn r16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn r32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_package_is_valid() {
        let mut b = [0u8; 68];
        b[..4].copy_from_slice(b"KFP9");
        b[4..6].copy_from_slice(&9u16.to_le_bytes());
        b[6..8].copy_from_slice(&64u16.to_le_bytes());
        b[8..12].copy_from_slice(&68u32.to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        let crc = crc32_ieee(&b[..64]);
        b[64..68].copy_from_slice(&crc.to_le_bytes());
        let p = FinalistPackage::parse(&b).unwrap();
        assert_eq!(p.count, 0);
        assert_eq!(p.record(0), Err(Kfp9Error::Record))
    }
}
