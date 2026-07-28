//! Deterministic Phase 11 debrief construction and canonical encoding.

use crate::phase11_operations::ProcedureState;
use crate::phase11_scenarios::{
    run_gnss_loss_delayed_action_probe, run_gnss_loss_no_action_probe, run_gnss_loss_procedure,
    run_nominal_operations_probe, OperationalScenarioEvidence,
};
use ksa64_interface::phase11::{
    write_kdr11, CounterfactualDebrief, CounterfactualFactor, DebriefOutcome,
    DebriefProcedureState, DebriefSummary, DEBRIEF_FLAG_CONTROLLED_COUNTERFACTUALS,
    DEBRIEF_FLAG_DIRECT_OBSERVATIONS, DEBRIEF_FLAG_MODEL_EXPLANATIONS,
    DEBRIEF_FLAG_PREDICTION_RESIDUALS, DEBRIEF_FLAG_PROCEDURE_EVIDENCE,
    DEBRIEF_FLAG_UNRESOLVED_HYPOTHESES, KDR11_LENGTH,
};

pub const GNSS_LOSS_DEBRIEF_MODEL_ID: u32 = 0x11d0_0001;
pub const GNSS_LOSS_OBSERVATION_MODEL_ID: u32 = 0x11d0_0002;
pub const GNSS_LOSS_PREDICTION_MODEL_ID: u32 = 0x11d0_0003;
pub const GNSS_LOSS_JOURNAL_MODEL_ID: u32 = 0x11d0_0004;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebriefEvidence {
    pub accepted: OperationalScenarioEvidence,
    pub no_failure: OperationalScenarioEvidence,
    pub no_action: OperationalScenarioEvidence,
    pub delayed: OperationalScenarioEvidence,
    pub summary: DebriefSummary,
}

pub fn build_gnss_loss_debrief(
    definition_identity: u32,
    action_identity: u32,
    completed_identity: u32,
) -> DebriefEvidence {
    let accepted = run_gnss_loss_procedure(true);
    let no_failure = run_nominal_operations_probe();
    let no_action = run_gnss_loss_no_action_probe();
    let delayed = run_gnss_loss_delayed_action_probe();
    let factors = [
        (&no_failure, CounterfactualFactor::NoGnssFailure),
        (&no_action, CounterfactualFactor::NoOperatorAction),
        (&accepted, CounterfactualFactor::AcceptedUpdateAndBranch),
        (&delayed, CounterfactualFactor::DelayedAction),
    ];
    let mut counterfactuals = [CounterfactualDebrief::EMPTY; 4];
    for (index, (evidence, factor)) in factors.into_iter().enumerate() {
        counterfactuals[index] = CounterfactualDebrief {
            scenario_identity: evidence.scenario_identity,
            factor,
            outcome: outcome(evidence),
            flags: 0,
            evidence_identity: evidence.evidence_identity,
            primary_delta: signed_delta(evidence.releases, accepted.releases),
        };
    }
    let evidence_checksums = [
        accepted.flight_checksum,
        accepted.navigation_checksum,
        accepted.command_checksum,
        accepted.prediction_checksum,
        accepted.procedure_chain,
        accepted.journal_chain,
    ];
    let debrief_identity = hash_words(&[
        definition_identity,
        action_identity,
        completed_identity,
        accepted.evidence_identity,
        no_failure.evidence_identity,
        no_action.evidence_identity,
        delayed.evidence_identity,
        GNSS_LOSS_DEBRIEF_MODEL_ID,
    ]);
    let summary = DebriefSummary {
        debrief_identity,
        session_definition_identity: definition_identity,
        action_transcript_identity: action_identity,
        completed_evidence_identity: completed_identity,
        scenario_identity: accepted.scenario_identity,
        flags: DEBRIEF_FLAG_DIRECT_OBSERVATIONS
            | DEBRIEF_FLAG_PROCEDURE_EVIDENCE
            | DEBRIEF_FLAG_PREDICTION_RESIDUALS
            | DEBRIEF_FLAG_MODEL_EXPLANATIONS
            | DEBRIEF_FLAG_CONTROLLED_COUNTERFACTUALS
            | DEBRIEF_FLAG_UNRESOLVED_HYPOTHESES,
        outcome: outcome(&accepted),
        procedure_state: procedure_state(accepted.procedure_state),
        direct_observation_count: 6,
        procedure_completed_steps: u16::from(accepted.procedure_state == ProcedureState::Completed)
            * 6,
        procedure_skipped_steps: 0,
        procedure_failed_steps: 0,
        procedure_mistimed_steps: 0,
        manual_override_count: 0,
        hint_count: 0,
        rejected_action_count: accepted.rejected_loads,
        prediction_apogee_residual_q12: 0,
        prediction_time_residual_q16: 0,
        prediction_impact_residual_q12: 0,
        evidence_checksums,
        counterfactuals,
        unresolved_hypothesis_mask: 1,
        causal_claim_mask: 0b1011,
        model_explanation_mask: 0b111,
        observation_identity: GNSS_LOSS_OBSERVATION_MODEL_ID,
        procedure_identity: accepted.procedure_chain.max(1),
        prediction_identity: GNSS_LOSS_PREDICTION_MODEL_ID,
        journal_identity: GNSS_LOSS_JOURNAL_MODEL_ID,
    };
    DebriefEvidence {
        accepted,
        no_failure,
        no_action,
        delayed,
        summary,
    }
}

pub fn encode_debrief(summary: &DebriefSummary) -> [u8; KDR11_LENGTH] {
    let mut bytes = [0; KDR11_LENGTH];
    write_kdr11(summary, &mut bytes).expect("constructed debrief must encode");
    bytes
}

fn outcome(evidence: &OperationalScenarioEvidence) -> DebriefOutcome {
    if evidence.safe {
        DebriefOutcome::SafeState
    } else if evidence.rejected_loads != 0 {
        DebriefOutcome::Rejected
    } else if evidence.procedure_state == ProcedureState::Completed {
        DebriefOutcome::Recovered
    } else {
        DebriefOutcome::Incomplete
    }
}

fn procedure_state(state: ProcedureState) -> DebriefProcedureState {
    match state {
        ProcedureState::Active => DebriefProcedureState::ActiveAtTermination,
        ProcedureState::Completed => DebriefProcedureState::Completed,
        ProcedureState::Skipped => DebriefProcedureState::Skipped,
        ProcedureState::Failed => DebriefProcedureState::Failed,
        ProcedureState::Mistimed => DebriefProcedureState::Mistimed,
        ProcedureState::ManuallyOverridden => DebriefProcedureState::ManuallyOverridden,
    }
}

fn signed_delta(left: u32, right: u32) -> i32 {
    i64::from(left)
        .saturating_sub(i64::from(right))
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn hash_words(values: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in values {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_interface::phase11::parse_kdr11;

    #[test]
    fn gnss_debrief_separates_evidence_classes_and_is_strict() {
        let evidence = build_gnss_loss_debrief(1, 2, 3);
        let bytes = encode_debrief(&evidence.summary);
        assert_eq!(parse_kdr11(&bytes).unwrap(), evidence.summary);
        assert_eq!(evidence.no_action.procedure_state, ProcedureState::Failed);
        assert_eq!(evidence.delayed.procedure_state, ProcedureState::Failed);
        assert_ne!(
            evidence.no_action.evidence_identity,
            evidence.delayed.evidence_identity
        );
    }
}
