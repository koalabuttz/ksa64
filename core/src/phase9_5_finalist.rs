//! Allocation-free KFE9 finalist package reader shared with stock-C64 presentation.
use crate::phase9_5_contract::{
    parse_advanced_effector_summary, AdvancedEffectorEvaluationSummary, KAS9_LENGTH,
};
use crate::phase9_contract::{CandidateAggregate, DesignVector, KDV9_LENGTH, KOE9_LENGTH};
use crate::scenario::crc32_ieee;

pub const KFE9_HEADER_LENGTH: usize = 128;
pub const KFE9_RECORD_LENGTH: usize = KDV9_LENGTH + KOE9_LENGTH + KAS9_LENGTH;
pub const KFE9_MAX_FINALISTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kfe9Error {
    Length,
    Magic,
    Version,
    Reserved,
    Checksum,
    Record,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFinalistRecord {
    pub design: DesignVector,
    pub aggregate: CandidateAggregate,
    pub summary: AdvancedEffectorEvaluationSummary,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFinalistPackage<'a> {
    input: &'a [u8],
    pub manifest_identity: u32,
    pub study_identity: u32,
    pub count: u8,
}
impl<'a> AdvancedFinalistPackage<'a> {
    pub fn parse(input: &'a [u8]) -> Result<Self, Kfe9Error> {
        if input.len() < KFE9_HEADER_LENGTH || &input[..4] != b"KFE9" {
            return Err(Kfe9Error::Magic);
        }
        if r16(input, 4) != 1
            || r16(input, 6) as usize != KFE9_HEADER_LENGTH
            || r32(input, 28) as usize != KFE9_RECORD_LENGTH
        {
            return Err(Kfe9Error::Version);
        }
        let count = r32(input, 16) as usize;
        if count > KFE9_MAX_FINALISTS
            || r32(input, 20) as usize != input.len() - KFE9_HEADER_LENGTH
            || input.len() != KFE9_HEADER_LENGTH + count * KFE9_RECORD_LENGTH
        {
            return Err(Kfe9Error::Length);
        }
        if input[36..124].iter().any(|b| *b != 0) {
            return Err(Kfe9Error::Reserved);
        }
        if r32(input, 24) != crc32_ieee(&input[KFE9_HEADER_LENGTH..])
            || r32(input, 124) != crc32_ieee(&input[..124])
        {
            return Err(Kfe9Error::Checksum);
        }
        Ok(Self {
            input,
            manifest_identity: r32(input, 8),
            study_identity: r32(input, 12),
            count: count as u8,
        })
    }
    fn offset(&self, index: usize) -> Result<usize, Kfe9Error> {
        if index >= self.count as usize {
            Err(Kfe9Error::Record)
        } else {
            Ok(KFE9_HEADER_LENGTH + index * KFE9_RECORD_LENGTH)
        }
    }
    pub fn design(&self, index: usize) -> Result<DesignVector, Kfe9Error> {
        let o = self.offset(index)?;
        let value =
            DesignVector::parse(&self.input[o..o + KDV9_LENGTH]).map_err(|_| Kfe9Error::Record)?;
        if value.manifest_identity != self.manifest_identity {
            return Err(Kfe9Error::Record);
        }
        Ok(value)
    }
    pub fn aggregate(&self, index: usize) -> Result<CandidateAggregate, Kfe9Error> {
        let o = self.offset(index)?;
        let value =
            CandidateAggregate::parse(&self.input[o + KDV9_LENGTH..o + KDV9_LENGTH + KOE9_LENGTH])
                .map_err(|_| Kfe9Error::Record)?;
        if value.manifest_identity != self.manifest_identity || !value.feasible {
            return Err(Kfe9Error::Record);
        }
        Ok(value)
    }
    pub fn summary(&self, index: usize) -> Result<AdvancedEffectorEvaluationSummary, Kfe9Error> {
        let o = self.offset(index)?;
        Ok(parse_advanced_effector_summary(
            &self.input[o + KDV9_LENGTH + KOE9_LENGTH..o + KFE9_RECORD_LENGTH],
        )
        .map_err(|_| Kfe9Error::Record)?
        .summary)
    }
    pub fn record(&self, index: usize) -> Result<AdvancedFinalistRecord, Kfe9Error> {
        let design = self.design(index)?;
        let aggregate = self.aggregate(index)?;
        if design.identity != aggregate.candidate_identity {
            return Err(Kfe9Error::Record);
        }
        let summary = self.summary(index)?;
        Ok(AdvancedFinalistRecord {
            design,
            aggregate,
            summary,
        })
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
    fn checked_in_mixed_package_is_strict() {
        let bytes = include_bytes!("../../phase9_5/evidence/workbench/mixed-nsga2.kfe9");
        let p = AdvancedFinalistPackage::parse(bytes).unwrap();
        assert_eq!(p.count, 8);
        assert!(p.record(0).unwrap().aggregate.feasible);
        let mut bad = bytes.to_vec();
        let n = bad.len();
        bad[n - 1] ^= 1;
        assert_eq!(
            AdvancedFinalistPackage::parse(&bad),
            Err(Kfe9Error::Checksum)
        );
    }
}
