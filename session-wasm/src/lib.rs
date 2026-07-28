use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
};

use ksa64_interface::phase11::OperationalRole;
use ksa64_presentation::{
    encode_typed_payload, write_kps1_frame, ActionProposalView, Kps1Header,
    PresentationActionIntent, PresentationActionOperation, PresentationPace, PresentationPayload,
    PresentationRole, PresentationSession, SealedEvidenceChunk,
    KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH, KPS1_EVIDENCE_OBJECT_MAX_LENGTH, KPS1_FLAG_RESPONSE,
};
use ksa64_session::{
    phase11_live::MissionSessionError,
    phase12b_live::{
        BRANCH_COMMIT_RELEASE, BRANCH_STAGE_RELEASE, UPDATE_COMMIT_RELEASE, UPDATE_STAGE_RELEASE,
    },
    presentation_adapter::{FullMissionPresentationSession, PresentationSessionError},
    presentation_replay::{
        PresentationReplayError, VerifiedPresentationReplay, VerifiedReplayMetadata,
    },
};

pub const KSW1_MAGIC: [u8; 4] = *b"KSW1";
pub const KSR1_MAGIC: [u8; 4] = *b"KSR1";
pub const KPW1_MAGIC: [u8; 4] = *b"KPW1";
pub const WORKER_ABI_MAJOR: u16 = 1;
pub const COMMAND_LENGTH: usize = 32;
pub const RESULT_HEADER_LENGTH: usize = 12;
pub const REPLAY_INFO_LENGTH: usize = 72;
pub const REPLAY_READ_MAX_FRAMES: u32 = 256;
pub const REPLAY_READ_MAX_BYTES: usize = 1024 * 1024;
pub const EXPECTED_RELEASE_EPOCH: u32 = 21_591;
pub const EXPECTED_EVIDENCE_LENGTH: usize = 2_911_464;
pub const EXPECTED_EVIDENCE_SHA256: &str =
    "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4";

/// Fixed-width worker commands. Inputs and all output records are little-endian.
/// This is a worker boundary, not a canonical artifact. Poll returns a KPW1
/// record list containing strict KPS1 publications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum WorkerCommandKind {
    Catalog = 1,
    Start = 2,
    Prepare = 3,
    Pace = 4,
    Advance = 5,
    Poll = 6,
    Action = 7,
    Evidence = 8,
    RunScripted = 9,
    Destroy = 10,
    Summary = 11,
    ReplayInfo = 12,
    ReplayRead = 13,
}

impl WorkerCommandKind {
    fn from_raw(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Catalog),
            2 => Some(Self::Start),
            3 => Some(Self::Prepare),
            4 => Some(Self::Pace),
            5 => Some(Self::Advance),
            6 => Some(Self::Poll),
            7 => Some(Self::Action),
            8 => Some(Self::Evidence),
            9 => Some(Self::RunScripted),
            10 => Some(Self::Destroy),
            11 => Some(Self::Summary),
            12 => Some(Self::ReplayInfo),
            13 => Some(Self::ReplayRead),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerCommand {
    pub kind: WorkerCommandKind,
    pub arg0: u32,
    pub arg1: u32,
    pub arg2: u32,
    pub arg3: u32,
    pub arg4: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerError {
    Length,
    Magic,
    Version,
    Kind,
    Reserved,
    Lifecycle,
    Role,
    Value,
    Session,
    Presentation,
    Incomplete,
    Panic,
    Replay,
}

impl WorkerError {
    const fn code(self) -> i16 {
        match self {
            Self::Length => -1,
            Self::Magic => -2,
            Self::Version => -3,
            Self::Kind => -4,
            Self::Reserved => -5,
            Self::Lifecycle => -6,
            Self::Role => -7,
            Self::Value => -8,
            Self::Session => -9,
            Self::Presentation => -10,
            Self::Incomplete => -11,
            Self::Panic => -12,
            Self::Replay => -13,
        }
    }
}

impl From<PresentationReplayError> for WorkerError {
    fn from(_: PresentationReplayError) -> Self {
        Self::Replay
    }
}

impl From<PresentationSessionError> for WorkerError {
    fn from(value: PresentationSessionError) -> Self {
        match value {
            PresentationSessionError::Authority(MissionSessionError::Lifecycle) => Self::Lifecycle,
            PresentationSessionError::Authority(_) => Self::Session,
            PresentationSessionError::Intent(_)
            | PresentationSessionError::ActionSequence
            | PresentationSessionError::Proposal => Self::Value,
            PresentationSessionError::Retention(_) => Self::Presentation,
        }
    }
}

pub fn parse_command(bytes: &[u8]) -> Result<WorkerCommand, WorkerError> {
    if bytes.len() != COMMAND_LENGTH {
        return Err(WorkerError::Length);
    }
    if bytes[..4] != KSW1_MAGIC {
        return Err(WorkerError::Magic);
    }
    if get_u16(bytes, 4) != WORKER_ABI_MAJOR {
        return Err(WorkerError::Version);
    }
    if bytes[28..32] != [0; 4] {
        return Err(WorkerError::Reserved);
    }
    Ok(WorkerCommand {
        kind: WorkerCommandKind::from_raw(get_u16(bytes, 6)).ok_or(WorkerError::Kind)?,
        arg0: get_u32(bytes, 8),
        arg1: get_u32(bytes, 12),
        arg2: get_u32(bytes, 16),
        arg3: get_u32(bytes, 20),
        arg4: get_u32(bytes, 24),
    })
}

pub fn encode_command(command: WorkerCommand) -> [u8; COMMAND_LENGTH] {
    let mut bytes = [0; COMMAND_LENGTH];
    bytes[..4].copy_from_slice(&KSW1_MAGIC);
    put_u16(&mut bytes, 4, WORKER_ABI_MAJOR);
    put_u16(&mut bytes, 6, command.kind as u16);
    put_u32(&mut bytes, 8, command.arg0);
    put_u32(&mut bytes, 12, command.arg1);
    put_u32(&mut bytes, 16, command.arg2);
    put_u32(&mut bytes, 20, command.arg3);
    put_u32(&mut bytes, 24, command.arg4);
    bytes
}

/// Owns exactly one local deterministic session. This is also the native test
/// facade; the wasm FFI delegates to it through a worker-local cell.
pub struct WasmAuthority {
    session: Option<FullMissionPresentationSession>,
    evidence: Option<Vec<u8>>,
    role: PresentationRole,
    session_nonce: u64,
    server_sequence: u64,
    incomplete: bool,
    completed_summary: Option<(u32, u32)>,
    evidence_published: bool,
    replay: Option<VerifiedPresentationReplay>,
    last_result: Vec<u8>,
}

impl Default for WasmAuthority {
    fn default() -> Self {
        Self {
            session: None,
            evidence: None,
            role: PresentationRole::Observer,
            session_nonce: 0,
            server_sequence: 1,
            incomplete: false,
            completed_summary: None,
            evidence_published: false,
            replay: None,
            last_result: Vec::new(),
        }
    }
}

impl WasmAuthority {
    pub fn execute(&mut self, input: &[u8]) -> Vec<u8> {
        match parse_command(input).and_then(|command| self.dispatch(command)) {
            Ok(payload) => encode_result(0, &payload),
            Err(error) => encode_result(error.code(), &[]),
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.incomplete
    }

    fn dispatch(&mut self, command: WorkerCommand) -> Result<Vec<u8>, WorkerError> {
        if self.incomplete && command.kind != WorkerCommandKind::Destroy {
            return Err(WorkerError::Incomplete);
        }
        match command.kind {
            WorkerCommandKind::Catalog => Ok(CATALOG_JSON.as_bytes().to_vec()),
            WorkerCommandKind::Start => {
                if self.session.is_some() || self.replay.is_some() {
                    return Err(WorkerError::Lifecycle);
                }
                let role = role_from_raw(command.arg0 as u8)?;
                let session_nonce = u64::from(command.arg1) | (u64::from(command.arg2) << 32);
                if session_nonce == 0 {
                    return Err(WorkerError::Value);
                }
                self.role = PresentationRole::from(role);
                self.session_nonce = session_nonce;
                self.server_sequence = 1;
                self.session = Some(FullMissionPresentationSession::new(role)?);
                self.evidence = None;
                self.completed_summary = None;
                self.evidence_published = false;
                self.replay = None;
                Ok(Vec::new())
            }
            WorkerCommandKind::Prepare => {
                self.session_mut()?.prepare()?;
                Ok(Vec::new())
            }
            WorkerCommandKind::Pace => {
                if self.replay.is_some() {
                    return Err(WorkerError::Lifecycle);
                }
                let pace =
                    PresentationPace::from_raw(command.arg0 as u8).ok_or(WorkerError::Value)?;
                self.session_mut()?.set_pace(pace)?;
                Ok(Vec::new())
            }
            WorkerCommandKind::Advance => {
                let advanced = self.session_mut()?.advance_bounded(command.arg0)?;
                Ok(advanced.to_le_bytes().to_vec())
            }
            WorkerCommandKind::Poll => self.poll(),
            WorkerCommandKind::Action => {
                if self.replay.is_some() {
                    return Err(WorkerError::Lifecycle);
                }
                self.action(command)
            }
            WorkerCommandKind::Evidence => self.evidence_bytes(),
            WorkerCommandKind::RunScripted => self.run_scripted(),
            WorkerCommandKind::Destroy => {
                self.session = None;
                self.evidence = None;
                self.completed_summary = None;
                self.evidence_published = false;
                self.replay = None;
                self.session_nonce = 0;
                self.server_sequence = 1;
                self.incomplete = false;
                Ok(Vec::new())
            }
            WorkerCommandKind::Summary => self.summary(),
            WorkerCommandKind::ReplayInfo => self.replay_info(),
            WorkerCommandKind::ReplayRead => self.replay_read(command),
        }
    }

    fn session_mut(&mut self) -> Result<&mut FullMissionPresentationSession, WorkerError> {
        self.session.as_mut().ok_or(WorkerError::Session)
    }

    /// Transactionally validates opaque KSB11 evidence and prepares a read-only,
    /// role-filtered KPS1 replay. No replay state is installed on rejection.
    pub fn open_replay(
        &mut self,
        input: &[u8],
        role_raw: u8,
        session_nonce: u64,
    ) -> Result<Vec<u8>, WorkerError> {
        if self.incomplete {
            return Err(WorkerError::Incomplete);
        }
        if self.session.is_some() || self.replay.is_some() || self.evidence.is_some() {
            return Err(WorkerError::Lifecycle);
        }
        if input.len() > KPS1_EVIDENCE_OBJECT_MAX_LENGTH as usize {
            return Err(WorkerError::Value);
        }
        let role = replay_role_from_raw(role_raw)?;
        if session_nonce == 0 {
            return Err(WorkerError::Value);
        }
        let replay = VerifiedPresentationReplay::open(input, role, session_nonce)?;
        self.role = role;
        self.session_nonce = session_nonce;
        self.replay = Some(replay);
        self.replay_info()
    }

    fn replay_info(&self) -> Result<Vec<u8>, WorkerError> {
        let replay = self.replay.as_ref().ok_or(WorkerError::Lifecycle)?;
        Ok(encode_replay_info(replay.metadata()))
    }

    fn replay_read(&self, command: WorkerCommand) -> Result<Vec<u8>, WorkerError> {
        let replay = self.replay.as_ref().ok_or(WorkerError::Lifecycle)?;
        let first = u64::from(command.arg0) | (u64::from(command.arg1) << 32);
        let maximum_frames = command.arg2;
        let byte_budget = if command.arg3 == 0 {
            REPLAY_READ_MAX_BYTES
        } else {
            usize::try_from(command.arg3).map_err(|_| WorkerError::Value)?
        };
        if command.arg4 != 0
            || maximum_frames == 0
            || maximum_frames > REPLAY_READ_MAX_FRAMES
            || !(64..=REPLAY_READ_MAX_BYTES).contains(&byte_budget)
        {
            return Err(WorkerError::Value);
        }
        let frames = replay.kps1_frames();
        let start = usize::try_from(first).map_err(|_| WorkerError::Value)?;
        if start > frames.len() {
            return Err(WorkerError::Value);
        }
        let mut end = start;
        let mut encoded_length = 8usize;
        while end < frames.len() && end - start < maximum_frames as usize {
            let added = 4usize
                .checked_add(frames[end].len())
                .ok_or(WorkerError::Value)?;
            let next_length = encoded_length
                .checked_add(added)
                .ok_or(WorkerError::Value)?;
            if next_length > byte_budget {
                if end == start {
                    return Err(WorkerError::Value);
                }
                break;
            }
            encoded_length = next_length;
            end += 1;
        }
        Ok(encode_publication_bundle(&frames[start..end]))
    }

    fn poll(&mut self) -> Result<Vec<u8>, WorkerError> {
        let role = self.role;
        let session = self.session.as_ref().ok_or(WorkerError::Session)?;
        let mut payloads = Vec::new();
        payloads.push(PresentationPayload::Snapshot(session.latest_snapshot()));
        if let Some(value) = session.current_procedure() {
            payloads.push(PresentationPayload::Procedure(value));
        }
        if let Some(value) = session.current_disposition() {
            payloads.push(PresentationPayload::Disposition(value));
        }
        for value in session.current_prediction_paths() {
            payloads.push(PresentationPayload::PredictionPath(value));
        }
        payloads.push(PresentationPayload::TransportStatus(
            session.transport_status(),
        ));
        let evidence_metadata = session.finalization_evidence();
        let evidence_bytes = if evidence_metadata.is_some() && !self.evidence_published {
            session.sealed_evidence_bytes().map(<[u8]>::to_vec)
        } else {
            None
        };
        if let Some(value) = evidence_metadata {
            payloads.push(PresentationPayload::EvidenceMetadata(value));
            if let Some(bytes) = evidence_bytes.as_ref() {
                for (chunk_index, chunk) in bytes
                    .chunks(KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH)
                    .enumerate()
                {
                    payloads.push(PresentationPayload::EvidenceChunk(SealedEvidenceChunk {
                        evidence_identity: value.evidence_identity,
                        chunk_index: chunk_index as u32,
                        chunk_count: value.chunk_count,
                        logical_offset: (chunk_index * KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH) as u64,
                        bytes: chunk.to_vec(),
                    }));
                }
            }
        }
        if let Some(value) = session.current_action_proposal() {
            payloads.push(PresentationPayload::ActionProposal(value));
        }
        let mut frames = Vec::new();
        for value in payloads {
            self.push_payload(&mut frames, value, 0, role)?;
        }
        if evidence_bytes.is_some() {
            self.evidence_published = true;
        }
        Ok(encode_publication_bundle(&frames))
    }

    fn push_payload(
        &mut self,
        frames: &mut Vec<Vec<u8>>,
        payload: PresentationPayload,
        correlation_id: u64,
        role: PresentationRole,
    ) -> Result<(), WorkerError> {
        let bytes = encode_typed_payload(&payload, role).map_err(|error| {
            #[cfg(test)]
            eprintln!(
                "presentation payload {:?} failed to encode: {error:?}",
                payload.kind()
            );
            let _ = error;
            WorkerError::Presentation
        })?;
        let sequence = self.server_sequence;
        self.server_sequence = self
            .server_sequence
            .checked_add(1)
            .ok_or(WorkerError::Presentation)?;
        let header = Kps1Header {
            kind: payload.kind(),
            flags: KPS1_FLAG_RESPONSE,
            session_nonce: self.session_nonce,
            sequence,
            correlation_id,
            payload_length: bytes.len() as u32,
        };
        let mut frame = vec![0; 48 + bytes.len()];
        let written = write_kps1_frame(header, &bytes, &mut frame).map_err(|error| {
            #[cfg(test)]
            eprintln!(
                "presentation frame {:?} failed to encode ({} bytes): {error:?}",
                payload.kind(),
                bytes.len()
            );
            let _ = error;
            WorkerError::Presentation
        })?;
        frame.truncate(written);
        frames.push(frame);
        Ok(())
    }

    fn action(&mut self, command: WorkerCommand) -> Result<Vec<u8>, WorkerError> {
        let operation = action_from_raw(command.arg0 as u8)?;
        let role = self.role;
        let session = self.session_mut()?;
        let proposal: ActionProposalView = session
            .current_action_proposal()
            .ok_or(WorkerError::Value)?;
        let proposal_identity = if command.arg1 == 0 {
            proposal.proposal_identity
        } else {
            command.arg1
        };
        let load_identity = if command.arg2 == 0 {
            proposal.load_identity
        } else {
            command.arg2
        };
        let activation_epoch = if command.arg3 == 0 {
            proposal.activation_epoch
        } else {
            command.arg3
        };
        let receipt = session.submit_action(PresentationActionIntent {
            client_action_sequence: if command.arg4 == 0 {
                1
            } else {
                u64::from(command.arg4)
            },
            proposal_identity,
            expected_load_identity: load_identity,
            requested_activation_epoch: activation_epoch,
            operation,
        })?;
        let mut frames = Vec::new();
        self.push_payload(
            &mut frames,
            PresentationPayload::ActionReceipt(receipt),
            0,
            role,
        )?;
        Ok(encode_publication_bundle(&frames))
    }

    fn run_scripted(&mut self) -> Result<Vec<u8>, WorkerError> {
        if self.session.is_some() || self.evidence.is_some() || self.replay.is_some() {
            return Err(WorkerError::Lifecycle);
        }
        // Drive the real presentation adapter and its stage-validate-commit
        // surface. Review is presentation-only; Stage and Commit are exactly the
        // four accepted canonical operations in the frozen transcript.
        let mut session = FullMissionPresentationSession::new(OperationalRole::ScriptedOperator)?;
        session.prepare()?;
        session.set_pace(PresentationPace::Fast)?;
        let mut client_sequence = 1_u64;
        loop {
            let snapshot = session.latest_snapshot();
            if snapshot.lifecycle == ksa64_presentation::PresentationLifecycle::Completed {
                break;
            }
            match snapshot.release_epoch {
                UPDATE_STAGE_RELEASE | BRANCH_STAGE_RELEASE => {
                    let proposal = session
                        .current_action_proposal()
                        .ok_or(WorkerError::Session)?;
                    let review = PresentationActionIntent {
                        client_action_sequence: client_sequence,
                        proposal_identity: proposal.proposal_identity,
                        expected_load_identity: proposal.load_identity,
                        requested_activation_epoch: proposal.activation_epoch,
                        operation: PresentationActionOperation::Review,
                    };
                    session.submit_action(review)?;
                    client_sequence = client_sequence.checked_add(1).ok_or(WorkerError::Session)?;
                    let stage = PresentationActionIntent {
                        client_action_sequence: client_sequence,
                        operation: PresentationActionOperation::Stage,
                        ..review
                    };
                    session.submit_action(stage)?;
                    client_sequence = client_sequence.checked_add(1).ok_or(WorkerError::Session)?;
                }
                UPDATE_COMMIT_RELEASE | BRANCH_COMMIT_RELEASE => {
                    let proposal = session
                        .current_action_proposal()
                        .ok_or(WorkerError::Session)?;
                    session.submit_action(PresentationActionIntent {
                        client_action_sequence: client_sequence,
                        proposal_identity: proposal.proposal_identity,
                        expected_load_identity: proposal.load_identity,
                        requested_activation_epoch: proposal.activation_epoch,
                        operation: PresentationActionOperation::Commit,
                    })?;
                    client_sequence = client_sequence.checked_add(1).ok_or(WorkerError::Session)?;
                }
                _ => {}
            }
            let next_decision = [
                UPDATE_STAGE_RELEASE,
                UPDATE_COMMIT_RELEASE,
                BRANCH_STAGE_RELEASE,
                BRANCH_COMMIT_RELEASE,
            ]
            .into_iter()
            .filter(|epoch| *epoch > snapshot.release_epoch)
            .min()
            .unwrap_or(EXPECTED_RELEASE_EPOCH);
            let budget = next_decision
                .saturating_sub(snapshot.release_epoch)
                .clamp(1, 256);
            session.advance_bounded(budget)?;
        }
        let final_snapshot = session.latest_snapshot();
        if final_snapshot.release_epoch != EXPECTED_RELEASE_EPOCH
            || final_snapshot.action_count != 4
        {
            return Err(WorkerError::Session);
        }
        let bytes = session
            .sealed_evidence_bytes()
            .ok_or(WorkerError::Session)?
            .to_vec();
        if bytes.len() != EXPECTED_EVIDENCE_LENGTH {
            return Err(WorkerError::Session);
        }
        self.completed_summary = Some((final_snapshot.release_epoch, final_snapshot.action_count));
        self.evidence = Some(bytes.clone());
        self.session = Some(session);
        Ok(bytes)
    }

    fn summary(&self) -> Result<Vec<u8>, WorkerError> {
        let (releases, actions) = self.completed_state()?;
        let evidence = self.evidence_slice()?;
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&releases.to_le_bytes());
        bytes.extend_from_slice(&actions.to_le_bytes());
        bytes.extend_from_slice(&(evidence.len() as u32).to_le_bytes());
        Ok(bytes)
    }

    fn completed_state(&self) -> Result<(u32, u32), WorkerError> {
        if let Some(summary) = self.completed_summary {
            return Ok(summary);
        }
        let snapshot = self
            .session
            .as_ref()
            .ok_or(WorkerError::Session)?
            .latest_snapshot();
        if snapshot.lifecycle != ksa64_presentation::PresentationLifecycle::Completed {
            return Err(WorkerError::Incomplete);
        }
        Ok((snapshot.release_epoch, snapshot.action_count))
    }

    fn evidence_slice(&self) -> Result<&[u8], WorkerError> {
        if let Some(evidence) = self.evidence.as_deref() {
            return Ok(evidence);
        }
        self.session
            .as_ref()
            .and_then(FullMissionPresentationSession::sealed_evidence_bytes)
            .ok_or(WorkerError::Incomplete)
    }

    fn evidence_bytes(&self) -> Result<Vec<u8>, WorkerError> {
        Ok(self.evidence_slice()?.to_vec())
    }
}

fn encode_replay_info(metadata: VerifiedReplayMetadata) -> Vec<u8> {
    let mut output = vec![0_u8; REPLAY_INFO_LENGTH];
    output[..4].copy_from_slice(b"KPRI");
    put_u16(&mut output, 4, WORKER_ABI_MAJOR);
    put_u16(&mut output, 6, REPLAY_INFO_LENGTH as u16);
    put_u64(&mut output, 8, metadata.frame_count);
    put_u32(&mut output, 16, metadata.session_definition_identity);
    put_u32(&mut output, 20, metadata.action_identity);
    put_u32(&mut output, 24, metadata.completed_evidence_identity);
    output[28] = metadata.role as u8;
    put_u64(&mut output, 32, metadata.session_nonce);
    output[40..72].copy_from_slice(&metadata.manifest_sha256);
    output
}

fn encode_publication_bundle(frames: &[Vec<u8>]) -> Vec<u8> {
    let length = 8 + frames.iter().map(|frame| 4 + frame.len()).sum::<usize>();
    let mut output = Vec::with_capacity(length);
    output.extend_from_slice(&KPW1_MAGIC);
    output.extend_from_slice(&WORKER_ABI_MAJOR.to_le_bytes());
    output.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    for frame in frames {
        output.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        output.extend_from_slice(frame);
    }
    output
}

fn encode_result(status: i16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RESULT_HEADER_LENGTH + payload.len());
    bytes.extend_from_slice(&KSR1_MAGIC);
    bytes.extend_from_slice(&WORKER_ABI_MAJOR.to_le_bytes());
    bytes.extend_from_slice(&status.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub fn parse_result(input: &[u8]) -> Result<(i16, &[u8]), WorkerError> {
    if input.len() < RESULT_HEADER_LENGTH {
        return Err(WorkerError::Length);
    }
    if input[..4] != KSR1_MAGIC {
        return Err(WorkerError::Magic);
    }
    if get_u16(input, 4) != WORKER_ABI_MAJOR {
        return Err(WorkerError::Version);
    }
    let payload_length = get_u32(input, 8) as usize;
    if input.len() != RESULT_HEADER_LENGTH + payload_length {
        return Err(WorkerError::Length);
    }
    Ok((i16::from_le_bytes([input[6], input[7]]), &input[12..]))
}

fn role_from_raw(value: u8) -> Result<OperationalRole, WorkerError> {
    match value {
        1 => Ok(OperationalRole::Observer),
        2 => Ok(OperationalRole::GuidedOperator),
        3 => Ok(OperationalRole::FlightController),
        4 => Ok(OperationalRole::FlightSoftwareEngineer),
        5 => Ok(OperationalRole::SimDirector),
        6 => Ok(OperationalRole::ScriptedOperator),
        _ => Err(WorkerError::Role),
    }
}

fn replay_role_from_raw(value: u8) -> Result<PresentationRole, WorkerError> {
    match value {
        1 => Ok(PresentationRole::Observer),
        2 => Ok(PresentationRole::GuidedOperator),
        3 => Ok(PresentationRole::FlightController),
        4 => Ok(PresentationRole::FlightSoftwareEngineer),
        5 => Ok(PresentationRole::SimDirector),
        _ => Err(WorkerError::Role),
    }
}

fn action_from_raw(value: u8) -> Result<PresentationActionOperation, WorkerError> {
    match value {
        1 => Ok(PresentationActionOperation::Review),
        2 => Ok(PresentationActionOperation::Stage),
        3 => Ok(PresentationActionOperation::Commit),
        4 => Ok(PresentationActionOperation::Cancel),
        _ => Err(WorkerError::Value),
    }
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

const CATALOG_JSON: &str = r#"{"schema":"ksa64.product-catalog.v1","worker":"ksa64-session-wasm.v1","experiences":[{"id":"ksa-g10r.operations","scenario":"gnss-loss","role_filtered":true,"authority":"rust","evidence":"KSB11"}]}"#;

thread_local! { static WORKER: RefCell<WasmAuthority> = RefCell::new(WasmAuthority::default()); }

/// Allocate command memory in wasm linear memory. Return it with dealloc using
/// the same length.
#[no_mangle]
pub extern "C" fn ksa64_wasm_alloc(length: usize) -> *mut u8 {
    let mut bytes = vec![0_u8; length];
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}

/// # Safety
///
/// `pointer` must be the exact allocation returned by `ksa64_wasm_alloc`, and
/// `length` must be the exact length passed to that allocator. It must be freed
/// at most once.
#[no_mangle]
pub unsafe extern "C" fn ksa64_wasm_dealloc(pointer: *mut u8, length: usize) {
    if !pointer.is_null() {
        drop(Vec::from_raw_parts(pointer, 0, length));
    }
}

/// Submit a fixed KSW1 command. The result remains valid until the next submit
/// or clear_result; clients obtain it through result_ptr and result_len.
///
/// # Safety
///
/// When `length` is nonzero, `pointer` must designate a readable `length` byte
/// region in wasm linear memory for the entire call. The caller must not mutate
/// that region until this function returns.
#[no_mangle]
pub unsafe extern "C" fn ksa64_wasm_submit(pointer: *const u8, length: usize) -> i32 {
    if pointer.is_null() && length != 0 {
        return WorkerError::Length.code() as i32;
    }
    let input = if length == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(pointer, length)
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        WORKER.with(|worker| worker.borrow_mut().execute(input))
    }));
    match result {
        Ok(bytes) => {
            WORKER.with(|worker| worker.borrow_mut().last_result = bytes);
            0
        }
        Err(_) => {
            WORKER.with(|worker| {
                let mut worker = worker.borrow_mut();
                worker.incomplete = true;
                worker.last_result = encode_result(WorkerError::Panic.code(), &[]);
            });
            WorkerError::Panic.code() as i32
        }
    }
}

/// Validate and open an opaque KSB11 archive as a role-filtered replay.
///
/// The archive never leaves Rust again. Results use the existing KSR1 buffer;
/// ordinary validation rejection is reported there without poisoning the worker.
///
/// # Safety
///
/// When `length` is nonzero, `pointer` must designate a readable `length`
/// byte region for this call.
#[no_mangle]
pub unsafe extern "C" fn ksa64_wasm_open_replay(
    pointer: *const u8,
    length: usize,
    role: u32,
    nonce_low: u32,
    nonce_high: u32,
) -> i32 {
    let domain_result = if (pointer.is_null() && length != 0)
        || length > KPS1_EVIDENCE_OBJECT_MAX_LENGTH as usize
        || role > u32::from(u8::MAX)
    {
        Err(WorkerError::Value)
    } else {
        let input = if length == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(pointer, length)
        };
        let nonce = u64::from(nonce_low) | (u64::from(nonce_high) << 32);
        let result = catch_unwind(AssertUnwindSafe(|| {
            WORKER.with(|worker| worker.borrow_mut().open_replay(input, role as u8, nonce))
        }));
        match result {
            Ok(value) => value,
            Err(_) => {
                WORKER.with(|worker| worker.borrow_mut().incomplete = true);
                Err(WorkerError::Panic)
            }
        }
    };
    let return_code = domain_result
        .as_ref()
        .err()
        .copied()
        .filter(|error| *error == WorkerError::Panic)
        .map_or(0, |error| i32::from(error.code()));
    let encoded = match domain_result {
        Ok(payload) => encode_result(0, &payload),
        Err(error) => encode_result(error.code(), &[]),
    };
    WORKER.with(|worker| worker.borrow_mut().last_result = encoded);
    return_code
}

#[no_mangle]
pub extern "C" fn ksa64_wasm_result_ptr() -> *const u8 {
    WORKER.with(|worker| worker.borrow().last_result.as_ptr())
}
#[no_mangle]
pub extern "C" fn ksa64_wasm_result_len() -> usize {
    WORKER.with(|worker| worker.borrow().last_result.len())
}
#[no_mangle]
pub extern "C" fn ksa64_wasm_clear_result() {
    WORKER.with(|worker| worker.borrow_mut().last_result.clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(kind: WorkerCommandKind, values: [u32; 5]) -> [u8; COMMAND_LENGTH] {
        encode_command(WorkerCommand {
            kind,
            arg0: values[0],
            arg1: values[1],
            arg2: values[2],
            arg3: values[3],
            arg4: values[4],
        })
    }
    #[test]
    fn command_codec_rejects_reserved_bytes() {
        let mut bytes = command(WorkerCommandKind::Catalog, [0; 5]);
        bytes[31] = 1;
        assert_eq!(parse_command(&bytes), Err(WorkerError::Reserved));
    }
    #[test]
    fn live_start_requires_a_nonzero_caller_nonce() {
        let mut worker = WasmAuthority::default();
        let result = worker.execute(&command(WorkerCommandKind::Start, [2, 0, 0, 0, 0]));
        assert_eq!(parse_result(&result).unwrap().0, WorkerError::Value.code());
    }
    #[test]
    fn facade_exposes_role_filtered_kps1_poll() {
        let mut worker = WasmAuthority::default();
        assert_eq!(
            parse_result(&worker.execute(&command(
                WorkerCommandKind::Start,
                [2, 878133587, 1263747382, 0, 0]
            )))
            .unwrap()
            .0,
            0
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Prepare, [0; 5])))
                .unwrap()
                .0,
            0
        );
        let result = worker.execute(&command(WorkerCommandKind::Poll, [0; 5]));
        let (_, payload) = parse_result(&result).unwrap();
        assert_eq!(&payload[..4], b"KPW1");
        assert!(payload.len() > 16);
    }
    #[test]
    fn replay_rejection_is_transactional_and_destroy_recovers_incomplete_state() {
        let mut worker = WasmAuthority::default();
        assert_eq!(worker.open_replay(&[], 1, 7), Err(WorkerError::Replay));
        assert!(worker.replay.is_none());
        assert_eq!(worker.open_replay(&[], 0, 7), Err(WorkerError::Role));
        assert_eq!(worker.open_replay(&[], 1, 0), Err(WorkerError::Value));
        let read = worker.execute(&command(WorkerCommandKind::ReplayRead, [0, 0, 1, 64, 0]));
        assert_eq!(
            parse_result(&read).unwrap().0,
            WorkerError::Lifecycle.code()
        );

        worker.incomplete = true;
        assert_eq!(worker.open_replay(&[], 1, 7), Err(WorkerError::Incomplete));
        let destroyed = worker.execute(&command(WorkerCommandKind::Destroy, [0; 5]));
        assert_eq!(parse_result(&destroyed).unwrap().0, 0);
        let started = worker.execute(&command(WorkerCommandKind::Start, [2, 7, 0, 0, 0]));
        assert_eq!(parse_result(&started).unwrap().0, 0);
    }

    #[test]
    fn replay_upload_ceiling_fails_before_pointer_dereference() {
        let invalid = unsafe {
            ksa64_wasm_open_replay(
                core::ptr::NonNull::<u8>::dangling().as_ptr(),
                KPS1_EVIDENCE_OBJECT_MAX_LENGTH as usize + 1,
                1,
                7,
                0,
            )
        };
        assert_eq!(invalid, 0);
        let result = WORKER.with(|worker| worker.borrow().last_result.clone());
        assert_eq!(parse_result(&result).unwrap().0, WorkerError::Value.code());
    }

    #[test]
    fn fast_advance_and_poll_keep_terminal_procedure_visible() {
        let mut worker = WasmAuthority::default();
        assert_eq!(
            parse_result(&worker.execute(&command(
                WorkerCommandKind::Start,
                [2, 878133587, 1263747382, 0, 0]
            )))
            .unwrap()
            .0,
            0
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Prepare, [0; 5])))
                .unwrap()
                .0,
            0
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Pace, [1, 0, 0, 0, 0])))
                .unwrap()
                .0,
            0
        );
        loop {
            let result = worker.execute(&command(WorkerCommandKind::Advance, [32, 0, 0, 0, 0]));
            let (status, _) = parse_result(&result).unwrap();
            if status != 0 {
                panic!(
                    "fast advancement failed at release {} with status {status}",
                    worker
                        .session
                        .as_ref()
                        .unwrap()
                        .authority()
                        .snapshot()
                        .release_epoch
                );
            }
            let poll = worker.execute(&command(WorkerCommandKind::Poll, [0; 5]));
            let (poll_status, _) = parse_result(&poll).unwrap();
            if poll_status != 0 {
                panic!(
                    "fast polling failed at release {} with status {poll_status}",
                    worker
                        .session
                        .as_ref()
                        .unwrap()
                        .authority()
                        .snapshot()
                        .release_epoch
                );
            }
            if worker
                .session
                .as_ref()
                .unwrap()
                .authority()
                .snapshot()
                .release_epoch
                >= 8_100
            {
                break;
            }
        }
    }
    #[test]
    #[ignore = "complete no-action presentation run; exercised by the Phase 12B.5 acceptance workflow"]
    fn fast_no_action_poll_reaches_completed_transport_state() {
        let mut worker = WasmAuthority::default();
        assert_eq!(
            parse_result(&worker.execute(&command(
                WorkerCommandKind::Start,
                [2, 878133587, 1263747382, 0, 0]
            )))
            .unwrap()
            .0,
            0
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Prepare, [0; 5])))
                .unwrap()
                .0,
            0
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Pace, [1, 0, 0, 0, 0])))
                .unwrap()
                .0,
            0
        );
        loop {
            let advance = worker.execute(&command(WorkerCommandKind::Advance, [32, 0, 0, 0, 0]));
            assert_eq!(parse_result(&advance).unwrap().0, 0);
            let poll = worker.execute(&command(WorkerCommandKind::Poll, [0; 5]));
            assert_eq!(parse_result(&poll).unwrap().0, 0);
            let snapshot = worker.session.as_ref().unwrap().authority().snapshot();
            if snapshot.lifecycle == ksa64_session::phase11_live::MissionSessionLifecycle::Completed
            {
                assert!(snapshot.release_epoch <= 22_100);
                let (_, poll_payload) = parse_result(&poll).unwrap();
                assert!(poll_payload.windows(4).any(|window| window == b"PEM1"));
                let evidence = worker.execute(&command(WorkerCommandKind::Evidence, [0; 5]));
                let (evidence_status, evidence_payload) = parse_result(&evidence).unwrap();
                assert_eq!(evidence_status, 0);
                assert_eq!(&evidence_payload[..4], b"KSB1");
                let summary = worker.execute(&command(WorkerCommandKind::Summary, [0; 5]));
                let (summary_status, summary_payload) = parse_result(&summary).unwrap();
                assert_eq!(summary_status, 0);
                assert_eq!(get_u32(summary_payload, 0), snapshot.release_epoch);
                assert_eq!(get_u32(summary_payload, 4), snapshot.action_count);
                assert_eq!(get_u32(summary_payload, 8) as usize, evidence_payload.len());
                break;
            }
        }
    }

    #[test]
    #[ignore = "accepted KSB11 generation plus complete strict replay"]
    fn accepted_evidence_round_trips_through_the_wasm_replay_lane() {
        let mut worker = WasmAuthority::default();
        let generated = worker.execute(&command(WorkerCommandKind::RunScripted, [0; 5]));
        let (status, payload) = parse_result(&generated).unwrap();
        assert_eq!(status, 0);
        let evidence = payload.to_vec();
        assert_eq!(evidence.len(), EXPECTED_EVIDENCE_LENGTH);
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Destroy, [0; 5])))
                .unwrap()
                .0,
            0
        );

        let mut corrupt = evidence.clone();
        corrupt[crate::COMMAND_LENGTH] ^= 1;
        assert_eq!(
            worker.open_replay(&corrupt, 1, 0x1234),
            Err(WorkerError::Replay)
        );
        assert!(worker.replay.is_none());

        let info = worker.open_replay(&evidence, 1, 0x1234).unwrap();
        assert_eq!(&info[..4], b"KPRI");
        assert_eq!(info.len(), REPLAY_INFO_LENGTH);
        assert_eq!(info[28], PresentationRole::Observer as u8);
        let frame_count = u64::from_le_bytes(info[8..16].try_into().unwrap());
        assert!(frame_count >= 21_592);
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Start, [1, 7, 0, 0, 0],)))
                .unwrap()
                .0,
            WorkerError::Lifecycle.code()
        );
        assert_eq!(
            parse_result(&worker.execute(&command(WorkerCommandKind::Action, [1, 0, 0, 0, 1],)))
                .unwrap()
                .0,
            WorkerError::Lifecycle.code()
        );

        let mut first = 0u64;
        let mut snapshots = 0usize;
        let mut final_metadata = false;
        while first < frame_count {
            let bundle = worker.execute(&command(
                WorkerCommandKind::ReplayRead,
                [
                    first as u32,
                    (first >> 32) as u32,
                    256,
                    REPLAY_READ_MAX_BYTES as u32,
                    0,
                ],
            ));
            let (read_status, bytes) = parse_result(&bundle).unwrap();
            assert_eq!(read_status, 0);
            assert_eq!(&bytes[..4], b"KPW1");
            let count = usize::from(get_u16(bytes, 6));
            assert!(count > 0);
            let mut at = 8usize;
            for _ in 0..count {
                let length = get_u32(bytes, at) as usize;
                at += 4;
                let frame = &bytes[at..at + length];
                at += length;
                let decoded = ksa64_presentation::parse_kps1_frame(frame).unwrap();
                assert_eq!(decoded.header.session_nonce, 0x1234);
                assert_eq!(decoded.header.sequence, first + 1);
                first += 1;
                let payload = ksa64_presentation::decode_typed_payload(
                    decoded.header.kind,
                    decoded.payload,
                    PresentationRole::Observer,
                )
                .unwrap();
                if let PresentationPayload::Snapshot(value) = payload {
                    snapshots += 1;
                    assert!(value.truth.is_none());
                }
                final_metadata = decoded.header.kind
                    == ksa64_presentation::PresentationMessageKind::EvidenceMetadata
                    && decoded.header.flags & ksa64_presentation::KPS1_FLAG_FINAL != 0;
            }
            assert_eq!(at, bytes.len());
        }
        assert_eq!(first, frame_count);
        assert!(snapshots >= 21_592);
        assert!(final_metadata);

        let eof = worker.execute(&command(
            WorkerCommandKind::ReplayRead,
            [first as u32, (first >> 32) as u32, 1, 64, 0],
        ));
        let (eof_status, eof_bytes) = parse_result(&eof).unwrap();
        assert_eq!(eof_status, 0);
        assert_eq!(get_u16(eof_bytes, 6), 0);
        let out_of_range = worker.execute(&command(
            WorkerCommandKind::ReplayRead,
            [(first + 1) as u32, ((first + 1) >> 32) as u32, 1, 64, 0],
        ));
        assert_eq!(
            parse_result(&out_of_range).unwrap().0,
            WorkerError::Value.code()
        );
    }

    #[test]
    #[ignore = "full exact mission evidence; run explicitly or through the Node WASM harness"]
    fn scripted_authority_yields_accepted_length() {
        let mut worker = WasmAuthority::default();
        let result = worker.execute(&command(WorkerCommandKind::RunScripted, [0; 5]));
        let (status, payload) = parse_result(&result).unwrap();
        assert_eq!(status, 0);
        assert_eq!(payload.len(), EXPECTED_EVIDENCE_LENGTH);
        assert_eq!(&payload[..4], b"KSB1");
    }
}
