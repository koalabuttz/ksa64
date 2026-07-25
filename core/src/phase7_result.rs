//! Strict 192-byte KSR7 evaluation-summary records.

use crate::evaluation::{
    EvaluationOutcome, EvaluationSummary, MetricValidity, ModelProfileId,
    EVALUATION_CHECKSUM_COUNT, EVALUATION_V1_METRIC_COUNT,
};
use crate::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordError,
    Phase7RecordKind, KSR7_LENGTH,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ksr7Error {
    Record(Phase7RecordError),
    Profile,
    Outcome,
    Validity,
    Reserved,
}

impl From<Phase7RecordError> for Ksr7Error {
    fn from(value: Phase7RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ksr7Record {
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

pub fn evaluation_input_identity(summary: EvaluationSummary) -> u32 {
    let mut hash = 2_166_136_261u32;
    for word in summary.identities {
        for byte in word.to_le_bytes() {
            hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
        }
    }
    hash
}

pub fn encode_ksr7(
    summary: EvaluationSummary,
    output: &mut [u8; KSR7_LENGTH],
) -> Result<(), Ksr7Error> {
    if summary.profile == ModelProfileId::HobbySpatialV1 {
        return Err(Ksr7Error::Profile);
    }
    if summary.metric_validity.bits() & !((1u32 << EVALUATION_V1_METRIC_COUNT) - 1) != 0 {
        return Err(Ksr7Error::Validity);
    }
    write_phase7_header(
        output,
        Phase7RecordKind::EvaluationSummary,
        evaluation_input_identity(summary),
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
    for (index, value) in summary
        .metrics
        .iter()
        .take(EVALUATION_V1_METRIC_COUNT)
        .enumerate()
    {
        w32(output, 68 + index * 4, *value);
    }
    wu32(output, 164, summary.events);
    for (index, value) in summary.source_checksums.iter().enumerate() {
        wu32(output, 168 + index * 4, *value);
    }
    seal_phase7_record(output)?;
    Ok(())
}

fn parse_profile(value: u8) -> Result<ModelProfileId, Ksr7Error> {
    match value {
        1 => Ok(ModelProfileId::LegacyKsa2PlanarV1),
        2 => Ok(ModelProfileId::LegacyKsa5SpatialV1),
        3 => Ok(ModelProfileId::HobbyVerticalV1),
        _ => Err(Ksr7Error::Profile),
    }
}

fn parse_outcome(value: u8) -> Result<EvaluationOutcome, Ksr7Error> {
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
        _ => Err(Ksr7Error::Outcome),
    }
}

pub fn parse_ksr7(input: &[u8]) -> Result<Ksr7Record, Ksr7Error> {
    let header = validate_phase7_record(input, Phase7RecordKind::EvaluationSummary)?;
    if input[35] != 0 {
        return Err(Ksr7Error::Reserved);
    }
    let validity_bits = ru32(input, 40);
    if validity_bits & !((1u32 << EVALUATION_V1_METRIC_COUNT) - 1) != 0 {
        return Err(Ksr7Error::Validity);
    }
    let mut summary = EvaluationSummary::empty(parse_profile(input[32])?);
    summary.outcome = parse_outcome(input[33])?;
    summary.numeric_faults = input[34];
    summary.steps = ru32(input, 36);
    summary.metric_validity = MetricValidity::from_bits(validity_bits);
    for index in 0..3 {
        summary.terminal_state_a[index] = r32(input, 44 + index * 4);
        summary.terminal_state_b[index] = r32(input, 56 + index * 4);
    }
    for index in 0..EVALUATION_V1_METRIC_COUNT {
        summary.metrics[index] = r32(input, 68 + index * 4);
    }
    summary.events = ru32(input, 164);
    for index in 0..EVALUATION_CHECKSUM_COUNT {
        summary.source_checksums[index] = ru32(input, 168 + index * 4);
    }
    Ok(Ksr7Record {
        input_identity: header.identity,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::MetricSlot;

    #[test]
    fn ksr7_round_trip_preserves_evaluation_fields() {
        let mut summary = EvaluationSummary::empty(ModelProfileId::HobbyVerticalV1);
        summary.outcome = EvaluationOutcome::GroundContact;
        summary.steps = 123;
        summary.identities = [1, 2, 3, 4, 5, 6];
        summary.source_checksums = [7, 8, 9, 10, 11];
        summary.set_metric(MetricSlot::ApogeeAltitude, 42);
        let mut bytes = [0u8; KSR7_LENGTH];
        encode_ksr7(summary, &mut bytes).unwrap();
        let decoded = parse_ksr7(&bytes).unwrap();
        assert_eq!(decoded.input_identity, evaluation_input_identity(summary));
        assert_eq!(decoded.summary.metric(MetricSlot::ApogeeAltitude), Some(42));
        assert_eq!(decoded.summary.source_checksums, summary.source_checksums);
    }

    #[test]
    fn ksr7_rejects_phase8_profile_and_upper_metrics() {
        let mut output = [0u8; KSR7_LENGTH];
        let spatial = EvaluationSummary::empty(ModelProfileId::HobbySpatialV1);
        assert_eq!(encode_ksr7(spatial, &mut output), Err(Ksr7Error::Profile));

        let mut vertical = EvaluationSummary::empty(ModelProfileId::HobbyVerticalV1);
        vertical.set_metric(MetricSlot::MinimumStaticMargin, 1);
        assert_eq!(encode_ksr7(vertical, &mut output), Err(Ksr7Error::Validity));
    }
}
