//! Strict 256-byte KSR8 spatial evaluation-summary records.

use crate::evaluation::{
    EvaluationOutcome, EvaluationSummary, MetricValidity, ModelProfileId,
    EVALUATION_CHECKSUM_COUNT, EVALUATION_IDENTITY_COUNT, EVALUATION_METRIC_COUNT,
};
use crate::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordError,
    Phase8RecordKind, KSR8_LENGTH,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ksr8Error {
    Record(Phase8RecordError),
    Profile,
    Outcome,
    Reserved,
}
impl From<Phase8RecordError> for Ksr8Error {
    fn from(value: Phase8RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ksr8Record {
    pub input_identity: u32,
    pub summary: EvaluationSummary,
}

fn r32(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub fn spatial_evaluation_identity(summary: EvaluationSummary) -> u32 {
    let mut hash = 2_166_136_261u32;
    for word in summary
        .identities
        .into_iter()
        .chain(summary.source_checksums)
    {
        for byte in word.to_le_bytes() {
            hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
        }
    }
    hash
}

pub fn encode_ksr8(
    summary: EvaluationSummary,
    output: &mut [u8; KSR8_LENGTH],
) -> Result<(), Ksr8Error> {
    if summary.profile != ModelProfileId::HobbySpatialV1 {
        return Err(Ksr8Error::Profile);
    }
    write_phase8_header(
        output,
        Phase8RecordKind::EvaluationSummary,
        spatial_evaluation_identity(summary),
    )?;
    output[32] = summary.profile as u8;
    output[33] = summary.outcome as u8;
    output[34] = summary.numeric_faults;
    wu32(output, 36, summary.steps);
    wu32(output, 40, summary.metric_validity.bits());
    for (index, value) in summary.terminal_state_a.iter().enumerate() {
        w32(output, 44 + index * 4, *value);
    }
    for (index, value) in summary.terminal_state_b.iter().enumerate() {
        w32(output, 56 + index * 4, *value);
    }
    for (index, value) in summary.metrics.iter().enumerate() {
        w32(output, 68 + index * 4, *value);
    }
    wu32(output, 196, summary.events);
    for (index, value) in summary.identities.iter().enumerate() {
        wu32(output, 200 + index * 4, *value);
    }
    for (index, value) in summary.source_checksums.iter().enumerate() {
        wu32(output, 224 + index * 4, *value);
    }
    seal_phase8_record(output)?;
    Ok(())
}

fn parse_outcome(value: u8) -> Result<EvaluationOutcome, Ksr8Error> {
    match value {
        0 => Ok(EvaluationOutcome::Complete),
        1 => Ok(EvaluationOutcome::StableOrbit),
        2 => Ok(EvaluationOutcome::CompleteNotOrbit),
        3 => Ok(EvaluationOutcome::GroundContact),
        4 => Ok(EvaluationOutcome::Aborted),
        5 => Ok(EvaluationOutcome::NumericFault),
        6 => Ok(EvaluationOutcome::StepLimit),
        7 => Ok(EvaluationOutcome::NoLiftoff),
        8 => Ok(EvaluationOutcome::ConfigurationFault),
        9 => Ok(EvaluationOutcome::RecoveryIncomplete),
        10 => Ok(EvaluationOutcome::ModelEnvelopeExceeded),
        _ => Err(Ksr8Error::Outcome),
    }
}

pub fn parse_ksr8(input: &[u8]) -> Result<Ksr8Record, Ksr8Error> {
    let header = validate_phase8_record(input, Phase8RecordKind::EvaluationSummary)?;
    if input[32] != ModelProfileId::HobbySpatialV1 as u8 {
        return Err(Ksr8Error::Profile);
    }
    if input[35] != 0 || input[244..252].iter().any(|value| *value != 0) {
        return Err(Ksr8Error::Reserved);
    }
    let mut summary = EvaluationSummary::empty(ModelProfileId::HobbySpatialV1);
    summary.outcome = parse_outcome(input[33])?;
    summary.numeric_faults = input[34];
    summary.steps = ru32(input, 36);
    summary.metric_validity = MetricValidity::from_bits(ru32(input, 40));
    for index in 0..3 {
        summary.terminal_state_a[index] = r32(input, 44 + index * 4);
        summary.terminal_state_b[index] = r32(input, 56 + index * 4);
    }
    for index in 0..EVALUATION_METRIC_COUNT {
        summary.metrics[index] = r32(input, 68 + index * 4);
    }
    summary.events = ru32(input, 196);
    for index in 0..EVALUATION_IDENTITY_COUNT {
        summary.identities[index] = ru32(input, 200 + index * 4);
    }
    for index in 0..EVALUATION_CHECKSUM_COUNT {
        summary.source_checksums[index] = ru32(input, 224 + index * 4);
    }
    Ok(Ksr8Record {
        input_identity: header.identity,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::MetricSlot;
    #[test]
    fn round_trip_and_corruption() {
        let mut summary = EvaluationSummary::empty(ModelProfileId::HobbySpatialV1);
        summary.outcome = EvaluationOutcome::GroundContact;
        summary.identities = [1, 2, 3, 4, 5, 6];
        summary.set_metric(MetricSlot::LandingDistance, 42);
        let mut bytes = [0u8; KSR8_LENGTH];
        encode_ksr8(summary, &mut bytes).unwrap();
        assert_eq!(parse_ksr8(&bytes).unwrap().summary, summary);
        bytes[100] ^= 1;
        assert!(parse_ksr8(&bytes).is_err());
    }
}
