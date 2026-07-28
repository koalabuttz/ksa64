#![no_std]

//! Passive 960x544 Vita Mission Control view model.
//!
//! The actual SDL2/Vita shell will own drawing, input polling, socket I/O, and
//! platform lifecycle. This crate owns only bounded presentation state and
//! strict KPS1 decoding. It never evaluates mission physics or creates
//! canonical evidence.

extern crate alloc;
#[cfg(feature = "vita-target")]
extern crate std;

use alloc::{string::String, vec::Vec};
use ksa64_presentation::{
    decode_typed_payload, encode_typed_payload, parse_kps1_frame, write_kps1_frame,
    ActionProposalView, ActionReceiptView, DispositionAxes, DispositionView, Kps1Error, Kps1Header,
    Kps1SequenceCursor, NavigationView, OperationalSnapshot, OverallDisposition,
    PredictionSummaryView, PresentationActionIntent, PresentationActionOperation,
    PresentationCursors, PresentationLifecycle, PresentationMessageKind, PresentationPace,
    PresentationPayload, PresentationRole, PresentationStaleness, ProcedurePredicateView,
    ProcedureStepState, ProcedureView, ReleaseSampleView, SealedEvidenceMetadata,
    TimelineEventView, TimelineSeverity, TransportStatusView, KPS1_HEADER_LENGTH,
    PRESENTATION_MODEL_ID, SNAPSHOT_VALID_DISPOSITION, SNAPSHOT_VALID_EVIDENCE,
    SNAPSHOT_VALID_GNSS, SNAPSHOT_VALID_GROUND_ESTIMATE, SNAPSHOT_VALID_MISSION_TIME,
    SNAPSHOT_VALID_NAVIGATION, SNAPSHOT_VALID_PREDICTION, SNAPSHOT_VALID_PROCEDURE,
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
    replay_cursors: PresentationCursors,
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
            replay_cursors: PresentationCursors::default(),
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

    /// Loads a bounded presentation-only replay used when no paired authority is
    /// available. It contains no private truth and cannot accept actions.
    pub fn load_offline_fixture(&mut self) {
        self.connection = VitaConnection::OfflineReplay;
        self.status_line = String::from("OFFLINE ROLE-FILTERED GNSS-LOSS REPLAY");
        self.proposal = None;
        self.snapshot = Some(
            OperationalSnapshot {
                presentation_model_identity: PRESENTATION_MODEL_ID,
                session_definition_identity: 0x4752_3131,
                publication_sequence: 240,
                validity_mask: SNAPSHOT_VALID_MISSION_TIME
                    | SNAPSHOT_VALID_NAVIGATION
                    | SNAPSHOT_VALID_GROUND_ESTIMATE
                    | SNAPSHOT_VALID_PREDICTION
                    | SNAPSHOT_VALID_PROCEDURE
                    | SNAPSHOT_VALID_DISPOSITION
                    | SNAPSHOT_VALID_EVIDENCE
                    | SNAPSHOT_VALID_GNSS,
                role: self.role,
                lifecycle: PresentationLifecycle::Completed,
                pace: PresentationPace::Paused,
                release_epoch: 21_591,
                release_period_micros: 31_250,
                frame_identity: 3,
                mission_time_q16: 44_217_344,
                onboard: NavigationView {
                    position_q12_km: [21_120, -9_600, 812_032],
                    velocity_q24_km_s: [10_082_304, -2_097_152, -786_432],
                    checksum: 0x0B0A_4D11,
                },
                ground: NavigationView {
                    position_q12_km: [21_135, -9_592, 812_018],
                    velocity_q24_km_s: [10_081_920, -2_097_010, -786_600],
                    checksum: 0x600D_4D11,
                },
                prediction: PredictionSummaryView {
                    prediction_identity: 0x5052_4431,
                    prediction_checksum: 0x81C0_77E1,
                    source_estimate_identity: 0x4752_4E44,
                    frame_identity: 3,
                    apogee_q12_km: 1_011_712,
                    perigee_q12_km: -4_096,
                    time_to_apogee_q16: 0,
                    time_to_impact_q16: 19_660_800,
                    impact_position_q12_km: [1_720_320, 23_040, 0],
                    terminal_reason: 2,
                },
                flight_checksum: 0xF11C_1021,
                command_checksum: 0xC04D_1021,
                procedure_chain: 0xA110_0024,
                journal_chain: 0xA110_0025,
                action_chain: 0xA110_0026,
                staged_load_identity: 0,
                action_count: 4,
                rejected_loads: 0,
                gnss_state: 3,
                safe: true,
                truth: None,
            }
            .filter_for_role(self.role),
        );
        self.procedure = Some(ProcedureView {
            procedure_identity: 0x4750_534C,
            active_step: 6,
            step_count: 6,
            state: ProcedureStepState::Completed,
            entered_epoch: 8_224,
            deadline_epoch: 9_600,
            title: String::from("ASCENT / LOSS OF GNSS AIDING"),
            instruction: String::from(
                "Ground update accepted; continue under inertial navigation.",
            ),
            predicates: alloc::vec![
                ProcedurePredicateView {
                    identity: 1,
                    satisfied: true
                },
                ProcedurePredicateView {
                    identity: 2,
                    satisfied: true
                },
                ProcedurePredicateView {
                    identity: 3,
                    satisfied: true
                },
            ],
            hints_available: false,
        });
        self.disposition = Some(DispositionView {
            overall: OverallDisposition::ContingencySuccess,
            axes: DispositionAxes {
                objective: 2,
                vehicle: 1,
                procedure: 1,
                operator: 1,
                avionics: 2,
                evidence: 1,
            },
            reason_identity: 0x434F_4E54,
        });
        self.evidence = Some(SealedEvidenceMetadata {
            evidence_identity: 0x4B53_4231,
            evidence_crc32: 0xA4A6_E037,
            total_length: 2_911_464,
            chunk_length: 65_536,
            chunk_count: 45,
            complete: true,
            content_kind: 11,
        });
        self.timeline.clear();
        for (sequence, epoch, severity, label) in [
            (
                1,
                1_248,
                TimelineSeverity::Information,
                "RAIL CLEAR / ECEF TRANSITION",
            ),
            (2, 5_760, TimelineSeverity::Information, "BURNOUT QUALIFIED"),
            (3, 7_840, TimelineSeverity::Warning, "GNSS AID INVALID"),
            (
                4,
                8_224,
                TimelineSeverity::Caution,
                "LOSS-OF-GNSS PROCEDURE ENTERED",
            ),
            (
                5,
                8_736,
                TimelineSeverity::Information,
                "GROUND STATE UPDATE COMMITTED",
            ),
            (
                6,
                10_368,
                TimelineSeverity::Information,
                "CONTINGENCY BRANCH ACTIVE",
            ),
            (
                7,
                21_591,
                TimelineSeverity::Information,
                "RECOVERY COMPLETE / EVIDENCE SEALED",
            ),
        ] {
            self.timeline.push(
                sequence,
                TimelineEventView {
                    sequence,
                    release_epoch: epoch,
                    source_identity: 0x5649_5441,
                    severity,
                    event_identity: sequence as u32,
                    detail_identity: 0,
                    label: String::from(label),
                },
            );
        }
        self.samples.clear();
        for index in 0..96_u64 {
            let altitude = if index < 48 {
                index as i32 * 20_000
            } else {
                (96 - index) as i32 * 20_000
            };
            let downrange = index as i32 * 17_500;
            self.samples.push(
                index + 1,
                ReleaseSampleView {
                    sequence: index + 1,
                    validity_mask: 0x7ff,
                    release_epoch: (index as u32) * 224,
                    mission_time_q16: (index as u32) * 458_752,
                    frame_identity: 3,
                    onboard_position_q12_km: [downrange, index as i32 * 240, altitude],
                    onboard_velocity_q24_km_s: [
                        10_000_000,
                        120_000,
                        if index < 48 { 1_400_000 } else { -1_400_000 },
                    ],
                    ground_position_q12_km: [
                        downrange + 64,
                        index as i32 * 240 - 32,
                        altitude - 48,
                    ],
                    ground_velocity_q24_km_s: [
                        9_999_800,
                        120_100,
                        if index < 48 { 1_399_900 } else { -1_400_100 },
                    ],
                    predicted_impact_q12_km: [1_720_320, 23_040, 0],
                    predicted_apogee_q12_km: 1_011_712,
                    altitude_q12_km: altitude,
                    speed_q24_km_s: 10_120_000,
                    downrange_q12_km: downrange,
                    crossrange_q12_km: index as i32 * 240,
                },
            );
        }
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

    /// The next retained-stream cursors for a broker publication request.
    pub const fn replay_cursors(&self) -> PresentationCursors {
        self.replay_cursors
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

    /// Reserves KPS1 sequence one for the encrypted transport handshake.
    /// The next locally constructed high-level action will therefore be
    /// sequence two; direct effector commands remain impossible.
    #[cfg(feature = "vita-target")]
    pub fn reserve_paired_handshake_sequence(&mut self) {
        self.outbound_sequence = 2;
    }

    /// Encodes a bounded replay/resynchronization request using the same
    /// session-local sequence cursor as high-level action proposals.
    pub fn encode_replay_control(
        &mut self,
        cursors: PresentationCursors,
        output: &mut [u8],
    ) -> Result<usize, VitaClientError> {
        let session_nonce = self.session_nonce.ok_or(VitaClientError::Sequence)?;
        let payload =
            encode_typed_payload(&PresentationPayload::ReplayControl(cursors), self.role)?;
        let header = Kps1Header {
            kind: PresentationMessageKind::ReplayControl,
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
        self.replay_cursors = PresentationCursors::default();
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
                self.replay_cursors = handshake.cursors;
                self.connection = VitaConnection::Current;
                self.status_line = String::from("PAIRED / ROLE-BOUND SESSION ACTIVE");
            }
            PresentationPayload::Snapshot(value) => {
                if value.role != self.role {
                    return Err(VitaClientError::Role);
                }
                self.replay_cursors.snapshots = value.publication_sequence;
                self.snapshot = Some(value);
                self.connection = VitaConnection::Current;
                self.stale_frames = 0;
            }
            PresentationPayload::Procedure(value) => self.procedure = Some(value),
            PresentationPayload::Disposition(value) => self.disposition = Some(value),
            PresentationPayload::ActionProposal(value) => self.proposal = Some(value),
            PresentationPayload::ActionReceipt(value) => {
                self.replay_cursors.action_receipts = value.publication_sequence.saturating_add(1);
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
            PresentationPayload::TimelineEvent(value) => {
                self.replay_cursors.timeline = value.sequence.saturating_add(1);
                self.timeline.push(value.sequence, value)
            }
            PresentationPayload::ReleaseSampleBatch(values) => {
                for value in values {
                    self.replay_cursors.release_samples = value.sequence.saturating_add(1);
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
            PresentationPayload::EventBatch(values) => {
                if let Some(last) = values.last() {
                    self.replay_cursors.events = last.sequence.saturating_add(1);
                }
            }
            PresentationPayload::PredictionPath(_) | PresentationPayload::EvidenceChunk(_) => {
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

#[cfg(feature = "vita-target")]
pub mod paired_transport {
    //! Opt-in paired LAN transport. Offline replay remains the safe default.
    use crate::{VitaClientError, VitaMissionControl};
    use alloc::{collections::VecDeque, vec, vec::Vec};
    use core::fmt;
    use ksa64_presentation::{
        encode_typed_payload, write_kps1_frame, Kps1Header, PresentationCursors,
        PresentationHandshake, PresentationMessageKind, PresentationPayload, PresentationRole,
        KPS1_HEADER_LENGTH,
    };
    use ksa64_session_broker::{
        AuthenticatedNoiseChannel, ComparisonCode, HandshakeEntropy, IkInitiator,
        NoiseTransportError, PeerRecord, PeerRegistry, StaticNoiseKeypair, XxInitiator,
        MAX_HANDSHAKE_MESSAGE_LENGTH, MAX_NOISE_CIPHERTEXT_LENGTH,
    };
    use std::{
        io::{self, Read, Write},
        net::{IpAddr, SocketAddr, TcpStream},
        time::Duration,
    };

    pub const VITA_PAIRED_ROLE: PresentationRole = PresentationRole::GuidedOperator;
    pub const MAX_QUEUED_PACKETS: usize = 8;
    pub const MAX_QUEUED_BYTES: usize = 512 * 1024;
    pub const MAX_INBOUND_BYTES: usize = 2 * MAX_NOISE_CIPHERTEXT_LENGTH;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct VitaLanConfig {
        pub server: SocketAddr,
        pub session_nonce: u64,
        pub connect_timeout_millis: u64,
        pub handshake_timeout_millis: u64,
    }
    impl VitaLanConfig {
        pub fn paired(server: SocketAddr, session_nonce: u64) -> Result<Self, VitaLanError> {
            let value = Self {
                server,
                session_nonce,
                connect_timeout_millis: 5_000,
                handshake_timeout_millis: 10_000,
            };
            value.validate()?;
            Ok(value)
        }
        pub fn validate(self) -> Result<(), VitaLanError> {
            if self.session_nonce == 0
                || self.server.port() == 0
                || self.server.ip().is_unspecified()
                || !private_lan(self.server.ip())
                || self.connect_timeout_millis == 0
                || self.handshake_timeout_millis == 0
            {
                return Err(VitaLanError::Config);
            }
            Ok(())
        }
    }
    fn private_lan(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v) => v.is_private() || v.is_link_local(),
            IpAddr::V6(v) => v.is_unique_local() || v.is_unicast_link_local(),
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VitaLanState {
        Offline,
        PairingCodePending,
        Active,
        Stale,
        ResyncRequired,
        Closed,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VitaLanError {
        Config,
        Entropy,
        Persistence,
        Io,
        Timeout,
        QueueFull,
        PacketLength,
        State,
        Role,
        Handshake(NoiseTransportError),
        Protocol(VitaClientError),
    }
    impl From<NoiseTransportError> for VitaLanError {
        fn from(value: NoiseTransportError) -> Self {
            Self::Handshake(value)
        }
    }
    impl From<VitaClientError> for VitaLanError {
        fn from(value: VitaClientError) -> Self {
            Self::Protocol(value)
        }
    }

    /// Local user-partition identity. This bounded opaque setting is neither
    /// evidence nor a presentation record. The host peer key is saved only
    /// after both ends confirm the same XX comparison code.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct VitaPeerIdentity {
        pub private_key: [u8; 32],
        pub public_key: [u8; 32],
        pub server_public_key: Option<[u8; 32]>,
    }
    impl VitaPeerIdentity {
        pub const ENCODED_LENGTH: usize = 112;
        pub fn from_parts(private_key: [u8; 32], public_key: [u8; 32]) -> Self {
            Self {
                private_key,
                public_key,
                server_public_key: None,
            }
        }
        pub fn generate(entropy: HandshakeEntropy) -> Result<Self, VitaLanError> {
            let keys = StaticNoiseKeypair::generate_with_entropy(entropy)?;
            Ok(Self {
                private_key: keys.private_key_for_secure_store(),
                public_key: keys.public_key(),
                server_public_key: None,
            })
        }
        pub fn keys(&self) -> StaticNoiseKeypair {
            StaticNoiseKeypair::from_parts(self.private_key, self.public_key)
        }
        pub fn encode(&self, output: &mut [u8]) -> Result<usize, VitaLanError> {
            if output.len() < Self::ENCODED_LENGTH {
                return Err(VitaLanError::Persistence);
            }
            output[..Self::ENCODED_LENGTH].fill(0);
            output[..4].copy_from_slice(b"VPI1");
            output[4..6].copy_from_slice(&1_u16.to_le_bytes());
            output[8] = u8::from(self.server_public_key.is_some());
            output[12..44].copy_from_slice(&self.private_key);
            output[44..76].copy_from_slice(&self.public_key);
            if let Some(key) = self.server_public_key {
                output[76..108].copy_from_slice(&key);
            }
            let crc = identity_crc32(&output[..108]);
            output[108..112].copy_from_slice(&crc.to_le_bytes());
            Ok(Self::ENCODED_LENGTH)
        }
        pub fn decode(input: &[u8]) -> Result<Self, VitaLanError> {
            if input.len() != Self::ENCODED_LENGTH
                || input[..4] != *b"VPI1"
                || u16::from_le_bytes([input[4], input[5]]) != 1
                || input[6..8].iter().any(|v| *v != 0)
                || input[8] > 1
                || input[9..12].iter().any(|v| *v != 0)
                || u32::from_le_bytes(input[108..112].try_into().unwrap())
                    != identity_crc32(&input[..108])
            {
                return Err(VitaLanError::Persistence);
            }
            let mut private_key = [0; 32];
            let mut public_key = [0; 32];
            private_key.copy_from_slice(&input[12..44]);
            public_key.copy_from_slice(&input[44..76]);
            if private_key.iter().all(|v| *v == 0) || public_key.iter().all(|v| *v == 0) {
                return Err(VitaLanError::Persistence);
            }
            let server_public_key = if input[8] == 1 {
                let mut key = [0; 32];
                key.copy_from_slice(&input[76..108]);
                if key.iter().all(|v| *v == 0) {
                    return Err(VitaLanError::Persistence);
                }
                Some(key)
            } else {
                if input[76..108].iter().any(|v| *v != 0) {
                    return Err(VitaLanError::Persistence);
                }
                None
            };
            Ok(Self {
                private_key,
                public_key,
                server_public_key,
            })
        }
    }

    fn identity_crc32(bytes: &[u8]) -> u32 {
        let mut value = 0xffff_ffff_u32;
        for byte in bytes {
            value ^= u32::from(*byte);
            for _ in 0..8 {
                value = if value & 1 != 0 {
                    (value >> 1) ^ 0xedb8_8320
                } else {
                    value >> 1
                };
            }
        }
        !value
    }

    #[cfg(target_os = "vita")]
    #[link(name = "SceLibKernel_stub")]
    unsafe extern "C" {
        fn sceKernelGetRandomNumber(output: *mut core::ffi::c_void, size: usize) -> i32;
    }
    #[cfg(target_os = "vita")]
    pub fn vita_secure_entropy() -> Result<HandshakeEntropy, VitaLanError> {
        let mut bytes = [0; 32];
        if unsafe { sceKernelGetRandomNumber(bytes.as_mut_ptr().cast(), bytes.len()) } < 0 {
            Err(VitaLanError::Entropy)
        } else {
            Ok(HandshakeEntropy::from_bytes(bytes))
        }
    }
    #[cfg(not(target_os = "vita"))]
    pub fn vita_secure_entropy() -> Result<HandshakeEntropy, VitaLanError> {
        Err(VitaLanError::Entropy)
    }

    struct PendingPairing {
        unconfirmed: ksa64_session_broker::UnconfirmedNoiseChannel,
        code: ComparisonCode,
    }
    pub struct VitaLanClient {
        config: VitaLanConfig,
        identity: VitaPeerIdentity,
        stream: TcpStream,
        state: VitaLanState,
        pending: Option<PendingPairing>,
        channel: Option<AuthenticatedNoiseChannel>,
        inbound: Vec<u8>,
        outbound: VecDeque<Vec<u8>>,
        offset: usize,
        queued: usize,
    }
    impl VitaLanClient {
        pub fn begin_pairing(
            config: VitaLanConfig,
            identity: VitaPeerIdentity,
            entropy: HandshakeEntropy,
        ) -> Result<Self, VitaLanError> {
            config.validate()?;
            let mut stream = connect(config)?;
            blocking_write(&mut stream, &selector(1))?;
            let mut init = XxInitiator::with_entropy(&identity.keys(), entropy)?;
            let first = init.write_first()?;
            write_handshake(&mut stream, &first)?;
            let second = read_handshake(&mut stream)?;
            init.read_second(&second)?;
            let (third, unconfirmed) = init.write_third_and_finish()?;
            let code = unconfirmed.comparison_code();
            write_handshake(&mut stream, &third)?;
            stream.set_nonblocking(true).map_err(|_| VitaLanError::Io)?;
            Ok(Self {
                config,
                identity,
                stream,
                state: VitaLanState::PairingCodePending,
                pending: Some(PendingPairing { unconfirmed, code }),
                channel: None,
                inbound: Vec::with_capacity(4096),
                outbound: VecDeque::new(),
                offset: 0,
                queued: 0,
            })
        }
        pub fn begin_reconnect(
            config: VitaLanConfig,
            identity: VitaPeerIdentity,
            entropy: HandshakeEntropy,
            client: &mut VitaMissionControl,
        ) -> Result<Self, VitaLanError> {
            config.validate()?;
            let public = identity.server_public_key.ok_or(VitaLanError::State)?;
            let peer = PeerRecord {
                public_key: public,
                role: VITA_PAIRED_ROLE,
                revoked: false,
            };
            let mut stream = connect(config)?;
            blocking_write(&mut stream, &selector(2))?;
            let mut init = IkInitiator::with_entropy(&identity.keys(), peer, entropy)?;
            let first = init.write_first()?;
            write_handshake(&mut stream, &first)?;
            let second = read_handshake(&mut stream)?;
            let channel = init.read_second_and_finish(&second)?;
            stream.set_nonblocking(true).map_err(|_| VitaLanError::Io)?;
            let mut value = Self {
                config,
                identity,
                stream,
                state: VitaLanState::Active,
                pending: None,
                channel: Some(channel),
                inbound: Vec::with_capacity(4096),
                outbound: VecDeque::new(),
                offset: 0,
                queued: 0,
            };
            value.start_kps1(client)?;
            Ok(value)
        }
        pub const fn state(&self) -> VitaLanState {
            self.state
        }
        pub const fn identity(&self) -> VitaPeerIdentity {
            self.identity
        }
        pub fn comparison_code(&self) -> Option<ComparisonCode> {
            self.pending.as_ref().map(|p| p.code)
        }
        pub fn confirm_pairing(
            &mut self,
            code: ComparisonCode,
            client: &mut VitaMissionControl,
        ) -> Result<(), VitaLanError> {
            if self.state != VitaLanState::PairingCodePending {
                return Err(VitaLanError::State);
            }
            let pending = self.pending.take().ok_or(VitaLanError::State)?;
            if pending.code != code {
                return Err(VitaLanError::Handshake(
                    NoiseTransportError::ComparisonMismatch,
                ));
            }
            let channel = PeerRegistry::default().confirm_pairing(
                pending.unconfirmed,
                code,
                VITA_PAIRED_ROLE,
            )?;
            self.identity.server_public_key = Some(channel.peer().public_key);
            self.channel = Some(channel);
            self.state = VitaLanState::Active;
            self.start_kps1(client)
        }
        pub fn tick(&mut self, client: &mut VitaMissionControl) -> Result<(), VitaLanError> {
            if !matches!(
                self.state,
                VitaLanState::Active | VitaLanState::Stale | VitaLanState::ResyncRequired
            ) {
                return Ok(());
            }
            self.flush()?;
            let mut scratch = [0; 4096];
            loop {
                match self.stream.read(&mut scratch) {
                    Ok(0) => {
                        self.state = VitaLanState::Closed;
                        client.mark_closed();
                        return Err(VitaLanError::Io);
                    }
                    Ok(n) => {
                        if self.inbound.len() + n > MAX_INBOUND_BYTES {
                            return self.fail(client, VitaLanError::PacketLength);
                        }
                        self.inbound.extend_from_slice(&scratch[..n]);
                        self.consume(client)?
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        self.state = VitaLanState::Stale;
                        client.mark_disconnected();
                        return Err(VitaLanError::Io);
                    }
                }
            }
            Ok(())
        }
        pub fn submit_intent(
            &mut self,
            client: &mut VitaMissionControl,
            intent: ksa64_presentation::PresentationActionIntent,
        ) -> Result<(), VitaLanError> {
            if self.state != VitaLanState::Active {
                return Err(VitaLanError::State);
            }
            let mut bytes = vec![0; 512];
            let length = client.encode_action_intent(intent, &mut bytes)?;
            bytes.truncate(length);
            self.queue(bytes)
        }
        /// Requests fresh role-filtered publications without changing authority.
        pub fn request_publication(
            &mut self,
            client: &mut VitaMissionControl,
        ) -> Result<(), VitaLanError> {
            if self.state != VitaLanState::Active {
                return Err(VitaLanError::State);
            }
            let mut frame = vec![0_u8; 256];
            let length =
                client.encode_replay_control(PresentationCursors::default(), &mut frame)?;
            frame.truncate(length);
            self.queue(frame)
        }

        pub fn request_resync(
            &mut self,
            client: &mut VitaMissionControl,
        ) -> Result<(), VitaLanError> {
            if self.state != VitaLanState::Active {
                return Err(VitaLanError::State);
            }
            client.reset_for_resync();
            self.request_publication(client)
        }
        fn start_kps1(&mut self, client: &mut VitaMissionControl) -> Result<(), VitaLanError> {
            self.channel
                .as_mut()
                .ok_or(VitaLanError::State)?
                .bind_kps1_session(self.config.session_nonce, 1, 1)?;
            client.reserve_paired_handshake_sequence();
            self.queue_payload(
                PresentationMessageKind::HandshakeRequest,
                PresentationPayload::HandshakeRequest(PresentationHandshake {
                    role: VITA_PAIRED_ROLE,
                    client_instance: 0,
                    capability_mask: 0,
                    cursors: PresentationCursors::default(),
                }),
                1,
                1,
            )
        }
        fn queue_payload(
            &mut self,
            kind: PresentationMessageKind,
            payload: PresentationPayload,
            correlation: u64,
            sequence: u64,
        ) -> Result<(), VitaLanError> {
            let payload =
                encode_typed_payload(&payload, VITA_PAIRED_ROLE).map_err(VitaClientError::from)?;
            let header = Kps1Header {
                kind,
                flags: 0,
                session_nonce: self.config.session_nonce,
                sequence,
                correlation_id: correlation,
                payload_length: payload.len() as u32,
            };
            let mut frame = vec![0; KPS1_HEADER_LENGTH + payload.len()];
            write_kps1_frame(header, &payload, &mut frame).map_err(VitaClientError::from)?;
            self.queue(frame)
        }
        fn queue(&mut self, frame: Vec<u8>) -> Result<(), VitaLanError> {
            let packets = self
                .channel
                .as_mut()
                .ok_or(VitaLanError::State)?
                .seal_kps1(&frame)?;
            let bytes: usize = packets.iter().map(Vec::len).sum();
            if self.outbound.len() + packets.len() > MAX_QUEUED_PACKETS
                || self.queued + bytes > MAX_QUEUED_BYTES
            {
                return Err(VitaLanError::QueueFull);
            }
            self.queued += bytes;
            self.outbound.extend(packets);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), VitaLanError> {
            while let Some(packet) = self.outbound.front() {
                match self.stream.write(&packet[self.offset..]) {
                    Ok(0) => return Err(VitaLanError::Io),
                    Ok(n) => {
                        self.offset += n;
                        if self.offset == packet.len() {
                            let done = self.outbound.pop_front().ok_or(VitaLanError::State)?;
                            self.queued = self.queued.saturating_sub(done.len());
                            self.offset = 0
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(VitaLanError::Io),
                }
            }
            Ok(())
        }
        fn consume(&mut self, client: &mut VitaMissionControl) -> Result<(), VitaLanError> {
            loop {
                if self.inbound.len() < 4 {
                    return Ok(());
                }
                let length = u32::from_be_bytes(self.inbound[..4].try_into().unwrap()) as usize;
                if length == 0 || length > MAX_NOISE_CIPHERTEXT_LENGTH {
                    return self.fail(client, VitaLanError::PacketLength);
                }
                let total = 4 + length;
                if self.inbound.len() < total {
                    return Ok(());
                }
                let packet: Vec<u8> = self.inbound.drain(..total).collect();
                if let Some(frame) = self
                    .channel
                    .as_mut()
                    .ok_or(VitaLanError::State)?
                    .open_packet(&packet)?
                {
                    match client.receive_kps1(&frame) {
                        Ok(()) => {}
                        Err(VitaClientError::ResyncRequired) => {
                            self.state = VitaLanState::ResyncRequired;
                            return Ok(());
                        }
                        Err(error) => return self.fail(client, VitaLanError::Protocol(error)),
                    }
                }
            }
        }
        fn fail<T>(
            &mut self,
            client: &mut VitaMissionControl,
            error: VitaLanError,
        ) -> Result<T, VitaLanError> {
            self.state = VitaLanState::Closed;
            client.mark_closed();
            Err(error)
        }
    }
    /// Fixed eight-byte broker selector, duplicated here because constrained
    /// targets link the broker's no_std crypto core but not its host listener.
    fn selector(mode: u8) -> [u8; 8] {
        [b'K', b'S', b'L', b'1', 0, 1, mode, 0]
    }

    /// Small testable wrapper retained for the XX vector and local-code flow.
    pub struct VitaPairingInitiator {
        initiator: Option<XxInitiator>,
        unconfirmed: Option<ksa64_session_broker::UnconfirmedNoiseChannel>,
    }
    impl VitaPairingInitiator {
        pub fn begin(
            private_key: [u8; 32],
            public_key: [u8; 32],
            entropy: [u8; 32],
        ) -> Result<(Self, Vec<u8>), NoiseTransportError> {
            let keys = StaticNoiseKeypair::from_parts(private_key, public_key);
            let mut initiator =
                XxInitiator::with_entropy(&keys, HandshakeEntropy::from_bytes(entropy))?;
            let first = initiator.write_first()?;
            Ok((
                Self {
                    initiator: Some(initiator),
                    unconfirmed: None,
                },
                first,
            ))
        }
        pub fn accept_server_response(
            &mut self,
            response: &[u8],
        ) -> Result<Vec<u8>, NoiseTransportError> {
            let mut initiator = self
                .initiator
                .take()
                .ok_or(NoiseTransportError::HandshakeOrder)?;
            initiator.read_second(response)?;
            let (third, channel) = initiator.write_third_and_finish()?;
            self.unconfirmed = Some(channel);
            Ok(third)
        }
        pub fn comparison_code(&self) -> Result<ComparisonCode, NoiseTransportError> {
            self.unconfirmed
                .as_ref()
                .map(ksa64_session_broker::UnconfirmedNoiseChannel::comparison_code)
                .ok_or(NoiseTransportError::HandshakeOrder)
        }
        pub fn confirm(
            mut self,
            code: ComparisonCode,
            role: PresentationRole,
        ) -> Result<AuthenticatedNoiseChannel, NoiseTransportError> {
            PeerRegistry::default().confirm_pairing(
                self.unconfirmed
                    .take()
                    .ok_or(NoiseTransportError::HandshakeOrder)?,
                code,
                role,
            )
        }
    }

    fn connect(config: VitaLanConfig) -> Result<TcpStream, VitaLanError> {
        let stream = TcpStream::connect_timeout(
            &config.server,
            Duration::from_millis(config.connect_timeout_millis),
        )
        .map_err(|_| VitaLanError::Io)?;
        stream
            .set_read_timeout(Some(Duration::from_millis(config.handshake_timeout_millis)))
            .map_err(|_| VitaLanError::Io)?;
        stream
            .set_write_timeout(Some(Duration::from_millis(config.handshake_timeout_millis)))
            .map_err(|_| VitaLanError::Io)?;
        Ok(stream)
    }
    fn blocking_write(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), VitaLanError> {
        stream.write_all(bytes).map_err(|_| VitaLanError::Io)
    }
    fn write_handshake(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), VitaLanError> {
        if bytes.is_empty() || bytes.len() > MAX_HANDSHAKE_MESSAGE_LENGTH {
            return Err(VitaLanError::PacketLength);
        }
        blocking_write(stream, &(bytes.len() as u32).to_be_bytes())?;
        blocking_write(stream, bytes)
    }
    fn read_handshake(stream: &mut TcpStream) -> Result<Vec<u8>, VitaLanError> {
        let mut length = [0; 4];
        stream
            .read_exact(&mut length)
            .map_err(|_| VitaLanError::Timeout)?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 || length > MAX_HANDSHAKE_MESSAGE_LENGTH {
            return Err(VitaLanError::PacketLength);
        }
        let mut value = vec![0; length];
        stream
            .read_exact(&mut value)
            .map_err(|_| VitaLanError::Timeout)?;
        Ok(value)
    }
    impl fmt::Display for VitaLanState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{:?}", self)
        }
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
    #[cfg(feature = "vita-target")]
    #[test]
    fn vita_pairing_wrapper_uses_shared_noise_xx_and_local_confirmation() {
        use crate::paired_transport::VitaPairingInitiator;
        use ksa64_session_broker::{
            HandshakeEntropy, PeerRegistry, StaticNoiseKeypair, XxResponder,
        };
        let client_private = [
            0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
            0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
            0x1d, 0xb9, 0x2c, 0x2a,
        ];
        let client_public = [
            0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
            0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
            0xaa, 0x9b, 0x4e, 0x6a,
        ];
        let server_keys = StaticNoiseKeypair::from_parts(
            [
                0x5d, 0xab, 0x08, 0x7e, 0x62, 0x4a, 0x8a, 0x4b, 0x79, 0xe1, 0x7f, 0x8b, 0x83, 0x80,
                0x0e, 0xe6, 0x6f, 0x3b, 0xb1, 0x29, 0x26, 0x18, 0xb6, 0xfd, 0x1c, 0x2f, 0x8b, 0x27,
                0xff, 0x88, 0xe0, 0xeb,
            ],
            [
                0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4,
                0x35, 0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14,
                0x6f, 0x88, 0x2b, 0x4f,
            ],
        );
        let (mut client, first) =
            VitaPairingInitiator::begin(client_private, client_public, [0x11; 32]).unwrap();
        let mut server =
            XxResponder::with_entropy(&server_keys, HandshakeEntropy::from_bytes([0x22; 32]))
                .unwrap();
        server.read_first(&first).unwrap();
        let second = server.write_second().unwrap();
        let third = client.accept_server_response(&second).unwrap();
        let server_pending = server.read_third_and_finish(&third).unwrap();
        let code = client.comparison_code().unwrap();
        assert_eq!(code, server_pending.comparison_code());
        let client_channel = client
            .confirm(code, PresentationRole::GuidedOperator)
            .unwrap();
        let server_channel = PeerRegistry::default()
            .confirm_pairing(server_pending, code, PresentationRole::GuidedOperator)
            .unwrap();
        assert_eq!(client_channel.peer().role, PresentationRole::GuidedOperator);
        assert_eq!(server_channel.peer().role, PresentationRole::GuidedOperator);
    }

    #[cfg(feature = "vita-target")]
    #[test]
    fn vita_peer_identity_has_bounded_crc_protected_storage() {
        use crate::paired_transport::VitaPeerIdentity;
        let mut identity = VitaPeerIdentity::from_parts([0x11; 32], [0x22; 32]);
        identity.server_public_key = Some([0x33; 32]);
        let mut bytes = [0_u8; VitaPeerIdentity::ENCODED_LENGTH];
        assert_eq!(identity.encode(&mut bytes).unwrap(), bytes.len());
        assert_eq!(VitaPeerIdentity::decode(&bytes).unwrap(), identity);
        bytes[76] ^= 1;
        assert_eq!(
            VitaPeerIdentity::decode(&bytes),
            Err(crate::paired_transport::VitaLanError::Persistence)
        );
    }
}
