//! Phase 12B live-operations presentation contracts.
//!
//! These types deliberately sit outside the canonical simulation formats. They
//! describe what an operational client may display and how a completed mission
//! is classified without changing Phase 10 physics or Phase 11 evidence.

use crate::phase11_operations::{
    METRIC_GNSS_VALID, METRIC_INERTIAL_HEALTHY, METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
    PHASE11_OPERATIONAL_METRIC_CATALOG_ID,
};
use ksa64_flight::phase11::ksa_g10r_reference_mission_plan;
use ksa64_interface::phase11::{
    ContingencyBranch, MissionPlan, OperatorDecisionPoint, ProcedureComparison, ProcedurePack,
    ProcedurePredicate, ProcedureStep, ProcedureStepKind, UplinkLoadType, UplinkReasonCode,
    UplinkState, KPC11_MAX_STEPS, PACKAGE_CAP_BRANCH_SELECT,
};

pub const FULL_GNSS_LOSS_SCENARIO_ID: u32 = 0x12b0_0001;
pub const FULL_GNSS_LOSS_DEFINITION_ID: u32 = 0x12b0_1001;
pub const PRESENTATION_MODEL_ID: u32 = 0x12b0_2001;

pub const GNSS_LOSS_RELEASE: u32 = 5_760;
pub const GNSS_LOSS_TIME_Q16: u32 = 180 * 65_536;
pub const GNSS_QUALIFIED_RELEASE: u32 = 5_824;
pub const DECISION_WINDOW_OPEN_RELEASE: u32 = 5_920;
pub const DECISION_WINDOW_CLOSE_RELEASE: u32 = 7_840;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MissionObjectiveDisposition {
    PrimaryAchieved = 1,
    AlternateAchieved = 2,
    ContingencyAchieved = 3,
    NotAchieved = 4,
    Indeterminate = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VehicleDisposition {
    Nominal = 1,
    Degraded = 2,
    Recovered = 3,
    SafeState = 4,
    Lost = 5,
    Unknown = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcedureDisposition {
    Completed = 1,
    AlternateBranch = 2,
    Skipped = 3,
    Mistimed = 4,
    Overridden = 5,
    Failed = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OperatorDisposition {
    TimelyReference = 1,
    TimelyAlternate = 2,
    DelayedValid = 3,
    NoAction = 4,
    RejectedAction = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AvionicsDisposition {
    Nominal = 1,
    DegradedOperational = 2,
    SafeRecovery = 3,
    Failed = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EvidenceDisposition {
    Complete = 1,
    ObservationIncomplete = 2,
    Aborted = 3,
    Invalid = 4,
    Unavailable = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OverallMissionDisposition {
    NominalSuccess = 1,
    DegradedSuccess = 2,
    ContingencySuccess = 3,
    MissionFailure = 4,
    Indeterminate = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationalDispositionEvidence {
    pub objective: MissionObjectiveDisposition,
    pub vehicle: VehicleDisposition,
    pub procedure: ProcedureDisposition,
    pub operator: OperatorDisposition,
    pub avionics: AvionicsDisposition,
    pub evidence: EvidenceDisposition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationalDispositionView {
    pub overall: OverallMissionDisposition,
    pub axes: OperationalDispositionEvidence,
}

pub fn classify_disposition(axes: OperationalDispositionEvidence) -> OperationalDispositionView {
    let overall = if axes.evidence != EvidenceDisposition::Complete {
        OverallMissionDisposition::Indeterminate
    } else if matches!(
        axes.objective,
        MissionObjectiveDisposition::NotAchieved | MissionObjectiveDisposition::Indeterminate
    ) || matches!(
        axes.vehicle,
        VehicleDisposition::Lost | VehicleDisposition::Unknown
    ) || axes.avionics == AvionicsDisposition::Failed
    {
        OverallMissionDisposition::MissionFailure
    } else if matches!(
        axes.objective,
        MissionObjectiveDisposition::AlternateAchieved
            | MissionObjectiveDisposition::ContingencyAchieved
    ) || matches!(
        axes.vehicle,
        VehicleDisposition::Recovered | VehicleDisposition::SafeState
    ) || axes.procedure == ProcedureDisposition::AlternateBranch
        || axes.operator == OperatorDisposition::TimelyAlternate
        || axes.avionics == AvionicsDisposition::SafeRecovery
    {
        OverallMissionDisposition::ContingencySuccess
    } else if axes.vehicle == VehicleDisposition::Degraded
        || matches!(
            axes.procedure,
            ProcedureDisposition::Skipped
                | ProcedureDisposition::Mistimed
                | ProcedureDisposition::Overridden
                | ProcedureDisposition::Failed
        )
        || matches!(
            axes.operator,
            OperatorDisposition::DelayedValid
                | OperatorDisposition::NoAction
                | OperatorDisposition::RejectedAction
        )
        || axes.avionics == AvionicsDisposition::DegradedOperational
    {
        OverallMissionDisposition::DegradedSuccess
    } else {
        OverallMissionDisposition::NominalSuccess
    };
    OperationalDispositionView { overall, axes }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedurePredicateView {
    pub predicate_id: u16,
    pub satisfied: bool,
    pub valid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcedureView {
    pub procedure_identity: u32,
    pub active_step: u16,
    pub step_count: u16,
    pub entered_epoch: u32,
    pub deadline_epoch: u32,
    pub title: String,
    pub instruction: String,
    pub predicates: Vec<ProcedurePredicateView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionProposalView {
    pub proposal_identity: u32,
    pub load_type: UplinkLoadType,
    pub earliest_commit_epoch: u32,
    pub activation_epoch: u32,
    pub expires_epoch: u32,
    pub payload_checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionReceiptView {
    pub proposal_identity: u32,
    pub epoch: u32,
    pub state: UplinkState,
    pub reason: UplinkReasonCode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TimelineSource {
    World = 1,
    Avionics = 2,
    Ground = 3,
    Procedure = 4,
    Operator = 5,
    Evidence = 6,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineEventView {
    pub epoch: u32,
    pub source: TimelineSource,
    pub severity: u8,
    pub event_identity: u32,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ReleaseSampleView {
    pub epoch: u32,
    /// Public active-frame identity carried with the operational estimate.
    pub frame: u8,
    pub mission_time_q16: u32,
    pub altitude_q12_km: i32,
    pub speed_q24_km_s: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
    pub onboard_altitude_q12_km: i32,
    pub ground_altitude_q12_km: i32,
    pub flags: u32,
}

pub fn full_gnss_loss_mission_plan() -> MissionPlan {
    let mut plan = ksa_g10r_reference_mission_plan();
    plan.plan_identity = 0x12b3_0101;
    plan.branch_count = 2;
    plan.decision_count = 1;
    plan.branches[0] = ContingencyBranch {
        branch_id: 1,
        source_decision_id: 1,
        first_event_id: 1,
        flags: 0,
        earliest_epoch: DECISION_WINDOW_OPEN_RELEASE,
        latest_epoch: DECISION_WINDOW_CLOSE_RELEASE,
        prerequisite_events: 0,
        required_capabilities: PACKAGE_CAP_BRANCH_SELECT,
        arguments: [1, 0],
    };
    plan.branches[1] = ContingencyBranch {
        branch_id: 2,
        source_decision_id: 1,
        first_event_id: 1,
        flags: 0,
        earliest_epoch: DECISION_WINDOW_OPEN_RELEASE,
        latest_epoch: DECISION_WINDOW_CLOSE_RELEASE,
        prerequisite_events: 0,
        required_capabilities: PACKAGE_CAP_BRANCH_SELECT,
        arguments: [2, 0],
    };
    plan.decisions[0] = OperatorDecisionPoint {
        decision_id: 1,
        default_branch_id: 1,
        flags: 0,
        earliest_epoch: DECISION_WINDOW_OPEN_RELEASE,
        latest_epoch: DECISION_WINDOW_CLOSE_RELEASE,
        required_event_mask: 0,
        timeout_branch_id: 1,
    };
    plan
}

pub fn full_gnss_loss_procedure_pack(plan_identity: u32) -> ProcedurePack {
    let mut steps = [ProcedureStep::EMPTY; KPC11_MAX_STEPS];
    steps[0] = verification_step(1, 2, METRIC_GNSS_VALID, ProcedureComparison::Equal, 0);
    steps[1] = verification_step(2, 3, METRIC_INERTIAL_HEALTHY, ProcedureComparison::Equal, 1);
    steps[2] = verification_step(
        3,
        4,
        METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
        ProcedureComparison::LessOrEqual,
        262_144,
    );
    steps[3] = full_action_step(4, UplinkLoadType::GroundNavigationUpdate, 5);
    steps[4] = full_action_step(5, UplinkLoadType::ContingencyBranch, 6);
    steps[5] = ProcedureStep {
        step_id: 6,
        kind: ProcedureStepKind::Complete,
        message_identity: 0x12b3_0006,
        ..ProcedureStep::EMPTY
    };
    steps[6] = ProcedureStep {
        step_id: 7,
        kind: ProcedureStepKind::Fail,
        message_identity: 0x12b3_0007,
        ..ProcedureStep::EMPTY
    };
    ProcedurePack {
        pack_identity: 0x12b3_0001,
        plan_identity,
        procedure_identity: 0x12b3_1001,
        metric_catalog_identity: PHASE11_OPERATIONAL_METRIC_CATALOG_ID,
        name_identity: 0x12b3_2001,
        step_count: 7,
        entry_step: 1,
        flags: 0,
        steps,
    }
}

fn verification_step(
    id: u16,
    next_complete: u16,
    metric_id: u16,
    comparison: ProcedureComparison,
    threshold: i32,
) -> ProcedureStep {
    ProcedureStep {
        step_id: id,
        kind: ProcedureStepKind::Verify,
        next_complete,
        next_failure: 7,
        timeout_epochs: DECISION_WINDOW_CLOSE_RELEASE - GNSS_QUALIFIED_RELEASE,
        predicate_count: 1,
        hint_identity: id,
        message_identity: 0x12b3_0000 | u32::from(id),
        predicates: [
            ProcedurePredicate {
                metric_id,
                comparison,
                flags: 0,
                threshold,
            },
            ProcedurePredicate::EMPTY,
            ProcedurePredicate::EMPTY,
            ProcedurePredicate::EMPTY,
        ],
        ..ProcedureStep::EMPTY
    }
}

fn full_action_step(id: u16, action: UplinkLoadType, next_complete: u16) -> ProcedureStep {
    ProcedureStep {
        step_id: id,
        kind: ProcedureStepKind::RequestAction,
        next_complete,
        next_failure: 7,
        timeout_epochs: DECISION_WINDOW_CLOSE_RELEASE - GNSS_QUALIFIED_RELEASE,
        action: Some(action),
        hint_identity: id,
        message_identity: 0x12b3_0000 | u32::from(id),
        ..ProcedureStep::EMPTY
    }
}

pub const fn procedure_copy(step: u16) -> (&'static str, &'static str) {
    match step {
        1 => (
            "Confirm GNSS loss",
            "Verify three consecutive one-hertz GNSS fixes are missing.",
        ),
        2 => (
            "Verify inertial propagation",
            "Confirm inertial navigation remains healthy and is propagating.",
        ),
        3 => (
            "Compare ground solution",
            "Confirm onboard-to-ground position residual is within the approved correction limit.",
        ),
        4 => (
            "Load ground state update",
            "Review the ground navigation update, then stage and separately commit it.",
        ),
        5 => (
            "Select continuation branch",
            "Review the contingency branch, then stage and separately commit it.",
        ),
        6 => (
            "Procedure complete",
            "Continue monitoring navigation residuals and recovery readiness.",
        ),
        7 => (
            "Procedure window missed",
            "The response was not completed in the preferred window; continue mission assessment.",
        ),
        _ => ("Stand by", "No active GNSS-loss procedure step."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn successful_axes() -> OperationalDispositionEvidence {
        OperationalDispositionEvidence {
            objective: MissionObjectiveDisposition::PrimaryAchieved,
            vehicle: VehicleDisposition::Nominal,
            procedure: ProcedureDisposition::Completed,
            operator: OperatorDisposition::TimelyReference,
            avionics: AvionicsDisposition::Nominal,
            evidence: EvidenceDisposition::Complete,
        }
    }

    #[test]
    fn no_action_can_still_be_a_degraded_success() {
        let mut axes = successful_axes();
        axes.procedure = ProcedureDisposition::Skipped;
        axes.operator = OperatorDisposition::NoAction;
        axes.avionics = AvionicsDisposition::DegradedOperational;
        assert_eq!(
            classify_disposition(axes).overall,
            OverallMissionDisposition::DegradedSuccess
        );
    }

    #[test]
    fn effective_contingency_is_not_called_failure() {
        let mut axes = successful_axes();
        axes.objective = MissionObjectiveDisposition::ContingencyAchieved;
        axes.vehicle = VehicleDisposition::Recovered;
        axes.procedure = ProcedureDisposition::AlternateBranch;
        axes.operator = OperatorDisposition::DelayedValid;
        axes.avionics = AvionicsDisposition::DegradedOperational;
        assert_eq!(
            classify_disposition(axes).overall,
            OverallMissionDisposition::ContingencySuccess
        );
    }

    #[test]
    fn incomplete_evidence_is_indeterminate() {
        let mut axes = successful_axes();
        axes.evidence = EvidenceDisposition::ObservationIncomplete;
        assert_eq!(
            classify_disposition(axes).overall,
            OverallMissionDisposition::Indeterminate
        );
    }
}
