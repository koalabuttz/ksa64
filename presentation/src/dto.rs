use alloc::{string::String, vec::Vec};

use ksa64_interface::phase11::OperationalRole;

pub const PRESENTATION_MODEL_ID: u32 = 0x12b5_0001;

pub const SNAPSHOT_VALID_MISSION_TIME: u64 = 1 << 0;
pub const SNAPSHOT_VALID_NAVIGATION: u64 = 1 << 1;
pub const SNAPSHOT_VALID_GROUND_ESTIMATE: u64 = 1 << 2;
pub const SNAPSHOT_VALID_PREDICTION: u64 = 1 << 3;
pub const SNAPSHOT_VALID_PROCEDURE: u64 = 1 << 4;
pub const SNAPSHOT_VALID_ACTION: u64 = 1 << 5;
pub const SNAPSHOT_VALID_DISPOSITION: u64 = 1 << 6;
pub const SNAPSHOT_VALID_EVIDENCE: u64 = 1 << 7;
pub const SNAPSHOT_VALID_GNSS: u64 = 1 << 8;
pub const SNAPSHOT_VALID_TRUTH: u64 = 1 << 63;
pub const SNAPSHOT_VALID_PUBLIC_MASK: u64 = (1 << 9) - 1;
pub const SNAPSHOT_VALID_MASK: u64 = SNAPSHOT_VALID_PUBLIC_MASK | SNAPSHOT_VALID_TRUTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentationRole {
    Observer = 1,
    GuidedOperator = 2,
    FlightController = 3,
    FlightSoftwareEngineer = 4,
    SimDirector = 5,
    ScriptedOperator = 6,
}

impl PresentationRole {
    pub const fn permits_private_truth(self) -> bool {
        matches!(self, Self::SimDirector)
    }

    pub const fn permits_operator_actions(self) -> bool {
        matches!(
            self,
            Self::GuidedOperator
                | Self::FlightController
                | Self::SimDirector
                | Self::ScriptedOperator
        )
    }

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Observer),
            2 => Some(Self::GuidedOperator),
            3 => Some(Self::FlightController),
            4 => Some(Self::FlightSoftwareEngineer),
            5 => Some(Self::SimDirector),
            6 => Some(Self::ScriptedOperator),
            _ => None,
        }
    }
}

impl From<OperationalRole> for PresentationRole {
    fn from(value: OperationalRole) -> Self {
        match value {
            OperationalRole::Observer => Self::Observer,
            OperationalRole::GuidedOperator => Self::GuidedOperator,
            OperationalRole::FlightController => Self::FlightController,
            OperationalRole::FlightSoftwareEngineer => Self::FlightSoftwareEngineer,
            OperationalRole::SimDirector => Self::SimDirector,
            OperationalRole::ScriptedOperator => Self::ScriptedOperator,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentationLifecycle {
    Compiled = 1,
    Ready = 2,
    Running = 3,
    Paused = 4,
    Completed = 5,
    Aborted = 6,
    Incomplete = 7,
}

impl PresentationLifecycle {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Compiled),
            2 => Some(Self::Ready),
            3 => Some(Self::Running),
            4 => Some(Self::Paused),
            5 => Some(Self::Completed),
            6 => Some(Self::Aborted),
            7 => Some(Self::Incomplete),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentationPace {
    Fast = 1,
    Realtime = 2,
    Paused = 3,
    SingleStep = 4,
}

impl PresentationPace {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Fast),
            2 => Some(Self::Realtime),
            3 => Some(Self::Paused),
            4 => Some(Self::SingleStep),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigationView {
    pub position_q12_km: [i32; 3],
    pub velocity_q24_km_s: [i32; 3],
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PredictionSummaryView {
    pub prediction_identity: u32,
    pub prediction_checksum: u32,
    pub source_estimate_identity: u32,
    pub frame_identity: u32,
    pub apogee_q12_km: i32,
    pub perigee_q12_km: i32,
    pub time_to_apogee_q16: u32,
    pub time_to_impact_q16: u32,
    pub impact_position_q12_km: [i32; 3],
    pub terminal_reason: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimTruthView {
    pub position_q12_km: [i32; 3],
    pub velocity_q24_km_s: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub physical_checksum: u32,
    pub injected_fault_mask: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalSnapshot {
    pub presentation_model_identity: u32,
    pub session_definition_identity: u32,
    pub publication_sequence: u64,
    pub validity_mask: u64,
    pub role: PresentationRole,
    pub lifecycle: PresentationLifecycle,
    pub pace: PresentationPace,
    pub release_epoch: u32,
    pub release_period_micros: u32,
    pub frame_identity: u32,
    pub mission_time_q16: u32,
    pub onboard: NavigationView,
    pub ground: NavigationView,
    pub prediction: PredictionSummaryView,
    pub flight_checksum: u32,
    pub command_checksum: u32,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub staged_load_identity: u32,
    pub action_count: u32,
    pub rejected_loads: u16,
    pub gnss_state: u8,
    pub safe: bool,
    pub truth: Option<SimTruthView>,
}

impl OperationalSnapshot {
    pub fn filter_for_role(mut self, role: PresentationRole) -> Self {
        self.role = role;
        if !role.permits_private_truth() {
            self.truth = None;
            self.validity_mask &= !SNAPSHOT_VALID_TRUTH;
        } else if self.truth.is_some() {
            self.validity_mask |= SNAPSHOT_VALID_TRUTH;
        }
        self.validity_mask &= SNAPSHOT_VALID_MASK;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcedureStepState {
    Pending = 1,
    Active = 2,
    Completed = 3,
    Skipped = 4,
    Failed = 5,
    Mistimed = 6,
    Overridden = 7,
}

impl ProcedureStepState {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Pending),
            2 => Some(Self::Active),
            3 => Some(Self::Completed),
            4 => Some(Self::Skipped),
            5 => Some(Self::Failed),
            6 => Some(Self::Mistimed),
            7 => Some(Self::Overridden),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcedurePredicateView {
    pub identity: u32,
    pub satisfied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcedureView {
    pub procedure_identity: u32,
    pub active_step: u16,
    pub step_count: u16,
    pub state: ProcedureStepState,
    pub entered_epoch: u32,
    pub deadline_epoch: u32,
    pub title: String,
    pub instruction: String,
    pub predicates: Vec<ProcedurePredicateView>,
    pub hints_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OverallDisposition {
    NominalSuccess = 1,
    DegradedSuccess = 2,
    ContingencySuccess = 3,
    MissionFailure = 4,
    Indeterminate = 5,
}

impl OverallDisposition {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::NominalSuccess),
            2 => Some(Self::DegradedSuccess),
            3 => Some(Self::ContingencySuccess),
            4 => Some(Self::MissionFailure),
            5 => Some(Self::Indeterminate),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DispositionAxes {
    pub objective: u8,
    pub vehicle: u8,
    pub procedure: u8,
    pub operator: u8,
    pub avionics: u8,
    pub evidence: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispositionView {
    pub overall: OverallDisposition,
    pub axes: DispositionAxes,
    pub reason_identity: u32,
}

pub const ACTION_PERMIT_REVIEW: u8 = 1 << 0;
pub const ACTION_PERMIT_STAGE: u8 = 1 << 1;
pub const ACTION_PERMIT_COMMIT: u8 = 1 << 2;
pub const ACTION_PERMIT_CANCEL: u8 = 1 << 3;
pub const ACTION_PERMIT_MASK: u8 =
    ACTION_PERMIT_REVIEW | ACTION_PERMIT_STAGE | ACTION_PERMIT_COMMIT | ACTION_PERMIT_CANCEL;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionProposalView {
    pub proposal_identity: u32,
    pub load_identity: u32,
    pub load_type: u8,
    pub permitted_operations: u8,
    pub stage_epoch: u32,
    pub earliest_commit_epoch: u32,
    pub activation_epoch: u32,
    pub expires_epoch: u32,
    pub payload_checksum: u32,
    pub completed_event_mask: u32,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentationActionOperation {
    Review = 1,
    Stage = 2,
    Commit = 3,
    Cancel = 4,
}

impl PresentationActionOperation {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Review),
            2 => Some(Self::Stage),
            3 => Some(Self::Commit),
            4 => Some(Self::Cancel),
            _ => None,
        }
    }

    pub const fn permission_bit(self) -> u8 {
        match self {
            Self::Review => ACTION_PERMIT_REVIEW,
            Self::Stage => ACTION_PERMIT_STAGE,
            Self::Commit => ACTION_PERMIT_COMMIT,
            Self::Cancel => ACTION_PERMIT_CANCEL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationActionIntent {
    pub proposal_identity: u32,
    pub expected_load_identity: u32,
    pub operation: PresentationActionOperation,
    pub requested_activation_epoch: u32,
    pub client_action_sequence: u64,
}

impl PresentationActionIntent {
    pub const fn validate(self, role: PresentationRole) -> Result<(), PresentationValueError> {
        if self.proposal_identity == 0 || self.client_action_sequence == 0 {
            return Err(PresentationValueError::Identity);
        }
        if !role.permits_operator_actions() {
            return Err(PresentationValueError::Role);
        }
        if !matches!(self.operation, PresentationActionOperation::Review)
            && self.expected_load_identity == 0
        {
            return Err(PresentationValueError::Identity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionReceiptView {
    pub publication_sequence: u64,
    pub proposal_identity: u32,
    pub load_identity: u32,
    pub control_identity: u32,
    pub receipt_epoch: u32,
    pub effective_epoch: u32,
    pub state: u8,
    pub reason: u8,
    pub accepted: bool,
    pub operation: PresentationActionOperation,
    pub receipt_checksum: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentationEventView {
    pub sequence: u64,
    pub release_epoch: u32,
    pub kind: u16,
    pub detail_identity: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TimelineSeverity {
    Information = 1,
    Caution = 2,
    Warning = 3,
    Critical = 4,
}

impl TimelineSeverity {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Information),
            2 => Some(Self::Caution),
            3 => Some(Self::Warning),
            4 => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineEventView {
    pub sequence: u64,
    pub release_epoch: u32,
    pub source_identity: u32,
    pub severity: TimelineSeverity,
    pub event_identity: u32,
    pub detail_identity: u32,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReleaseSampleView {
    pub sequence: u64,
    pub validity_mask: u64,
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub frame_identity: u32,
    pub onboard_position_q12_km: [i32; 3],
    pub onboard_velocity_q24_km_s: [i32; 3],
    pub ground_position_q12_km: [i32; 3],
    pub ground_velocity_q24_km_s: [i32; 3],
    pub predicted_impact_q12_km: [i32; 3],
    pub predicted_apogee_q12_km: i32,
    pub altitude_q12_km: i32,
    pub speed_q24_km_s: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PredictionPathPoint {
    pub release_epoch: u32,
    pub frame_identity: u32,
    pub position_q12_km: [i32; 3],
    pub altitude_q12_km: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredictionPathView {
    pub path_identity: u32,
    pub product_identity: u32,
    pub model_identity: u32,
    pub source_estimate_identity: u32,
    pub source_estimate_checksum: u32,
    pub source_epoch: u32,
    pub generation_epoch: u32,
    pub frame_identity: u32,
    pub terminal_reason: u8,
    pub cadence_releases: u32,
    pub path_checksum: u32,
    pub points: Vec<PredictionPathPoint>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentationQueueStatus {
    pub command_capacity: u32,
    pub commands_pending: u32,
    pub event_capacity: u32,
    pub events_pending: u32,
    pub timeline_capacity: u32,
    pub timeline_pending: u32,
    pub sample_capacity: u32,
    pub samples_pending: u32,
    pub event_overflow: bool,
    pub timeline_overflow: bool,
    pub sample_overflow: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PresentationStaleness {
    Current = 1,
    Delayed = 2,
    Stale = 3,
    Disconnected = 4,
    Resynchronizing = 5,
}

impl PresentationStaleness {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Current),
            2 => Some(Self::Delayed),
            3 => Some(Self::Stale),
            4 => Some(Self::Disconnected),
            5 => Some(Self::Resynchronizing),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationHandshake {
    pub role: PresentationRole,
    pub client_instance: u64,
    pub capability_mask: u64,
    pub cursors: crate::PresentationCursors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleControl {
    pub requested: PresentationLifecycle,
    pub bounded_releases: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaceControl {
    pub requested: PresentationPace,
    pub bounded_releases: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportStatusView {
    pub staleness: PresentationStaleness,
    pub worker_state: u8,
    pub finalization_state: u8,
    pub queue: PresentationQueueStatus,
    pub last_command_result: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationErrorView {
    pub code: u16,
    pub fatal: bool,
    pub detail_identity: u32,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedEvidenceChunk {
    pub evidence_identity: u32,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub logical_offset: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationValueError {
    Identity,
    Role,
    Permission,
    Range,
}

pub trait PresentationSession {
    type Error;

    fn role(&self) -> PresentationRole;
    fn lifecycle(&self) -> PresentationLifecycle;
    fn latest_snapshot(&self) -> OperationalSnapshot;
    fn current_procedure(&self) -> Option<ProcedureView>;
    fn current_disposition(&self) -> Option<DispositionView>;
    fn current_prediction_paths(&self) -> Vec<PredictionPathView>;
    fn transport_status(&self) -> TransportStatusView;
    fn finalization_evidence(&self) -> Option<crate::SealedEvidenceMetadata>;
    fn cursors(&self) -> crate::PresentationCursors;
    fn read_events(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<crate::PresentationBatch<PresentationEventView>, crate::CursorError>;
    fn read_timeline(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<crate::PresentationBatch<TimelineEventView>, crate::CursorError>;
    fn read_action_receipts(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<crate::PresentationBatch<ActionReceiptView>, crate::CursorError>;
    fn read_release_samples(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<crate::PresentationBatch<ReleaseSampleView>, crate::CursorError>;
    fn submit_action(
        &mut self,
        intent: PresentationActionIntent,
    ) -> Result<ActionReceiptView, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> OperationalSnapshot {
        OperationalSnapshot {
            presentation_model_identity: PRESENTATION_MODEL_ID,
            session_definition_identity: 1,
            publication_sequence: 7,
            validity_mask: SNAPSHOT_VALID_PUBLIC_MASK | SNAPSHOT_VALID_TRUTH,
            role: PresentationRole::SimDirector,
            lifecycle: PresentationLifecycle::Running,
            pace: PresentationPace::Realtime,
            release_epoch: 100,
            release_period_micros: 31_250,
            frame_identity: 2,
            mission_time_q16: 204_800,
            onboard: NavigationView::default(),
            ground: NavigationView::default(),
            prediction: PredictionSummaryView::default(),
            flight_checksum: 1,
            command_checksum: 2,
            procedure_chain: 3,
            journal_chain: 4,
            action_chain: 5,
            staged_load_identity: 0,
            action_count: 0,
            rejected_loads: 0,
            gnss_state: 1,
            safe: false,
            truth: Some(SimTruthView::default()),
        }
    }

    #[test]
    fn private_truth_is_structurally_removed_for_every_non_director_role() {
        for role in [
            PresentationRole::Observer,
            PresentationRole::GuidedOperator,
            PresentationRole::FlightController,
            PresentationRole::FlightSoftwareEngineer,
            PresentationRole::ScriptedOperator,
        ] {
            let filtered = snapshot().filter_for_role(role);
            assert_eq!(filtered.role, role);
            assert_eq!(filtered.truth, None);
            assert_eq!(filtered.validity_mask & SNAPSHOT_VALID_TRUTH, 0);
        }
    }

    #[test]
    fn sim_director_may_receive_declared_truth() {
        let filtered = snapshot().filter_for_role(PresentationRole::SimDirector);
        assert!(filtered.truth.is_some());
        assert_ne!(filtered.validity_mask & SNAPSHOT_VALID_TRUTH, 0);
    }

    #[test]
    fn action_intents_require_role_identity_and_load_when_stateful() {
        let review = PresentationActionIntent {
            proposal_identity: 7,
            expected_load_identity: 0,
            operation: PresentationActionOperation::Review,
            requested_activation_epoch: 0,
            client_action_sequence: 1,
        };
        assert_eq!(
            review.validate(PresentationRole::Observer),
            Err(PresentationValueError::Role)
        );
        assert_eq!(review.validate(PresentationRole::GuidedOperator), Ok(()));

        let stage = PresentationActionIntent {
            operation: PresentationActionOperation::Stage,
            ..review
        };
        assert_eq!(
            stage.validate(PresentationRole::GuidedOperator),
            Err(PresentationValueError::Identity)
        );
    }
}
