//! Host-owned Phase 11 derived debrief reports.

pub use ksa64_session::phase11_debrief::*;

use ksa64_interface::phase11::CounterfactualFactor;
use serde_json::{json, Value};

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

fn escape_script_json(value: &str) -> String {
    value.replace('<', "\\u003c").replace('>', "\\u003e")
}

#[cfg(test)]
mod tests {
    use super::*;

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
