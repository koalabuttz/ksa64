//! Compact, strict Phase 11 deterministic debrief contract (KDR11).

use crate::{crc32_ieee, CodecError};

pub const KDR11_LENGTH: usize = 512;
pub const KDR11_MAGIC: [u8; 4] = *b"KDR1";
pub const KDR11_VERSION: u16 = 1;
const KDR11_CRC_OFFSET: usize = KDR11_LENGTH - 4;
const COUNTERFACTUAL_OFFSET: usize = 88;
const COUNTERFACTUAL_LENGTH: usize = 16;
pub const KDR11_COUNTERFACTUALS: usize = 4;

pub const DEBRIEF_FLAG_DIRECT_OBSERVATIONS: u32 = 1 << 0;
pub const DEBRIEF_FLAG_PROCEDURE_EVIDENCE: u32 = 1 << 1;
pub const DEBRIEF_FLAG_PREDICTION_RESIDUALS: u32 = 1 << 2;
pub const DEBRIEF_FLAG_MODEL_EXPLANATIONS: u32 = 1 << 3;
pub const DEBRIEF_FLAG_CONTROLLED_COUNTERFACTUALS: u32 = 1 << 4;
pub const DEBRIEF_FLAG_UNRESOLVED_HYPOTHESES: u32 = 1 << 5;
pub const DEBRIEF_FLAG_MASK: u32 = (1 << 6) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DebriefOutcome {
    Nominal = 1,
    Recovered = 2,
    SafeState = 3,
    Incomplete = 4,
    Rejected = 5,
}

impl DebriefOutcome {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Nominal),
            2 => Ok(Self::Recovered),
            3 => Ok(Self::SafeState),
            4 => Ok(Self::Incomplete),
            5 => Ok(Self::Rejected),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DebriefProcedureState {
    NotApplicable = 0,
    Completed = 1,
    Skipped = 2,
    Failed = 3,
    Mistimed = 4,
    ManuallyOverridden = 5,
    ActiveAtTermination = 6,
}

impl DebriefProcedureState {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::NotApplicable),
            1 => Ok(Self::Completed),
            2 => Ok(Self::Skipped),
            3 => Ok(Self::Failed),
            4 => Ok(Self::Mistimed),
            5 => Ok(Self::ManuallyOverridden),
            6 => Ok(Self::ActiveAtTermination),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CounterfactualFactor {
    NoGnssFailure = 1,
    NoOperatorAction = 2,
    AcceptedUpdateAndBranch = 3,
    DelayedAction = 4,
}

impl CounterfactualFactor {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::NoGnssFailure),
            2 => Ok(Self::NoOperatorAction),
            3 => Ok(Self::AcceptedUpdateAndBranch),
            4 => Ok(Self::DelayedAction),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterfactualDebrief {
    pub scenario_identity: u32,
    pub factor: CounterfactualFactor,
    pub outcome: DebriefOutcome,
    pub flags: u16,
    pub evidence_identity: u32,
    pub primary_delta: i32,
}

impl CounterfactualDebrief {
    pub const EMPTY: Self = Self {
        scenario_identity: 0,
        factor: CounterfactualFactor::NoGnssFailure,
        outcome: DebriefOutcome::Incomplete,
        flags: 0,
        evidence_identity: 0,
        primary_delta: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebriefSummary {
    pub debrief_identity: u32,
    pub session_definition_identity: u32,
    pub action_transcript_identity: u32,
    pub completed_evidence_identity: u32,
    pub scenario_identity: u32,
    pub flags: u32,
    pub outcome: DebriefOutcome,
    pub procedure_state: DebriefProcedureState,
    pub direct_observation_count: u16,
    pub procedure_completed_steps: u16,
    pub procedure_skipped_steps: u16,
    pub procedure_failed_steps: u16,
    pub procedure_mistimed_steps: u16,
    pub manual_override_count: u16,
    pub hint_count: u16,
    pub rejected_action_count: u16,
    pub prediction_apogee_residual_q12: i32,
    pub prediction_time_residual_q16: i32,
    pub prediction_impact_residual_q12: i32,
    pub evidence_checksums: [u32; 6],
    pub counterfactuals: [CounterfactualDebrief; KDR11_COUNTERFACTUALS],
    pub unresolved_hypothesis_mask: u32,
    pub causal_claim_mask: u32,
    pub model_explanation_mask: u32,
    pub observation_identity: u32,
    pub procedure_identity: u32,
    pub prediction_identity: u32,
    pub journal_identity: u32,
}

pub fn write_kdr11(value: &DebriefSummary, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KDR11_LENGTH
        || value.debrief_identity == 0
        || value.session_definition_identity == 0
        || value.action_transcript_identity == 0
        || value.completed_evidence_identity == 0
        || value.scenario_identity == 0
        || value.flags == 0
        || value.flags & !DEBRIEF_FLAG_MASK != 0
        || value.observation_identity == 0
        || value.procedure_identity == 0
        || value.prediction_identity == 0
        || value.journal_identity == 0
        || value.counterfactuals.iter().any(|item| {
            item.scenario_identity == 0 || item.evidence_identity == 0 || item.flags != 0
        })
    {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    output[..4].copy_from_slice(&KDR11_MAGIC);
    p16(output, 4, KDR11_VERSION);
    p16(output, 6, KDR11_LENGTH as u16);
    for (offset, word) in [
        (8, value.debrief_identity),
        (12, value.session_definition_identity),
        (16, value.action_transcript_identity),
        (20, value.completed_evidence_identity),
        (24, value.scenario_identity),
        (28, value.flags),
    ] {
        p32(output, offset, word);
    }
    output[32] = value.outcome as u8;
    output[33] = value.procedure_state as u8;
    for (offset, word) in [
        (36, value.direct_observation_count),
        (38, value.procedure_completed_steps),
        (40, value.procedure_skipped_steps),
        (42, value.procedure_failed_steps),
        (44, value.procedure_mistimed_steps),
        (46, value.manual_override_count),
        (48, value.hint_count),
        (50, value.rejected_action_count),
    ] {
        p16(output, offset, word);
    }
    p32(output, 52, value.prediction_apogee_residual_q12 as u32);
    p32(output, 56, value.prediction_time_residual_q16 as u32);
    p32(output, 60, value.prediction_impact_residual_q12 as u32);
    for (index, checksum) in value.evidence_checksums.iter().enumerate() {
        p32(output, 64 + index * 4, *checksum);
    }
    for (index, item) in value.counterfactuals.iter().enumerate() {
        let offset = COUNTERFACTUAL_OFFSET + index * COUNTERFACTUAL_LENGTH;
        p32(output, offset, item.scenario_identity);
        output[offset + 4] = item.factor as u8;
        output[offset + 5] = item.outcome as u8;
        p16(output, offset + 6, item.flags);
        p32(output, offset + 8, item.evidence_identity);
        p32(output, offset + 12, item.primary_delta as u32);
    }
    for (offset, word) in [
        (152, value.unresolved_hypothesis_mask),
        (156, value.causal_claim_mask),
        (160, value.model_explanation_mask),
        (164, value.observation_identity),
        (168, value.procedure_identity),
        (172, value.prediction_identity),
        (176, value.journal_identity),
    ] {
        p32(output, offset, word);
    }
    p32(
        output,
        KDR11_CRC_OFFSET,
        crc32_ieee(&output[..KDR11_CRC_OFFSET]),
    );
    Ok(())
}

pub fn parse_kdr11(input: &[u8]) -> Result<DebriefSummary, CodecError> {
    if input.len() != KDR11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != KDR11_MAGIC
        || g16(input, 4) != KDR11_VERSION
        || g16(input, 6) != KDR11_LENGTH as u16
    {
        return Err(CodecError::Enum);
    }
    if g32(input, KDR11_CRC_OFFSET) != crc32_ieee(&input[..KDR11_CRC_OFFSET]) {
        return Err(CodecError::Checksum);
    }
    if input[34] != 0
        || input[35] != 0
        || input[180..KDR11_CRC_OFFSET].iter().any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut checksums = [0; 6];
    for (index, checksum) in checksums.iter_mut().enumerate() {
        *checksum = g32(input, 64 + index * 4);
    }
    let mut counterfactuals = [CounterfactualDebrief::EMPTY; KDR11_COUNTERFACTUALS];
    for (index, item) in counterfactuals.iter_mut().enumerate() {
        let offset = COUNTERFACTUAL_OFFSET + index * COUNTERFACTUAL_LENGTH;
        *item = CounterfactualDebrief {
            scenario_identity: g32(input, offset),
            factor: CounterfactualFactor::parse(input[offset + 4])?,
            outcome: DebriefOutcome::parse(input[offset + 5])?,
            flags: g16(input, offset + 6),
            evidence_identity: g32(input, offset + 8),
            primary_delta: g32(input, offset + 12) as i32,
        };
    }
    let value = DebriefSummary {
        debrief_identity: g32(input, 8),
        session_definition_identity: g32(input, 12),
        action_transcript_identity: g32(input, 16),
        completed_evidence_identity: g32(input, 20),
        scenario_identity: g32(input, 24),
        flags: g32(input, 28),
        outcome: DebriefOutcome::parse(input[32])?,
        procedure_state: DebriefProcedureState::parse(input[33])?,
        direct_observation_count: g16(input, 36),
        procedure_completed_steps: g16(input, 38),
        procedure_skipped_steps: g16(input, 40),
        procedure_failed_steps: g16(input, 42),
        procedure_mistimed_steps: g16(input, 44),
        manual_override_count: g16(input, 46),
        hint_count: g16(input, 48),
        rejected_action_count: g16(input, 50),
        prediction_apogee_residual_q12: g32(input, 52) as i32,
        prediction_time_residual_q16: g32(input, 56) as i32,
        prediction_impact_residual_q12: g32(input, 60) as i32,
        evidence_checksums: checksums,
        counterfactuals,
        unresolved_hypothesis_mask: g32(input, 152),
        causal_claim_mask: g32(input, 156),
        model_explanation_mask: g32(input, 160),
        observation_identity: g32(input, 164),
        procedure_identity: g32(input, 168),
        prediction_identity: g32(input, 172),
        journal_identity: g32(input, 176),
    };
    let mut canonical = [0; KDR11_LENGTH];
    write_kdr11(&value, &mut canonical)?;
    if canonical != input {
        return Err(CodecError::Reserved);
    }
    Ok(value)
}

fn p16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn p32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn g16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn g32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> DebriefSummary {
        let factors = [
            CounterfactualFactor::NoGnssFailure,
            CounterfactualFactor::NoOperatorAction,
            CounterfactualFactor::AcceptedUpdateAndBranch,
            CounterfactualFactor::DelayedAction,
        ];
        let mut counterfactuals = [CounterfactualDebrief::EMPTY; KDR11_COUNTERFACTUALS];
        for (index, factor) in factors.into_iter().enumerate() {
            counterfactuals[index] = CounterfactualDebrief {
                scenario_identity: 0x11b0_0100 + index as u32,
                factor,
                outcome: DebriefOutcome::Recovered,
                flags: 0,
                evidence_identity: 0x11b0_0200 + index as u32,
                primary_delta: index as i32,
            };
        }
        DebriefSummary {
            debrief_identity: 1,
            session_definition_identity: 2,
            action_transcript_identity: 3,
            completed_evidence_identity: 4,
            scenario_identity: 5,
            flags: DEBRIEF_FLAG_MASK,
            outcome: DebriefOutcome::Recovered,
            procedure_state: DebriefProcedureState::Completed,
            direct_observation_count: 10,
            procedure_completed_steps: 7,
            procedure_skipped_steps: 0,
            procedure_failed_steps: 0,
            procedure_mistimed_steps: 0,
            manual_override_count: 0,
            hint_count: 0,
            rejected_action_count: 0,
            prediction_apogee_residual_q12: 1,
            prediction_time_residual_q16: 2,
            prediction_impact_residual_q12: 3,
            evidence_checksums: [1, 2, 3, 4, 5, 6],
            counterfactuals,
            unresolved_hypothesis_mask: 1,
            causal_claim_mask: 2,
            model_explanation_mask: 4,
            observation_identity: 6,
            procedure_identity: 7,
            prediction_identity: 8,
            journal_identity: 9,
        }
    }

    #[test]
    fn kdr11_round_trips_and_rejects_reserved_or_corrupt_data() {
        let value = summary();
        let mut bytes = [0; KDR11_LENGTH];
        write_kdr11(&value, &mut bytes).unwrap();
        assert_eq!(parse_kdr11(&bytes).unwrap(), value);
        bytes[300] = 1;
        let crc = crc32_ieee(&bytes[..KDR11_CRC_OFFSET]);
        p32(&mut bytes, KDR11_CRC_OFFSET, crc);
        assert_eq!(parse_kdr11(&bytes), Err(CodecError::Reserved));
        write_kdr11(&value, &mut bytes).unwrap();
        bytes[80] ^= 1;
        assert_eq!(parse_kdr11(&bytes), Err(CodecError::Checksum));
    }
}
