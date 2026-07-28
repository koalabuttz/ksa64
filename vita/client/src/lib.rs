#![no_std]

//! Passive 960x544 Vita Mission Control view model.
//!
//! The actual SDL2/Vita shell will own drawing, input polling, socket I/O, and
//! platform lifecycle. This crate owns only bounded presentation state and
//! strict KPS1 decoding. It never evaluates mission physics or creates
//! canonical evidence.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use ksa64_presentation::{
    decode_typed_payload, encode_typed_payload, parse_kps1_frame, write_kps1_frame,
    ActionProposalView, ActionReceiptView, DispositionView, Kps1Error, Kps1Header,
    Kps1SequenceCursor, OperationalSnapshot, PresentationActionIntent, PresentationActionOperation,
    PresentationMessageKind, PresentationPayload, PresentationRole, PresentationStaleness,
    ProcedureView, ReleaseSampleView, SealedEvidenceMetadata, TimelineEventView,
    TransportStatusView, KPS1_HEADER_LENGTH,
};

pub const VITA_WIDTH: u16 = 960;
pub const VITA_HEIGHT: u16 = 544;
pub const VITA_FRAME_RATE_TARGET: u8 = 30;
pub const VITA_WORKING_SET_LIMIT_BYTES: usize = 64 * 1024 * 1024;

pub const MAX_TIMELINE_EVENTS: usize = 192;
pub const MAX_RELEASE_SAMPLES: usize = 768;
pub const MAX_TRAJECTORY_POINTS: usize = 768;
pub const MAX_ACTION_RECEIPTS: usize = 64;
pub const MAX_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_LABEL_BYTES: usize = 512;
pub const MAX_PRESENTATION_STATE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VitaPage {
    Status = 1,
    Navigation = 2,
    Procedure = 3,
    Trajectory = 4,
    Timeline = 5,
    Evidence = 6,
}

impl VitaPage {
    pub const fn next(self) -> Self {
        match self {
            Self::Status => Self::Navigation,
            Self::Navigation => Self::Procedure,
            Self::Procedure => Self::Trajectory,
            Self::Trajectory => Self::Timeline,
            Self::Timeline => Self::Evidence,
            Self::Evidence => Self::Status,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Status => Self::Evidence,
            Self::Navigation => Self::Status,
            Self::Procedure => Self::Navigation,
            Self::Trajectory => Self::Procedure,
            Self::Timeline => Self::Trajectory,
            Self::Evidence => Self::Timeline,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitaInput {
    Left,
    Right,
    Up,
    Down,
    Cross,
    Circle,
    Start,
    Select,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitaConnection {
    OfflineReplay,
    Connecting,
    Current,
    Stale,
    ResyncRequired,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitaActionState {
    Idle,
    Review,
    Stage,
    Commit,
    Cancel,
    AwaitingReceipt,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitaClientError {
    Protocol(Kps1Error),
    Sequence,
    Role,
    Oversize,
    NoProposal,
    NoPermission,
    ResyncRequired,
}

impl From<Kps1Error> for VitaClientError {
    fn from(value: Kps1Error) -> Self {
        Self::Protocol(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VitaMemoryBudget {
    pub frame_buffer_bytes: usize,
    pub snapshot_bytes: usize,
    pub procedure_bytes: usize,
    pub timeline_bytes: usize,
    pub samples_bytes: usize,
    pub path_bytes: usize,
    pub receipts_bytes: usize,
    pub evidence_metadata_bytes: usize,
    pub renderer_reserve_bytes: usize,
    pub network_reserve_bytes: usize,
    pub total_bytes: usize,
}

impl VitaMemoryBudget {
    pub const fn estimated() -> Self {
        // The allocated state is much smaller; reserves intentionally leave room
        // for SDL2, textures, network buffers, and Vita runtime overhead.
        let frame_buffer_bytes = MAX_FRAME_BYTES * 2;
        let snapshot_bytes = 16 * 1024;
        let procedure_bytes = 16 * 1024;
        let timeline_bytes = MAX_TIMELINE_EVENTS * 768;
        let samples_bytes = MAX_RELEASE_SAMPLES * 96;
        let path_bytes = MAX_TRAJECTORY_POINTS * 48;
        let receipts_bytes = MAX_ACTION_RECEIPTS * 64;
        let evidence_metadata_bytes = 16 * 1024;
        let renderer_reserve_bytes = 12 * 1024 * 1024;
        let network_reserve_bytes = 4 * 1024 * 1024;
        let total_bytes = frame_buffer_bytes
            + snapshot_bytes
            + procedure_bytes
            + timeline_bytes
            + samples_bytes
            + path_bytes
            + receipts_bytes
            + evidence_metadata_bytes
            + renderer_reserve_bytes
            + network_reserve_bytes;
        Self {
            frame_buffer_bytes,
            snapshot_bytes,
            procedure_bytes,
            timeline_bytes,
            samples_bytes,
            path_bytes,
            receipts_bytes,
            evidence_metadata_bytes,
            renderer_reserve_bytes,
            network_reserve_bytes,
            total_bytes,
        }
    }

    pub const fn fits(self) -> bool {
        self.total_bytes <= VITA_WORKING_SET_LIMIT_BYTES
    }
}

#[derive(Clone, Debug, Default)]
struct BoundedHistory<T> {
    values: Vec<T>,
    capacity: usize,
    first_sequence: u64,
    overflowed: bool,
}

impl<T> BoundedHistory<T> {
    fn new(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            capacity,
            first_sequence: 1,
            overflowed: false,
        }
    }

    fn push(&mut self, sequence: u64, value: T) {
        if self.values.len() == self.capacity {
            self.values.remove(0);
            self.first_sequence = self.first_sequence.saturating_add(1);
            self.overflowed = true;
        }
        self.values.push(value);
        if self.values.len() == 1 {
            self.first_sequence = sequence;
        }
    }

    fn clear(&mut self) {
        self.values.clear();
        self.first_sequence = 1;
        self.overflowed = false;
    }

    fn as_slice(&self) -> &[T] {
        &self.values
    }
}

/// Compact state handed to an SDL2 Vita renderer. It contains only public,
/// role-filtered presentation values.
#[derive(Clone, Debug)]
pub struct VitaViewModel {
    pub page: VitaPage,
    pub role: PresentationRole,
    pub connection: VitaConnection,
    pub action_state: VitaActionState,
    pub snapshot: Option<OperationalSnapshot>,
    pub procedure: Option<ProcedureView>,
    pub disposition: Option<DispositionView>,
    pub proposal: Option<ActionProposalView>,
    pub transport: Option<TransportStatusView>,
    pub evidence: Option<SealedEvidenceMetadata>,
    pub resync_required: bool,
    pub stale_frames: u32,
    pub status_line: String,
}

pub struct VitaMissionControl {
    role: PresentationRole,
    page: VitaPage,
    connection: VitaConnection,
    action_state: VitaActionState,
    session_nonce: Option<u64>,
    inbound_cursor: Option<Kps1SequenceCursor>,
    outbound_sequence: u64,
    client_action_sequence: u64,
    snapshot: Option<OperationalSnapshot>,
    procedure: Option<ProcedureView>,
    disposition: Option<DispositionView>,
    proposal: Option<ActionProposalView>,
    transport: Option<TransportStatusView>,
    evidence: Option<SealedEvidenceMetadata>,
    timeline: BoundedHistory<TimelineEventView>,
    samples: BoundedHistory<ReleaseSampleView>,
    receipts: BoundedHistory<ActionReceiptView>,
    status_line: String,
    stale_frames: u32,
    resync_required: bool,
}

impl VitaMissionControl {
    pub fn new(role: PresentationRole) -> Result<Self, VitaClientError> {
        if !matches!(
            role,
            PresentationRole::Observer
                | PresentationRole::GuidedOperator
                | PresentationRole::FlightController
                | PresentationRole::FlightSoftwareEngineer
                | PresentationRole::SimDirector
        ) {
            return Err(VitaClientError::Role);
        }
        debug_assert!(VitaMemoryBudget::estimated().fits());
        Ok(Self {
            role,
            page: VitaPage::Status,
            connection: VitaConnection::Connecting,
            action_state: VitaActionState::Idle,
            session_nonce: None,
            inbound_cursor: None,
            outbound_sequence: 1,
            client_action_sequence: 1,
            snapshot: None,
            procedure: None,
            disposition: None,
            proposal: None,
            transport: None,
            evidence: None,
            timeline: BoundedHistory::new(MAX_TIMELINE_EVENTS),
            samples: BoundedHistory::new(MAX_RELEASE_SAMPLES),
            receipts: BoundedHistory::new(MAX_ACTION_RECEIPTS),
            status_line: String::from("CONNECTING / WAITING FOR ROLE-BOUND KPS1"),
            stale_frames: 0,
            resync_required: false,
        })
    }

    pub fn offline_replay(role: PresentationRole) -> Result<Self, VitaClientError> {
        let mut client = Self::new(role)?;
        client.connection = VitaConnection::OfflineReplay;
        client.status_line = String::from("OFFLINE ROLE-FILTERED REPLAY");
        Ok(client)
    }

    pub const fn page(&self) -> VitaPage {
        self.page
    }

    pub const fn role(&self) -> PresentationRole {
        self.role
    }

    pub const fn connection(&self) -> VitaConnection {
        self.connection
    }

    pub fn memory_budget(&self) -> VitaMemoryBudget {
        VitaMemoryBudget::estimated()
    }

    pub fn view_model(&self) -> VitaViewModel {
        VitaViewModel {
            page: self.page,
            role: self.role,
            connection: self.connection,
            action_state: self.action_state,
            snapshot: self.snapshot.clone(),
            procedure: self.procedure.clone(),
            disposition: self.disposition,
            proposal: self.proposal.clone(),
            transport: self.transport,
            evidence: self.evidence,
            resync_required: self.resync_required,
            stale_frames: self.stale_frames,
            status_line: self.status_line.clone(),
        }
    }

    pub fn timeline(&self) -> &[TimelineEventView] {
        self.timeline.as_slice()
    }

    pub fn samples(&self) -> &[ReleaseSampleView] {
        self.samples.as_slice()
    }

    /// Returns whether a bounded local retention window lost older timeline data.
    /// The platform shell must surface this as a resynchronization prompt, never
    /// silently pretend the view is complete.
    pub const fn timeline_overflowed(&self) -> bool {
        self.timeline.overflowed
    }

    pub fn receipts(&self) -> &[ActionReceiptView] {
        self.receipts.as_slice()
    }

    pub fn handle_input(
        &mut self,
        input: VitaInput,
    ) -> Result<Option<PresentationActionIntent>, VitaClientError> {
        match input {
            VitaInput::Left => self.page = self.page.previous(),
            VitaInput::Right => self.page = self.page.next(),
            VitaInput::Circle => {
                self.action_state = VitaActionState::Cancel;
                return self
                    .build_intent(PresentationActionOperation::Cancel)
                    .map(Some);
            }
            VitaInput::Cross => {
                let operation = match self.action_state {
                    VitaActionState::Idle | VitaActionState::Rejected => {
                        PresentationActionOperation::Review
                    }
                    VitaActionState::Review => PresentationActionOperation::Stage,
                    VitaActionState::Stage => PresentationActionOperation::Commit,
                    VitaActionState::Commit
                    | VitaActionState::Cancel
                    | VitaActionState::AwaitingReceipt => return Ok(None),
                };
                return self.build_intent(operation).map(Some);
            }
            VitaInput::Start | VitaInput::Select | VitaInput::Up | VitaInput::Down => {}
        }
        Ok(None)
    }

    pub fn mark_disconnected(&mut self) {
        self.connection = VitaConnection::Stale;
        self.status_line = String::from("LINK STALE / REMOTE AUTHORITY CONTINUES");
    }

    pub fn mark_closed(&mut self) {
        self.connection = VitaConnection::Closed;
        self.status_line = String::from("SESSION CLOSED");
    }

    pub fn receive_kps1(&mut self, frame: &[u8]) -> Result<(), VitaClientError> {
        if frame.len() > MAX_FRAME_BYTES + KPS1_HEADER_LENGTH {
            return Err(VitaClientError::Oversize);
        }
        let decoded = parse_kps1_frame(frame)?;
        self.accept_inbound_header(decoded.header)?;
        let payload = decode_typed_payload(decoded.header.kind, decoded.payload, self.role)?;
        self.apply_payload(payload)
    }

    pub fn encode_action_intent(
        &mut self,
        intent: PresentationActionIntent,
        output: &mut [u8],
    ) -> Result<usize, VitaClientError> {
        let session_nonce = self.session_nonce.ok_or(VitaClientError::Sequence)?;
        let payload = encode_typed_payload(&PresentationPayload::ActionIntent(intent), self.role)?;
        let header = Kps1Header {
            kind: PresentationMessageKind::ActionIntent,
            flags: 0,
            session_nonce,
            sequence: self.outbound_sequence,
            correlation_id: self.outbound_sequence,
            payload_length: payload.len() as u32,
        };
        let written = write_kps1_frame(header, &payload, output)?;
        self.outbound_sequence = self
            .outbound_sequence
            .checked_add(1)
            .ok_or(VitaClientError::Sequence)?;
        Ok(written)
    }

    pub fn reset_for_resync(&mut self) {
        self.timeline.clear();
        self.samples.clear();
        self.receipts.clear();
        self.resync_required = false;
        self.connection = VitaConnection::Connecting;
        self.status_line = String::from("RESYNC REQUESTED / WAITING FOR FRESH SNAPSHOT");
    }

    fn build_intent(
        &mut self,
        operation: PresentationActionOperation,
    ) -> Result<PresentationActionIntent, VitaClientError> {
        if !self.role.permits_operator_actions() {
            return Err(VitaClientError::NoPermission);
        }
        if self.resync_required {
            return Err(VitaClientError::ResyncRequired);
        }
        let proposal = self.proposal.as_ref().ok_or(VitaClientError::NoProposal)?;
        if proposal.permitted_operations & operation.permission_bit() == 0 {
            return Err(VitaClientError::NoPermission);
        }
        let intent = PresentationActionIntent {
            proposal_identity: proposal.proposal_identity,
            expected_load_identity: proposal.load_identity,
            operation,
            requested_activation_epoch: proposal.activation_epoch,
            client_action_sequence: self.client_action_sequence,
        };
        self.client_action_sequence = self
            .client_action_sequence
            .checked_add(1)
            .ok_or(VitaClientError::Sequence)?;
        self.action_state = match operation {
            PresentationActionOperation::Review => VitaActionState::Review,
            PresentationActionOperation::Stage => VitaActionState::Stage,
            PresentationActionOperation::Commit => VitaActionState::Commit,
            PresentationActionOperation::Cancel => VitaActionState::Cancel,
        };
        self.status_line = String::from("ACTION PROPOSED / AWAITING AUTHORITATIVE RECEIPT");
        Ok(intent)
    }

    fn accept_inbound_header(&mut self, header: Kps1Header) -> Result<(), VitaClientError> {
        if matches!(header.kind, PresentationMessageKind::HandshakeResponse) {
            if header.session_nonce == 0 {
                return Err(VitaClientError::Sequence);
            }
            self.session_nonce = Some(header.session_nonce);
            self.inbound_cursor = Some(Kps1SequenceCursor::new(
                header.session_nonce,
                header.sequence,
            )?);
        }
        let cursor = self
            .inbound_cursor
            .as_mut()
            .ok_or(VitaClientError::Sequence)?;
        cursor.accept(header)?;
        Ok(())
    }

    fn apply_payload(&mut self, payload: PresentationPayload) -> Result<(), VitaClientError> {
        match payload {
            PresentationPayload::HandshakeResponse(handshake) => {
                if handshake.role != self.role {
                    return Err(VitaClientError::Role);
                }
                self.connection = VitaConnection::Current;
                self.status_line = String::from("PAIRED / ROLE-BOUND SESSION ACTIVE");
            }
            PresentationPayload::Snapshot(value) => {
                if value.role != self.role {
                    return Err(VitaClientError::Role);
                }
                self.snapshot = Some(value);
                self.connection = VitaConnection::Current;
                self.stale_frames = 0;
            }
            PresentationPayload::Procedure(value) => self.procedure = Some(value),
            PresentationPayload::Disposition(value) => self.disposition = Some(value),
            PresentationPayload::ActionProposal(value) => self.proposal = Some(value),
            PresentationPayload::ActionReceipt(value) => {
                self.receipts.push(value.publication_sequence, value);
                self.action_state = if value.accepted {
                    VitaActionState::AwaitingReceipt
                } else {
                    VitaActionState::Rejected
                };
                self.status_line = if value.accepted {
                    String::from("AUTHORITATIVE ACTION RECEIPT ACCEPTED")
                } else {
                    String::from("AUTHORITATIVE ACTION RECEIPT REJECTED")
                };
            }
            PresentationPayload::TimelineEvent(value) => self.timeline.push(value.sequence, value),
            PresentationPayload::ReleaseSampleBatch(values) => {
                for value in values {
                    self.samples.push(value.sequence, value);
                }
            }
            PresentationPayload::TransportStatus(value) => {
                self.stale_frames = value.queue.commands_pending;
                self.transport = Some(value);
                self.connection = match value.staleness {
                    PresentationStaleness::Current => VitaConnection::Current,
                    PresentationStaleness::Delayed
                    | PresentationStaleness::Stale
                    | PresentationStaleness::Disconnected => VitaConnection::Stale,
                    PresentationStaleness::Resynchronizing => {
                        self.resync_required = true;
                        VitaConnection::ResyncRequired
                    }
                };
            }
            PresentationPayload::EvidenceMetadata(value) => self.evidence = Some(value),
            PresentationPayload::Error(value) => {
                self.status_line = value.message;
                if value.fatal {
                    self.connection = VitaConnection::Closed;
                }
            }
            PresentationPayload::EventBatch(_)
            | PresentationPayload::PredictionPath(_)
            | PresentationPayload::EvidenceChunk(_) => {
                // The first Vita slice retains compact timeline and release
                // history only. Full paths/evidence are fetched page-wise by the
                // platform shell in a later UI pass.
            }
            PresentationPayload::HandshakeRequest(_)
            | PresentationPayload::LifecycleControl(_)
            | PresentationPayload::PaceControl(_)
            | PresentationPayload::ReplayControl(_)
            | PresentationPayload::ActionIntent(_) => {
                return Err(VitaClientError::Protocol(Kps1Error::Enum))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use ksa64_presentation::{
        encode_typed_payload, ActionProposalView, PresentationCursors, PresentationHandshake,
        PresentationPayload, ACTION_PERMIT_CANCEL, ACTION_PERMIT_COMMIT, ACTION_PERMIT_REVIEW,
        ACTION_PERMIT_STAGE, KPS1_FLAG_RESPONSE,
    };

    fn response_frame(
        kind: PresentationMessageKind,
        payload: PresentationPayload,
        sequence: u64,
        role: PresentationRole,
    ) -> alloc::vec::Vec<u8> {
        let payload = encode_typed_payload(&payload, role).unwrap();
        let header = Kps1Header {
            kind,
            flags: KPS1_FLAG_RESPONSE,
            session_nonce: 0xAABB_CCDD_EEFF_0011,
            sequence,
            correlation_id: if matches!(kind, PresentationMessageKind::HandshakeResponse) {
                1
            } else {
                0
            },
            payload_length: payload.len() as u32,
        };
        let mut frame = alloc::vec![0; KPS1_HEADER_LENGTH + payload.len()];
        write_kps1_frame(header, &payload, &mut frame).unwrap();
        frame
    }

    #[test]
    fn budget_leaves_substantial_headroom_inside_sixty_four_mebibytes() {
        assert!(VitaMemoryBudget::estimated().fits());
        assert!(VitaMemoryBudget::estimated().total_bytes < 20 * 1024 * 1024);
    }

    #[test]
    fn role_filtered_session_and_review_stage_commit_are_presentation_only() {
        let role = PresentationRole::GuidedOperator;
        let mut client = VitaMissionControl::new(role).unwrap();
        let handshake = PresentationHandshake {
            role,
            client_instance: 7,
            capability_mask: 0,
            cursors: PresentationCursors::default(),
        };
        client
            .receive_kps1(&response_frame(
                PresentationMessageKind::HandshakeResponse,
                PresentationPayload::HandshakeResponse(handshake),
                1,
                role,
            ))
            .unwrap();

        let proposal = ActionProposalView {
            proposal_identity: 99,
            load_identity: 77,
            load_type: 1,
            permitted_operations: ACTION_PERMIT_REVIEW
                | ACTION_PERMIT_STAGE
                | ACTION_PERMIT_COMMIT
                | ACTION_PERMIT_CANCEL,
            stage_epoch: 4,
            earliest_commit_epoch: 6,
            activation_epoch: 8,
            expires_epoch: 20,
            payload_checksum: 1,
            completed_event_mask: 0,
            label: String::from("GROUND UPDATE"),
        };
        client
            .receive_kps1(&response_frame(
                PresentationMessageKind::ActionProposal,
                PresentationPayload::ActionProposal(proposal),
                2,
                role,
            ))
            .unwrap();

        let review = client.handle_input(VitaInput::Cross).unwrap().unwrap();
        assert_eq!(review.operation, PresentationActionOperation::Review);
        let stage = client.handle_input(VitaInput::Cross).unwrap().unwrap();
        assert_eq!(stage.operation, PresentationActionOperation::Stage);
        let commit = client.handle_input(VitaInput::Cross).unwrap().unwrap();
        assert_eq!(commit.operation, PresentationActionOperation::Commit);

        let mut encoded = [0_u8; 128];
        let bytes = client.encode_action_intent(commit, &mut encoded).unwrap();
        assert!(bytes > KPS1_HEADER_LENGTH);
        assert_eq!(
            parse_kps1_frame(&encoded[..bytes]).unwrap().header.kind,
            PresentationMessageKind::ActionIntent
        );
    }

    #[test]
    fn stale_or_resync_status_never_mutates_authority_and_blocks_actions() {
        let mut client = VitaMissionControl::new(PresentationRole::GuidedOperator).unwrap();
        client.resync_required = true;
        assert_eq!(
            client.handle_input(VitaInput::Cross),
            Err(VitaClientError::ResyncRequired)
        );
        client.mark_disconnected();
        assert_eq!(client.connection(), VitaConnection::Stale);
        client.reset_for_resync();
        assert_eq!(client.connection(), VitaConnection::Connecting);
    }

    #[test]
    fn bounded_history_explicitly_records_a_retention_gap() {
        let mut history = BoundedHistory::new(2);
        history.push(1, 10_u32);
        history.push(2, 20_u32);
        history.push(3, 30_u32);
        assert!(history.overflowed);
        assert_eq!(history.first_sequence, 2);
        assert_eq!(history.as_slice(), &[20, 30]);
    }

    #[test]
    fn observer_cannot_construct_an_operator_action() {
        let mut client = VitaMissionControl::new(PresentationRole::Observer).unwrap();
        client.proposal = Some(ActionProposalView {
            proposal_identity: 1,
            load_identity: 2,
            load_type: 1,
            permitted_operations: ACTION_PERMIT_REVIEW,
            stage_epoch: 0,
            earliest_commit_epoch: 0,
            activation_epoch: 0,
            expires_epoch: 1,
            payload_checksum: 0,
            completed_event_mask: 0,
            label: String::new(),
        });
        assert_eq!(
            client.handle_input(VitaInput::Cross),
            Err(VitaClientError::NoPermission)
        );
    }
}
