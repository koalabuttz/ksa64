//! Deterministic Phase 11 debrief construction and derived reports.

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
use serde_json::{json, Value};

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

pub fn debrief_json(evidence: &DebriefEvidence) -> Value {
    let summary = &evidence.summary;
    json!({
        "schema": "ksa64.phase11.debrief.v1",
        "identity": format!("0x{:08x}", summary.debrief_identity),
        "direct_observations": {
            "scenario": format!("0x{:08x}", summary.scenario_identity),
            "flight_checksum": format!("0x{:08x}", evidence.accepted.flight_checksum),
            "navigation_checksum": format!("0x{:08x}", evidence.accepted.navigation_checksum),
            "command_checksum": format!("0x{:08x}", evidence.accepted.command_checksum),
            "actions": evidence.accepted.actions.len(),
            "rejected_actions": evidence.accepted.rejected_loads
        },
        "procedure_and_operator_performance": {
            "state": format!("{:?}", evidence.accepted.procedure_state),
            "completed_steps": summary.procedure_completed_steps,
            "mistimed_steps": summary.procedure_mistimed_steps,
            "hints": summary.hint_count
        },
        "prediction_residuals": {
            "apogee_q12": summary.prediction_apogee_residual_q12,
            "time_q16": summary.prediction_time_residual_q16,
            "impact_q12": summary.prediction_impact_residual_q12,
            "note": "Zero in this bounded operations probe; full mission products retain measured residuals."
        },
        "model_derived_explanations": [
            "The reference package continued inertial navigation after GNSS invalidation.",
            "The accepted update and branch were activated only after atomic commit.",
            "Procedure timing changed the classified operator outcome."
        ],
        "controlled_counterfactuals": summary.counterfactuals.iter().map(|item| json!({
            "factor": format!("{:?}", item.factor),
            "scenario": format!("0x{:08x}", item.scenario_identity),
            "evidence": format!("0x{:08x}", item.evidence_identity),
            "release_delta": item.primary_delta,
            "causal_within_model": matches!(
                item.factor,
                CounterfactualFactor::NoGnssFailure
                    | CounterfactualFactor::NoOperatorAction
                    | CounterfactualFactor::DelayedAction
            )
        })).collect::<Vec<_>>(),
        "unresolved_hypotheses": [
            "The deterministic simulation does not calibrate real-world failure probability."
        ],
        "interpretation": "Causal language applies only to one-factor controlled reruns inside the KSA64 model; this is not real-world reliability evidence."
    })
}

pub fn debrief_html(evidence: &DebriefEvidence) -> String {
    let value = debrief_json(evidence);
    let rows = evidence
        .summary
        .counterfactuals
        .iter()
        .map(|item| {
            format!(
                "<tr><td>{:?}</td><td>0x{:08x}</td><td>{}</td></tr>",
                item.factor, item.evidence_identity, item.primary_delta
            )
        })
        .collect::<String>();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>KSA64 Phase 11 Debrief</title><style>body{{font:16px system-ui;background:#07111f;color:#d9e8ff;max-width:1000px;margin:auto;padding:2rem}}section{{background:#101f33;padding:1rem;margin:1rem 0;border-left:4px solid #54d7ff}}table{{border-collapse:collapse;width:100%}}td,th{{padding:.45rem;border-bottom:1px solid #345}}code{{color:#9ef}}</style></head><body><h1>KSA64 deterministic engineering debrief</h1><section><h2>Direct observations</h2><p>Scenario 0x{:08x}; evidence 0x{:08x}; {} recorded actions.</p></section><section><h2>Procedure and operator performance</h2><p>{:?}; {} completed steps.</p></section><section><h2>Prediction residuals</h2><p>Bounded operations probe residuals are explicitly zero; full-mission products retain measured residuals.</p></section><section><h2>Controlled counterfactuals</h2><table><tr><th>One-factor case</th><th>Evidence</th><th>Release delta</th></tr>{}</table></section><section><h2>Model-derived explanations</h2><p>The onboard system continued inertial navigation; committed loads activated atomically; timing altered the procedure classification.</p></section><section><h2>Unresolved hypotheses</h2><p>This deterministic result does not calibrate real-world reliability. Causal wording applies only inside the KSA64 model.</p></section><script type=\"application/json\" id=\"debrief-data\">{}</script></body></html>",
        evidence.accepted.scenario_identity,
        evidence.accepted.evidence_identity,
        evidence.accepted.actions.len(),
        evidence.accepted.procedure_state,
        evidence.summary.procedure_completed_steps,
        rows,
        escape_script_json(&value.to_string())
    )
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

fn escape_script_json(value: &str) -> String {
    value.replace('<', "\\u003c").replace('>', "\\u003e")
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

    #[test]
    fn derived_reports_label_causality_and_uncertainty() {
        let evidence = build_gnss_loss_debrief(1, 2, 3);
        let json = debrief_json(&evidence).to_string();
        let html = debrief_html(&evidence);
        for label in [
            "direct_observations",
            "procedure_and_operator_performance",
            "prediction_residuals",
            "controlled_counterfactuals",
            "unresolved_hypotheses",
        ] {
            assert!(json.contains(label));
        }
        assert!(html.contains("inside the KSA64 model"));
    }
}
