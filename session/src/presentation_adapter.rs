//! Role-filtered presentation adapter for the accepted full-duration mission session.
//!
//! This module is deliberately an adapter: the Phase 10 world, Phase 11 flight
//! package, operation ordering, and KSB11 encoder remain owned by
//! FullMissionSession. Polling and presentation cursors cannot advance or
//! otherwise mutate mission authority.

use crate::phase11_authoring::CompiledMissionProject;
use crate::phase11_live::{
    MissionActionReceipt, MissionOperatorAction, MissionSessionError, MissionSessionEventKind,
    MissionSessionLifecycle, MissionSessionPace, LIVE_RELEASE_PERIOD_MICROS,
};
use crate::phase11_operations::ProcedureState;
use crate::phase11_prediction::HostPrediction;
use crate::phase12b::{OverallMissionDisposition, GNSS_LOSS_RELEASE};
use crate::phase12b_live::{FullMissionCompletion, FullMissionSession};
use ksa64_core::scenario::crc32_ieee;
use ksa64_interface::phase11::OperationalRole;
use ksa64_presentation::{
    ActionProposalView, ActionReceiptView, CursorError, DispositionAxes, DispositionView,
    NavigationView, OperationalSnapshot, OverallDisposition, PredictionPathPoint,
    PredictionPathView, PredictionSummaryView, PresentationActionIntent,
    PresentationActionOperation, PresentationBatch, PresentationCursors, PresentationEventView,
    PresentationLifecycle, PresentationPace, PresentationQueueStatus, PresentationRole,
    PresentationSession, PresentationStaleness, PresentationValueError, ProcedurePredicateView,
    ProcedureStepState, ProcedureView, ReleaseSampleView, RetainedStream, SealedEvidenceMetadata,
    TimelineEventView, TimelineSeverity, TransportStatusView, ACTION_PERMIT_CANCEL,
    ACTION_PERMIT_COMMIT, ACTION_PERMIT_REVIEW, ACTION_PERMIT_STAGE,
    KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH, PRESENTATION_MODEL_ID, SNAPSHOT_VALID_ACTION,
    SNAPSHOT_VALID_DISPOSITION, SNAPSHOT_VALID_EVIDENCE, SNAPSHOT_VALID_GNSS,
    SNAPSHOT_VALID_GROUND_ESTIMATE, SNAPSHOT_VALID_MISSION_TIME, SNAPSHOT_VALID_NAVIGATION,
    SNAPSHOT_VALID_PREDICTION, SNAPSHOT_VALID_PROCEDURE,
};

const EVENT_RETENTION: usize = 32_768;
const TIMELINE_RETENTION: usize = 2_048;
const RECEIPT_RETENTION: usize = 128;
const SAMPLE_RETENTION: usize = 4_096;
const EVIDENCE_CONTENT_KSB11: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationSessionError {
    Authority(MissionSessionError),
    Intent(PresentationValueError),
    ActionSequence,
    Proposal,
    Retention(CursorError),
}

impl From<MissionSessionError> for PresentationSessionError {
    fn from(value: MissionSessionError) -> Self {
        Self::Authority(value)
    }
}

impl From<CursorError> for PresentationSessionError {
    fn from(value: CursorError) -> Self {
        Self::Retention(value)
    }
}

/// Portable presentation boundary over the accepted deterministic authority.
pub struct FullMissionPresentationSession {
    authority: FullMissionSession,
    role: PresentationRole,
    snapshot_sequence: u64,
    latest_snapshot: OperationalSnapshot,
    events: RetainedStream<PresentationEventView>,
    timeline: RetainedStream<TimelineEventView>,
    receipts: RetainedStream<ActionReceiptView>,
    samples: RetainedStream<ReleaseSampleView>,
    source_event_count: u32,
    source_timeline_count: u32,
    source_sample_count: u32,
    next_client_action_sequence: u64,
    reviewed_proposal: Option<u32>,
    active_proposal: Option<ActionProposalView>,
}

impl FullMissionPresentationSession {
    pub fn new(role: OperationalRole) -> Result<Self, PresentationSessionError> {
        let authority = FullMissionSession::new(role)?;
        Self::from_authority(authority, role)
    }

    /// Opens a compiled full-duration mission through the same presentation boundary.
    /// Runtime role policy does not alter the compiled definition identity.
    pub fn compiled(
        mut project: CompiledMissionProject,
        role: OperationalRole,
    ) -> Result<Self, PresentationSessionError> {
        project.role = role;
        let authority = FullMissionSession::compiled(project)?;
        Self::from_authority(authority, role)
    }

    fn from_authority(
        authority: FullMissionSession,
        role: OperationalRole,
    ) -> Result<Self, PresentationSessionError> {
        let presentation_role = PresentationRole::from(role);
        let mut value = Self {
            authority,
            role: presentation_role,
            snapshot_sequence: 0,
            latest_snapshot: empty_snapshot(presentation_role),
            events: RetainedStream::new(EVENT_RETENTION)?,
            timeline: RetainedStream::new(TIMELINE_RETENTION)?,
            receipts: RetainedStream::new(RECEIPT_RETENTION)?,
            samples: RetainedStream::new(SAMPLE_RETENTION)?,
            source_event_count: 0,
            source_timeline_count: 0,
            source_sample_count: 0,
            next_client_action_sequence: 1,
            reviewed_proposal: None,
            active_proposal: None,
        };
        value.refresh_publications()?;
        Ok(value)
    }

    pub fn prepare(&mut self) -> Result<(), PresentationSessionError> {
        self.authority.prepare()?;
        self.refresh_publications()
    }

    pub fn set_pace(&mut self, pace: PresentationPace) -> Result<(), PresentationSessionError> {
        self.authority.set_pace(authority_pace(pace))?;
        self.refresh_publications()
    }

    pub fn pause(&mut self) -> Result<(), PresentationSessionError> {
        self.authority.pause()?;
        self.refresh_publications()
    }

    pub fn resume(&mut self) -> Result<(), PresentationSessionError> {
        self.authority.resume()?;
        self.refresh_publications()
    }

    pub fn advance_one_release(&mut self) -> Result<(), PresentationSessionError> {
        self.authority.advance_one_release()?;
        self.refresh_publications()
    }

    pub fn step_one_release(&mut self) -> Result<(), PresentationSessionError> {
        self.authority.step_one_release()?;
        self.refresh_publications()
    }

    pub fn advance_bounded(
        &mut self,
        maximum_releases: u32,
    ) -> Result<u32, PresentationSessionError> {
        let before = self.authority.snapshot().release_epoch;
        for _ in 0..maximum_releases {
            self.authority.advance_bounded(1)?;
            if self.authority.recommended_load().is_some()
                || matches!(
                    self.authority.lifecycle(),
                    MissionSessionLifecycle::Completed
                        | MissionSessionLifecycle::Paused
                        | MissionSessionLifecycle::Aborted
                )
            {
                break;
            }
        }
        self.refresh_publications()?;
        Ok(self
            .authority
            .snapshot()
            .release_epoch
            .saturating_sub(before))
    }

    pub fn abort(&mut self, reason_identity: u32) -> Result<(), PresentationSessionError> {
        self.authority.abort(reason_identity)?;
        self.refresh_publications()
    }

    pub fn current_action_proposal(&self) -> Option<ActionProposalView> {
        if self.authority.staged_load_identity().is_some() {
            return self.active_proposal.clone().map(|mut proposal| {
                proposal.permitted_operations =
                    ACTION_PERMIT_REVIEW | ACTION_PERMIT_COMMIT | ACTION_PERMIT_CANCEL;
                proposal
            });
        }
        self.authority
            .recommended_load()
            .map(|load| proposal_from_load(load, self.authority.snapshot().release_epoch))
    }

    pub fn sealed_evidence_bytes(&self) -> Option<&[u8]> {
        self.authority
            .completed_session()
            .map(|completion| completion.session.bundle.as_slice())
    }

    pub fn finish(self) -> Result<FullMissionCompletion, PresentationSessionError> {
        self.authority.finish().map_err(Into::into)
    }

    pub fn authority(&self) -> &FullMissionSession {
        &self.authority
    }

    fn refresh_publications(&mut self) -> Result<(), PresentationSessionError> {
        for event in self.authority.events_after(self.source_event_count) {
            let sequence = self.events.next_cursor();
            self.events.push(PresentationEventView {
                sequence,
                release_epoch: event.release_epoch,
                kind: event_kind(event.kind),
                detail_identity: event.detail_identity,
            })?;
            self.source_event_count = self.source_event_count.saturating_add(1);
        }
        for event in self.authority.timeline_after(self.source_timeline_count) {
            let sequence = self.timeline.next_cursor();
            self.timeline.push(TimelineEventView {
                sequence,
                release_epoch: event.epoch,
                source_identity: event.source as u32,
                severity: timeline_severity(event.severity),
                event_identity: event.event_identity,
                detail_identity: 0,
                label: event.label.clone(),
            })?;
            self.source_timeline_count = self.source_timeline_count.saturating_add(1);
        }
        for sample in self
            .authority
            .release_samples_after(self.source_sample_count)
        {
            let sequence = self.samples.next_cursor();
            self.samples.push(ReleaseSampleView {
                sequence,
                validity_mask: u64::from(sample.flags),
                release_epoch: sample.epoch,
                mission_time_q16: sample.mission_time_q16,
                frame_identity: u32::from(sample.frame),
                onboard_position_q12_km: [
                    sample.downrange_q12_km,
                    sample.crossrange_q12_km,
                    sample.onboard_altitude_q12_km,
                ],
                onboard_velocity_q24_km_s: [0; 3],
                ground_position_q12_km: [
                    sample.downrange_q12_km,
                    sample.crossrange_q12_km,
                    sample.ground_altitude_q12_km,
                ],
                ground_velocity_q24_km_s: [0; 3],
                predicted_impact_q12_km: [0; 3],
                predicted_apogee_q12_km: 0,
                altitude_q12_km: sample.altitude_q12_km,
                speed_q24_km_s: sample.speed_q24_km_s,
                downrange_q12_km: sample.downrange_q12_km,
                crossrange_q12_km: sample.crossrange_q12_km,
            })?;
            self.source_sample_count = self.source_sample_count.saturating_add(1);
        }
        self.snapshot_sequence = self
            .snapshot_sequence
            .checked_add(1)
            .ok_or(PresentationSessionError::ActionSequence)?;
        self.latest_snapshot = self.map_snapshot(self.snapshot_sequence);
        Ok(())
    }

    fn map_snapshot(&self, publication_sequence: u64) -> OperationalSnapshot {
        let source = self.authority.snapshot();
        let mut validity_mask = SNAPSHOT_VALID_MISSION_TIME | SNAPSHOT_VALID_GNSS;
        let onboard = source
            .flight
            .map_or_else(NavigationView::default, |flight| {
                validity_mask |= SNAPSHOT_VALID_NAVIGATION;
                NavigationView {
                    position_q12_km: flight.navigation.position_q12,
                    velocity_q24_km_s: flight.navigation.velocity_q24,
                    checksum: flight.navigation.checksum,
                }
            });
        let ground = source
            .ground
            .map_or_else(NavigationView::default, |estimate| {
                validity_mask |= SNAPSHOT_VALID_GROUND_ESTIMATE;
                NavigationView {
                    position_q12_km: estimate.position_q12_km,
                    velocity_q24_km_s: estimate.velocity_q24_km_s,
                    checksum: estimate.estimator_checksum,
                }
            });
        let prediction = source
            .latest_ground_prediction
            .as_ref()
            .or(source.latest_onboard_prediction.as_ref())
            .map_or_else(PredictionSummaryView::default, |value| {
                validity_mask |= SNAPSHOT_VALID_PREDICTION;
                map_prediction_summary(value)
            });
        if source.procedure.is_some() {
            validity_mask |= SNAPSHOT_VALID_PROCEDURE;
        }
        if self.current_action_proposal().is_some() {
            validity_mask |= SNAPSHOT_VALID_ACTION;
        }
        if source.disposition.is_some() {
            validity_mask |= SNAPSHOT_VALID_DISPOSITION;
        }
        if self.authority.completed_session().is_some() {
            validity_mask |= SNAPSHOT_VALID_EVIDENCE;
        }
        let flight_checksum = source.flight.map_or(0, |value| value.flight_checksum);
        let command_checksum = source
            .flight
            .map_or(0, |value| value.command.command_checksum);
        OperationalSnapshot {
            presentation_model_identity: PRESENTATION_MODEL_ID,
            session_definition_identity: self.authority.definition_identity(),
            publication_sequence,
            validity_mask,
            role: self.role,
            lifecycle: presentation_lifecycle(source.lifecycle),
            pace: presentation_pace(source.pace),
            release_epoch: source.release_epoch,
            release_period_micros: LIVE_RELEASE_PERIOD_MICROS,
            frame_identity: source.frame.map_or(0, |frame| frame as u32),
            mission_time_q16: source.mission_time_q16,
            onboard,
            ground,
            prediction,
            flight_checksum,
            command_checksum,
            procedure_chain: self.authority.procedure_chain(),
            journal_chain: self.authority.journal_chain(),
            action_chain: self.authority.action_chain(),
            staged_load_identity: self.authority.staged_load_identity().unwrap_or(0),
            action_count: source.action_count,
            rejected_loads: source.rejected_loads,
            gnss_state: gnss_state(source.release_epoch),
            safe: source.flight.is_some_and(|value| value.safe),
            truth: None,
        }
        .filter_for_role(self.role)
    }

    fn map_procedure(&self) -> Option<ProcedureView> {
        let source = self.authority.snapshot().procedure?;
        Some(ProcedureView {
            procedure_identity: source.procedure_identity,
            active_step: source.active_step,
            step_count: source.step_count,
            state: procedure_state(self.authority.procedure_state()),
            entered_epoch: source.entered_epoch,
            deadline_epoch: source.deadline_epoch,
            title: source.title,
            instruction: source.instruction,
            predicates: source
                .predicates
                .into_iter()
                .filter(|predicate| predicate.valid)
                .map(|predicate| ProcedurePredicateView {
                    identity: u32::from(predicate.predicate_id),
                    satisfied: predicate.satisfied,
                })
                .collect(),
            hints_available: self.authority.hints_enabled(),
        })
    }

    fn map_disposition(&self) -> Option<DispositionView> {
        let value = self.authority.snapshot().disposition?;
        Some(DispositionView {
            overall: match value.overall {
                OverallMissionDisposition::NominalSuccess => OverallDisposition::NominalSuccess,
                OverallMissionDisposition::DegradedSuccess => OverallDisposition::DegradedSuccess,
                OverallMissionDisposition::ContingencySuccess => {
                    OverallDisposition::ContingencySuccess
                }
                OverallMissionDisposition::MissionFailure => OverallDisposition::MissionFailure,
                OverallMissionDisposition::Indeterminate => OverallDisposition::Indeterminate,
            },
            axes: DispositionAxes {
                objective: value.axes.objective as u8,
                vehicle: value.axes.vehicle as u8,
                procedure: value.axes.procedure as u8,
                operator: value.axes.operator as u8,
                avionics: value.axes.avionics as u8,
                evidence: value.axes.evidence as u8,
            },
            reason_identity: self.authority.definition_identity(),
        })
    }

    fn push_receipt(
        &mut self,
        mut receipt: ActionReceiptView,
    ) -> Result<ActionReceiptView, PresentationSessionError> {
        receipt.publication_sequence = self.receipts.next_cursor();
        self.receipts.push(receipt)?;
        Ok(receipt)
    }
}

impl PresentationSession for FullMissionPresentationSession {
    type Error = PresentationSessionError;

    fn role(&self) -> PresentationRole {
        self.role
    }

    fn lifecycle(&self) -> PresentationLifecycle {
        self.latest_snapshot.lifecycle
    }

    fn latest_snapshot(&self) -> OperationalSnapshot {
        self.latest_snapshot.clone()
    }

    fn current_procedure(&self) -> Option<ProcedureView> {
        self.map_procedure()
    }

    fn current_disposition(&self) -> Option<DispositionView> {
        self.map_disposition()
    }

    fn current_prediction_paths(&self) -> Vec<PredictionPathView> {
        let source = self.authority.snapshot();
        [
            source.latest_onboard_prediction.as_ref(),
            source.latest_ground_prediction.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(map_prediction_path)
        .collect()
    }

    fn transport_status(&self) -> TransportStatusView {
        TransportStatusView {
            staleness: PresentationStaleness::Current,
            worker_state: transport_worker_state(self.latest_snapshot.lifecycle),
            finalization_state: transport_finalization_state(
                self.latest_snapshot.lifecycle,
                self.authority.completed_session().is_some(),
            ),
            queue: PresentationQueueStatus {
                command_capacity: 1,
                commands_pending: u32::from(self.authority.staged_load_identity().is_some()),
                event_capacity: EVENT_RETENTION as u32,
                events_pending: self.events.len() as u32,
                timeline_capacity: TIMELINE_RETENTION as u32,
                timeline_pending: self.timeline.len() as u32,
                sample_capacity: SAMPLE_RETENTION as u32,
                samples_pending: self.samples.len() as u32,
                event_overflow: self.events.oldest_cursor() > 1,
                timeline_overflow: self.timeline.oldest_cursor() > 1,
                sample_overflow: self.samples.oldest_cursor() > 1,
            },
            last_command_result: 0,
        }
    }

    fn finalization_evidence(&self) -> Option<SealedEvidenceMetadata> {
        let completion = self.authority.completed_session()?;
        let bytes = completion.session.bundle.as_slice();
        let chunk_length = KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH as u32;
        Some(SealedEvidenceMetadata {
            evidence_identity: completion.session.evidence.evidence_identity,
            evidence_crc32: crc32_ieee(bytes),
            total_length: bytes.len() as u64,
            chunk_length,
            chunk_count: (bytes.len() as u64).div_ceil(u64::from(chunk_length)) as u32,
            complete: true,
            content_kind: EVIDENCE_CONTENT_KSB11,
        })
    }

    fn cursors(&self) -> PresentationCursors {
        PresentationCursors {
            snapshots: self.latest_snapshot.publication_sequence,
            events: self.events.oldest_cursor(),
            timeline: self.timeline.oldest_cursor(),
            action_receipts: self.receipts.oldest_cursor(),
            release_samples: self.samples.oldest_cursor(),
        }
    }

    fn read_events(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<PresentationEventView>, CursorError> {
        self.events.read(cursor, limit)
    }

    fn read_timeline(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<TimelineEventView>, CursorError> {
        self.timeline.read(cursor, limit)
    }

    fn read_action_receipts(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<ActionReceiptView>, CursorError> {
        self.receipts.read(cursor, limit)
    }

    fn read_release_samples(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<ReleaseSampleView>, CursorError> {
        self.samples.read(cursor, limit)
    }

    fn submit_action(
        &mut self,
        intent: PresentationActionIntent,
    ) -> Result<ActionReceiptView, Self::Error> {
        intent
            .validate(self.role)
            .map_err(PresentationSessionError::Intent)?;
        if intent.client_action_sequence != self.next_client_action_sequence {
            return Err(PresentationSessionError::ActionSequence);
        }
        let proposal = self
            .current_action_proposal()
            .ok_or(PresentationSessionError::Proposal)?;
        if intent.proposal_identity != proposal.proposal_identity
            || (intent.expected_load_identity != 0
                && intent.expected_load_identity != proposal.load_identity)
            || proposal.permitted_operations & intent.operation.permission_bit() == 0
            || (intent.requested_activation_epoch != 0
                && intent.requested_activation_epoch != proposal.activation_epoch)
        {
            return Err(PresentationSessionError::Proposal);
        }

        let receipt = match intent.operation {
            PresentationActionOperation::Review => {
                self.reviewed_proposal = Some(proposal.proposal_identity);
                self.active_proposal = Some(proposal.clone());
                ActionReceiptView {
                    publication_sequence: 0,
                    proposal_identity: proposal.proposal_identity,
                    load_identity: proposal.load_identity,
                    control_identity: 0,
                    receipt_epoch: self.authority.snapshot().release_epoch,
                    effective_epoch: proposal.activation_epoch,
                    state: 0,
                    reason: 0,
                    accepted: true,
                    operation: intent.operation,
                    receipt_checksum: review_checksum(
                        intent,
                        self.authority.snapshot().release_epoch,
                    ),
                }
            }
            PresentationActionOperation::Stage => {
                if self.reviewed_proposal != Some(proposal.proposal_identity) {
                    return Err(PresentationSessionError::Proposal);
                }
                let load = self
                    .authority
                    .recommended_load()
                    .filter(|load| load.load_identity == proposal.load_identity)
                    .ok_or(PresentationSessionError::Proposal)?;
                let authority_receipt =
                    self.authority
                        .submit_operator_action(MissionOperatorAction::Stage {
                            load,
                            completed_event_mask: proposal.completed_event_mask,
                        })?;
                self.active_proposal = Some(proposal.clone());
                map_action_receipt(
                    authority_receipt,
                    proposal.proposal_identity,
                    intent.operation,
                )
            }
            PresentationActionOperation::Commit => {
                let request = self
                    .authority
                    .commit_request_for_staged()
                    .filter(|request| request.load_identity == proposal.load_identity)
                    .ok_or(PresentationSessionError::Proposal)?;
                let authority_receipt = self
                    .authority
                    .submit_operator_action(MissionOperatorAction::Commit(request))?;
                self.active_proposal = None;
                self.reviewed_proposal = None;
                map_action_receipt(
                    authority_receipt,
                    proposal.proposal_identity,
                    intent.operation,
                )
            }
            PresentationActionOperation::Cancel => {
                let request = self
                    .authority
                    .cancel_request_for_staged()
                    .filter(|request| request.load_identity == proposal.load_identity)
                    .ok_or(PresentationSessionError::Proposal)?;
                let authority_receipt = self
                    .authority
                    .submit_operator_action(MissionOperatorAction::Cancel(request))?;
                self.active_proposal = None;
                self.reviewed_proposal = None;
                map_action_receipt(
                    authority_receipt,
                    proposal.proposal_identity,
                    intent.operation,
                )
            }
        };
        self.next_client_action_sequence = self
            .next_client_action_sequence
            .checked_add(1)
            .ok_or(PresentationSessionError::ActionSequence)?;
        let receipt = self.push_receipt(receipt)?;
        self.refresh_publications()?;
        Ok(receipt)
    }
}

fn empty_snapshot(role: PresentationRole) -> OperationalSnapshot {
    OperationalSnapshot {
        presentation_model_identity: PRESENTATION_MODEL_ID,
        session_definition_identity: 1,
        publication_sequence: 1,
        validity_mask: 0,
        role,
        lifecycle: PresentationLifecycle::Compiled,
        pace: PresentationPace::Realtime,
        release_epoch: 0,
        release_period_micros: LIVE_RELEASE_PERIOD_MICROS,
        frame_identity: 0,
        mission_time_q16: 0,
        onboard: NavigationView::default(),
        ground: NavigationView::default(),
        prediction: PredictionSummaryView::default(),
        flight_checksum: 0,
        command_checksum: 0,
        procedure_chain: 0,
        journal_chain: 0,
        action_chain: 0,
        staged_load_identity: 0,
        action_count: 0,
        rejected_loads: 0,
        gnss_state: 1,
        safe: false,
        truth: None,
    }
}

fn transport_worker_state(lifecycle: PresentationLifecycle) -> u8 {
    match lifecycle {
        PresentationLifecycle::Completed | PresentationLifecycle::Aborted => 2,
        PresentationLifecycle::Incomplete => 3,
        PresentationLifecycle::Compiled
        | PresentationLifecycle::Ready
        | PresentationLifecycle::Running
        | PresentationLifecycle::Paused => 1,
    }
}

fn transport_finalization_state(lifecycle: PresentationLifecycle, sealed: bool) -> u8 {
    if sealed {
        1
    } else if matches!(
        lifecycle,
        PresentationLifecycle::Aborted | PresentationLifecycle::Incomplete
    ) {
        2
    } else {
        0
    }
}

fn presentation_lifecycle(value: MissionSessionLifecycle) -> PresentationLifecycle {
    match value {
        MissionSessionLifecycle::Compiled => PresentationLifecycle::Compiled,
        MissionSessionLifecycle::Ready => PresentationLifecycle::Ready,
        MissionSessionLifecycle::Running => PresentationLifecycle::Running,
        MissionSessionLifecycle::Paused => PresentationLifecycle::Paused,
        MissionSessionLifecycle::Completed => PresentationLifecycle::Completed,
        MissionSessionLifecycle::Aborted => PresentationLifecycle::Aborted,
    }
}

fn presentation_pace(value: MissionSessionPace) -> PresentationPace {
    match value {
        MissionSessionPace::Fast => PresentationPace::Fast,
        MissionSessionPace::Realtime => PresentationPace::Realtime,
        MissionSessionPace::Paused => PresentationPace::Paused,
        MissionSessionPace::SingleStep => PresentationPace::SingleStep,
    }
}

fn authority_pace(value: PresentationPace) -> MissionSessionPace {
    match value {
        PresentationPace::Fast => MissionSessionPace::Fast,
        PresentationPace::Realtime => MissionSessionPace::Realtime,
        PresentationPace::Paused => MissionSessionPace::Paused,
        PresentationPace::SingleStep => MissionSessionPace::SingleStep,
    }
}

fn procedure_state(value: Option<ProcedureState>) -> ProcedureStepState {
    match value {
        None | Some(ProcedureState::Active) => ProcedureStepState::Active,
        Some(ProcedureState::Completed) => ProcedureStepState::Completed,
        Some(ProcedureState::Skipped) => ProcedureStepState::Skipped,
        Some(ProcedureState::Failed) => ProcedureStepState::Failed,
        Some(ProcedureState::Mistimed) => ProcedureStepState::Mistimed,
        Some(ProcedureState::ManuallyOverridden) => ProcedureStepState::Overridden,
    }
}

fn event_kind(value: MissionSessionEventKind) -> u16 {
    match value {
        MissionSessionEventKind::Compiled => 1,
        MissionSessionEventKind::Prepared => 2,
        MissionSessionEventKind::Release => 3,
        MissionSessionEventKind::Paused => 4,
        MissionSessionEventKind::Resumed => 5,
        MissionSessionEventKind::PaceChanged => 6,
        MissionSessionEventKind::ActionStaged => 7,
        MissionSessionEventKind::ActionCommitted => 8,
        MissionSessionEventKind::ActionCancelled => 9,
        MissionSessionEventKind::ActionRejected => 10,
        MissionSessionEventKind::Completed => 11,
        MissionSessionEventKind::Aborted => 12,
    }
}

fn timeline_severity(value: u8) -> TimelineSeverity {
    match value {
        0 => TimelineSeverity::Information,
        1 => TimelineSeverity::Caution,
        2 => TimelineSeverity::Warning,
        _ => TimelineSeverity::Critical,
    }
}

fn gnss_state(release_epoch: u32) -> u8 {
    if release_epoch < GNSS_LOSS_RELEASE {
        1
    } else {
        2
    }
}

fn proposal_from_load(
    load: ksa64_interface::phase11::UplinkCommandLoad,
    release_epoch: u32,
) -> ActionProposalView {
    ActionProposalView {
        proposal_identity: load.load_identity,
        load_identity: load.load_identity,
        load_type: load.load_type as u8,
        permitted_operations: ACTION_PERMIT_REVIEW | ACTION_PERMIT_STAGE,
        stage_epoch: release_epoch,
        earliest_commit_epoch: load.not_before_epoch,
        activation_epoch: load.requested_effective_epoch,
        expires_epoch: load.expires_epoch,
        payload_checksum: accepted_load_checksum(load),
        completed_event_mask: load.prerequisite_event_mask,
        label: action_label(load.load_type),
    }
}

fn accepted_load_checksum(load: ksa64_interface::phase11::UplinkCommandLoad) -> u32 {
    let mut words = Vec::with_capacity(29);
    words.extend_from_slice(&[
        load.load_identity,
        load.package_manifest_identity,
        load.plan_identity,
        load.stage_epoch,
        load.not_before_epoch,
        load.expires_epoch,
        load.requested_effective_epoch,
        load.required_capabilities,
        load.prerequisite_event_mask,
        load.position_residual_limit_q12 as u32,
        load.velocity_residual_limit_q24 as u32,
        load.source_estimator_identity,
        load.source_estimator_checksum,
    ]);
    words.extend(load.arguments.iter().map(|value| *value as u32));
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    crc32_ieee(&bytes)
}

fn action_label(load_type: ksa64_interface::phase11::UplinkLoadType) -> String {
    use ksa64_interface::phase11::UplinkLoadType;
    match load_type {
        UplinkLoadType::GroundNavigationUpdate => "Ground navigation update",
        UplinkLoadType::MissionEventTarget => "Mission event target",
        UplinkLoadType::ContingencyBranch => "Contingency branch selection",
        UplinkLoadType::NavigationMode => "Navigation mode request",
        UplinkLoadType::HighLevelMode => "High-level mode request",
    }
    .to_owned()
}

fn review_checksum(intent: PresentationActionIntent, epoch: u32) -> u32 {
    intent.proposal_identity
        ^ intent.expected_load_identity.rotate_left(7)
        ^ epoch.rotate_left(13)
        ^ (intent.client_action_sequence as u32)
}

fn map_action_receipt(
    receipt: MissionActionReceipt,
    proposal_identity: u32,
    operation: PresentationActionOperation,
) -> ActionReceiptView {
    ActionReceiptView {
        publication_sequence: 0,
        proposal_identity,
        load_identity: receipt.record.load_identity,
        control_identity: receipt.record.control_identity,
        receipt_epoch: receipt.record.request_epoch,
        effective_epoch: receipt.record.effective_epoch,
        state: receipt.record.state as u8,
        reason: receipt.record.reason as u8,
        accepted: receipt.accepted,
        operation,
        receipt_checksum: receipt.record.receipt_checksum,
    }
}

fn map_prediction_summary(value: &HostPrediction) -> PredictionSummaryView {
    let summary = value.summary;
    PredictionSummaryView {
        prediction_identity: summary.prediction_identity,
        prediction_checksum: summary.prediction_checksum,
        source_estimate_identity: summary.source_estimate_identity,
        frame_identity: summary.frame as u32,
        apogee_q12_km: summary.apogee_q12_km,
        perigee_q12_km: summary.perigee_q12_km,
        time_to_apogee_q16: summary.time_to_apogee_q16,
        time_to_impact_q16: summary.time_to_impact_q16,
        impact_position_q12_km: summary.impact_position_q12_km,
        terminal_reason: summary.terminal_reason as u8,
    }
}

fn map_prediction_path(value: &HostPrediction) -> PredictionPathView {
    PredictionPathView {
        path_identity: value.header.path_identity,
        product_identity: value.header.product as u32,
        model_identity: value.header.model_identity,
        source_estimate_identity: value.header.source_estimate_identity,
        source_estimate_checksum: value.header.source_estimate_checksum,
        source_epoch: value.header.source_epoch,
        generation_epoch: value.header.generation_epoch,
        frame_identity: value.points.first().map_or(0, |point| point.frame as u32),
        terminal_reason: value.header.terminal_reason as u8,
        cadence_releases: u32::from(value.header.cadence_releases),
        path_checksum: value.header.path_checksum,
        points: value
            .points
            .iter()
            .map(|point| PredictionPathPoint {
                release_epoch: point.epoch,
                frame_identity: point.frame as u32,
                position_q12_km: point.position_q12_km,
                altitude_q12_km: point.altitude_q12_km,
                downrange_q12_km: point.downrange_q12_km,
                crossrange_q12_km: point.crossrange_q12_km,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase11_session::sha256;
    use crate::phase12b_live::{
        BRANCH_COMMIT_RELEASE, BRANCH_STAGE_RELEASE, UPDATE_COMMIT_RELEASE,
        UPDATE_EFFECTIVE_RELEASE, UPDATE_STAGE_RELEASE,
    };

    #[test]
    fn transport_worker_and_finalization_states_are_distinct_from_session_lifecycle() {
        assert_eq!(transport_worker_state(PresentationLifecycle::Running), 1);
        assert_eq!(transport_worker_state(PresentationLifecycle::Completed), 2);
        assert_eq!(transport_worker_state(PresentationLifecycle::Incomplete), 3);
        assert_eq!(
            transport_finalization_state(PresentationLifecycle::Running, false),
            0
        );
        assert_eq!(
            transport_finalization_state(PresentationLifecycle::Completed, true),
            1
        );
        assert_eq!(
            transport_finalization_state(PresentationLifecycle::Incomplete, false),
            2
        );
    }

    #[test]
    fn gnss_presentation_state_stays_within_frozen_enum() {
        assert_eq!(gnss_state(GNSS_LOSS_RELEASE - 1), 1);
        assert_eq!(gnss_state(GNSS_LOSS_RELEASE), 2);
        assert_eq!(gnss_state(u32::MAX), 2);
    }

    #[test]
    fn observer_snapshot_never_contains_private_truth_and_cannot_act() {
        let mut session = FullMissionPresentationSession::new(OperationalRole::Observer).unwrap();
        session.prepare().unwrap();
        assert_eq!(session.latest_snapshot().truth, None);
        let intent = PresentationActionIntent {
            proposal_identity: 1,
            expected_load_identity: 0,
            operation: PresentationActionOperation::Review,
            requested_activation_epoch: 0,
            client_action_sequence: 1,
        };
        assert_eq!(
            session.submit_action(intent),
            Err(PresentationSessionError::Intent(
                PresentationValueError::Role
            ))
        );
    }

    #[test]
    fn polling_and_cursors_do_not_advance_authority() {
        let mut session =
            FullMissionPresentationSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        let before = session.latest_snapshot();
        let _ = session.latest_snapshot();
        let _ = session.current_procedure();
        let _ = session.current_prediction_paths();
        let _ = session.transport_status();
        let _ = session.read_events(session.cursors().events, 32).unwrap();
        assert_eq!(
            session.latest_snapshot().release_epoch,
            before.release_epoch
        );
        assert_eq!(
            session.latest_snapshot().flight_checksum,
            before.flight_checksum
        );
    }

    #[test]
    fn stale_action_sequence_fails_before_authority_changes() {
        let mut session =
            FullMissionPresentationSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        let before = session.latest_snapshot();
        let intent = PresentationActionIntent {
            proposal_identity: 1,
            expected_load_identity: 0,
            operation: PresentationActionOperation::Review,
            requested_activation_epoch: 0,
            client_action_sequence: 2,
        };
        assert_eq!(
            session.submit_action(intent),
            Err(PresentationSessionError::ActionSequence)
        );
        assert_eq!(session.latest_snapshot(), before);
    }

    #[test]
    #[ignore = "full 21,591-release exact presentation run"]
    fn presentation_actions_preserve_accepted_ksb11_exactly() {
        let mut session =
            FullMissionPresentationSession::new(OperationalRole::ScriptedOperator).unwrap();
        session.prepare().unwrap();
        session.set_pace(PresentationPace::Fast).unwrap();
        let mut client_sequence = 1u64;
        while session.lifecycle() != PresentationLifecycle::Completed {
            let epoch = session.latest_snapshot().release_epoch;
            if matches!(epoch, UPDATE_STAGE_RELEASE | BRANCH_STAGE_RELEASE) {
                let proposal = session.current_action_proposal().unwrap();
                session
                    .submit_action(PresentationActionIntent {
                        proposal_identity: proposal.proposal_identity,
                        expected_load_identity: 0,
                        operation: PresentationActionOperation::Review,
                        requested_activation_epoch: proposal.activation_epoch,
                        client_action_sequence: client_sequence,
                    })
                    .unwrap();
                client_sequence += 1;
                session
                    .submit_action(PresentationActionIntent {
                        proposal_identity: proposal.proposal_identity,
                        expected_load_identity: proposal.load_identity,
                        operation: PresentationActionOperation::Stage,
                        requested_activation_epoch: proposal.activation_epoch,
                        client_action_sequence: client_sequence,
                    })
                    .unwrap();
                client_sequence += 1;
            }
            if matches!(epoch, UPDATE_COMMIT_RELEASE | BRANCH_COMMIT_RELEASE) {
                let proposal = session.current_action_proposal().unwrap();
                session
                    .submit_action(PresentationActionIntent {
                        proposal_identity: proposal.proposal_identity,
                        expected_load_identity: proposal.load_identity,
                        operation: PresentationActionOperation::Commit,
                        requested_activation_epoch: proposal.activation_epoch,
                        client_action_sequence: client_sequence,
                    })
                    .unwrap();
                client_sequence += 1;
            }
            session.advance_one_release().unwrap();
        }
        let metadata = session.finalization_evidence().unwrap();
        assert!(metadata.complete);
        assert_eq!(metadata.total_length, 2_911_464);
        assert_eq!(session.latest_snapshot().action_count, 4);
        assert_eq!(session.latest_snapshot().release_epoch, 21_591);
        let bytes = session.sealed_evidence_bytes().unwrap();
        assert_eq!(
            sha256(bytes),
            [
                0x75, 0x54, 0x11, 0x1f, 0x28, 0xd8, 0xf3, 0x62, 0x8a, 0xe3, 0xca, 0x9d, 0x06, 0x9f,
                0xad, 0x34, 0x20, 0x4e, 0x12, 0xf8, 0x62, 0x52, 0xef, 0xd0, 0x0e, 0xcf, 0x74, 0x4c,
                0x0e, 0xe0, 0xfc, 0xd4,
            ]
        );
    }

    #[test]
    fn fast_bounded_advancement_stops_at_the_first_operator_gate() {
        let mut session =
            FullMissionPresentationSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        session.set_pace(PresentationPace::Fast).unwrap();

        while session.latest_snapshot().release_epoch < UPDATE_STAGE_RELEASE {
            session.advance_bounded(256).unwrap();
        }

        assert_eq!(
            session.latest_snapshot().release_epoch,
            UPDATE_STAGE_RELEASE
        );
        let proposal = session.current_action_proposal().unwrap();
        assert_eq!(proposal.activation_epoch, UPDATE_EFFECTIVE_RELEASE);
        assert_ne!(
            session.latest_snapshot().validity_mask & SNAPSHOT_VALID_ACTION,
            0
        );
    }
}
