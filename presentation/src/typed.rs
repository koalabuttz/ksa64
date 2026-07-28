use alloc::{string::String, vec::Vec};

use crate::*;

pub const PRESENTATION_TEXT_MAX_LENGTH: usize = 512;
pub const PRESENTATION_PREDICATE_MAX_COUNT: usize = 16;
pub const PRESENTATION_PATH_MAX_POINTS: usize = 4_096;
pub const PRESENTATION_SAMPLE_BATCH_MAX_COUNT: usize = 2_048;
const PAYLOAD_HEADER_LENGTH: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationPayload {
    HandshakeRequest(PresentationHandshake),
    HandshakeResponse(PresentationHandshake),
    LifecycleControl(LifecycleControl),
    PaceControl(PaceControl),
    ReplayControl(PresentationCursors),
    Snapshot(OperationalSnapshot),
    Procedure(ProcedureView),
    Disposition(DispositionView),
    PredictionPath(PredictionPathView),
    TimelineEvent(TimelineEventView),
    EventBatch(Vec<PresentationEventView>),
    ReleaseSampleBatch(Vec<ReleaseSampleView>),
    GlobalDisplayDefinition(GlobalDisplayDefinitionV1),
    GlobalDisplaySampleBatch(Vec<GlobalDisplaySampleV1>),
    GlobalDisplayPathChunk(GlobalDisplayPathChunkV1),
    GlobalDisplayTransition(GlobalDisplayTransitionV1),
    GlobalReplayIndex(GlobalReplayIndexV1),
    GlobalDisplayCursorState(GlobalDisplayCursorStateV1),
    GlobalDisplayRangeRequest(GlobalDisplayRangeRequestV1),
    TransportStatus(TransportStatusView),
    ActionIntent(PresentationActionIntent),
    ActionReceipt(ActionReceiptView),
    ActionProposal(ActionProposalView),
    EvidenceMetadata(SealedEvidenceMetadata),
    EvidenceChunk(SealedEvidenceChunk),
    Error(PresentationErrorView),
}

impl PresentationPayload {
    pub const fn kind(&self) -> PresentationMessageKind {
        match self {
            Self::HandshakeRequest(_) => PresentationMessageKind::HandshakeRequest,
            Self::HandshakeResponse(_) => PresentationMessageKind::HandshakeResponse,
            Self::LifecycleControl(_) => PresentationMessageKind::LifecycleControl,
            Self::PaceControl(_) => PresentationMessageKind::PaceControl,
            Self::ReplayControl(_) => PresentationMessageKind::ReplayControl,
            Self::Snapshot(_) => PresentationMessageKind::Snapshot,
            Self::Procedure(_) => PresentationMessageKind::Procedure,
            Self::Disposition(_) => PresentationMessageKind::Disposition,
            Self::PredictionPath(_) => PresentationMessageKind::PredictionPath,
            Self::TimelineEvent(_) => PresentationMessageKind::TimelineEvent,
            Self::EventBatch(_) => PresentationMessageKind::EventBatch,
            Self::ReleaseSampleBatch(_) => PresentationMessageKind::ReleaseSampleBatch,
            Self::GlobalDisplayDefinition(_) => PresentationMessageKind::GlobalDisplayDefinition,
            Self::GlobalDisplaySampleBatch(_) => PresentationMessageKind::GlobalDisplaySampleBatch,
            Self::GlobalDisplayPathChunk(_) => PresentationMessageKind::GlobalDisplayPathChunk,
            Self::GlobalDisplayTransition(_) => PresentationMessageKind::GlobalDisplayTransition,
            Self::GlobalReplayIndex(_) => PresentationMessageKind::GlobalReplayIndex,
            Self::GlobalDisplayCursorState(_) => PresentationMessageKind::GlobalDisplayCursorState,
            Self::GlobalDisplayRangeRequest(_) => {
                PresentationMessageKind::GlobalDisplayRangeRequest
            }
            Self::TransportStatus(_) => PresentationMessageKind::TransportStatus,
            Self::ActionIntent(_) => PresentationMessageKind::ActionIntent,
            Self::ActionReceipt(_) => PresentationMessageKind::ActionReceipt,
            Self::ActionProposal(_) => PresentationMessageKind::ActionProposal,
            Self::EvidenceMetadata(_) => PresentationMessageKind::EvidenceMetadata,
            Self::EvidenceChunk(_) => PresentationMessageKind::EvidenceChunk,
            Self::Error(_) => PresentationMessageKind::Error,
        }
    }
}

pub fn encode_typed_payload(
    value: &PresentationPayload,
    role: PresentationRole,
) -> Result<Vec<u8>, Kps1Error> {
    match value {
        PresentationPayload::HandshakeRequest(value)
        | PresentationPayload::HandshakeResponse(value) => encode_handshake(value),
        PresentationPayload::LifecycleControl(value) => encode_lifecycle_control(*value),
        PresentationPayload::PaceControl(value) => encode_pace_control(*value),
        PresentationPayload::ReplayControl(value) => {
            let mut bytes = alloc::vec![0; CURSORS_PAYLOAD_LENGTH];
            write_cursors_payload(*value, &mut bytes)?;
            Ok(bytes)
        }
        PresentationPayload::Snapshot(value) => encode_snapshot(value, role),
        PresentationPayload::Procedure(value) => encode_procedure(value),
        PresentationPayload::Disposition(value) => encode_disposition(*value),
        PresentationPayload::PredictionPath(value) => encode_prediction_path(value),
        PresentationPayload::TimelineEvent(value) => encode_timeline_event(value),
        PresentationPayload::EventBatch(value) => encode_event_batch(value),
        PresentationPayload::ReleaseSampleBatch(value) => encode_release_samples(value),
        PresentationPayload::GlobalDisplayDefinition(value) => {
            encode_global_display_definition_payload(*value, role)
        }
        PresentationPayload::GlobalDisplaySampleBatch(value) => {
            encode_global_display_samples_payload(value, role)
        }
        PresentationPayload::GlobalDisplayPathChunk(value) => {
            encode_global_display_path_payload(value, role)
        }
        PresentationPayload::GlobalDisplayTransition(value) => {
            encode_global_display_transition_payload(*value)
        }
        PresentationPayload::GlobalReplayIndex(value) => encode_global_replay_index_payload(value),
        PresentationPayload::GlobalDisplayCursorState(value) => {
            encode_global_display_cursor_state_payload(*value)
        }
        PresentationPayload::GlobalDisplayRangeRequest(value) => {
            encode_global_display_range_request_payload(*value)
        }
        PresentationPayload::TransportStatus(value) => encode_transport_status(*value),
        PresentationPayload::ActionIntent(value) => {
            let mut bytes = alloc::vec![0; ACTION_INTENT_PAYLOAD_LENGTH];
            write_action_intent_payload(*value, role, &mut bytes)?;
            Ok(bytes)
        }
        PresentationPayload::ActionReceipt(value) => encode_action_receipt(*value),
        PresentationPayload::ActionProposal(value) => encode_action_proposal(value),
        PresentationPayload::EvidenceMetadata(value) => {
            let mut bytes = alloc::vec![0; EVIDENCE_METADATA_PAYLOAD_LENGTH];
            write_evidence_metadata_payload(*value, &mut bytes)?;
            Ok(bytes)
        }
        PresentationPayload::EvidenceChunk(value) => encode_evidence_chunk(value),
        PresentationPayload::Error(value) => encode_error(value),
    }
}

pub fn decode_typed_payload(
    kind: PresentationMessageKind,
    input: &[u8],
    role: PresentationRole,
) -> Result<PresentationPayload, Kps1Error> {
    if input.len() > KPS1_MAX_PAYLOAD_LENGTH {
        return Err(Kps1Error::PayloadTooLarge);
    }
    Ok(match kind {
        PresentationMessageKind::HandshakeRequest => {
            PresentationPayload::HandshakeRequest(decode_handshake(input)?)
        }
        PresentationMessageKind::HandshakeResponse => {
            PresentationPayload::HandshakeResponse(decode_handshake(input)?)
        }
        PresentationMessageKind::LifecycleControl => {
            PresentationPayload::LifecycleControl(decode_lifecycle_control(input)?)
        }
        PresentationMessageKind::PaceControl => {
            PresentationPayload::PaceControl(decode_pace_control(input)?)
        }
        PresentationMessageKind::ReplayControl => {
            PresentationPayload::ReplayControl(parse_cursors_payload(input)?)
        }
        PresentationMessageKind::Snapshot => {
            PresentationPayload::Snapshot(decode_snapshot(input, role)?)
        }
        PresentationMessageKind::Procedure => {
            PresentationPayload::Procedure(decode_procedure(input)?)
        }
        PresentationMessageKind::Disposition => {
            PresentationPayload::Disposition(decode_disposition(input)?)
        }
        PresentationMessageKind::PredictionPath => {
            PresentationPayload::PredictionPath(decode_prediction_path(input)?)
        }
        PresentationMessageKind::TimelineEvent => {
            PresentationPayload::TimelineEvent(decode_timeline_event(input)?)
        }
        PresentationMessageKind::EventBatch => {
            PresentationPayload::EventBatch(decode_event_batch(input)?)
        }
        PresentationMessageKind::ReleaseSampleBatch => {
            PresentationPayload::ReleaseSampleBatch(decode_release_samples(input)?)
        }
        PresentationMessageKind::GlobalDisplayDefinition => {
            PresentationPayload::GlobalDisplayDefinition(decode_global_display_definition_payload(
                input, role,
            )?)
        }
        PresentationMessageKind::GlobalDisplaySampleBatch => {
            PresentationPayload::GlobalDisplaySampleBatch(decode_global_display_samples_payload(
                input, role,
            )?)
        }
        PresentationMessageKind::GlobalDisplayPathChunk => {
            PresentationPayload::GlobalDisplayPathChunk(decode_global_display_path_payload(
                input, role,
            )?)
        }
        PresentationMessageKind::GlobalDisplayTransition => {
            PresentationPayload::GlobalDisplayTransition(decode_global_display_transition_payload(
                input,
            )?)
        }
        PresentationMessageKind::GlobalReplayIndex => {
            PresentationPayload::GlobalReplayIndex(decode_global_replay_index_payload(input)?)
        }
        PresentationMessageKind::GlobalDisplayCursorState => {
            PresentationPayload::GlobalDisplayCursorState(
                decode_global_display_cursor_state_payload(input)?,
            )
        }
        PresentationMessageKind::GlobalDisplayRangeRequest => {
            PresentationPayload::GlobalDisplayRangeRequest(
                decode_global_display_range_request_payload(input)?,
            )
        }
        PresentationMessageKind::TransportStatus => {
            PresentationPayload::TransportStatus(decode_transport_status(input)?)
        }
        PresentationMessageKind::ActionIntent => {
            PresentationPayload::ActionIntent(parse_action_intent_payload(input, role)?)
        }
        PresentationMessageKind::ActionReceipt => {
            PresentationPayload::ActionReceipt(decode_action_receipt(input)?)
        }
        PresentationMessageKind::ActionProposal => {
            PresentationPayload::ActionProposal(decode_action_proposal(input)?)
        }
        PresentationMessageKind::EvidenceMetadata => {
            PresentationPayload::EvidenceMetadata(parse_evidence_metadata_payload(input)?)
        }
        PresentationMessageKind::EvidenceChunk => {
            PresentationPayload::EvidenceChunk(decode_evidence_chunk(input)?)
        }
        PresentationMessageKind::Error => PresentationPayload::Error(decode_error(input)?),
    })
}

fn encode_handshake(value: &PresentationHandshake) -> Result<Vec<u8>, Kps1Error> {
    value.cursors.validate().map_err(|_| Kps1Error::Cursor)?;
    if value.client_instance == 0 {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PHS1");
    writer.u8(value.role as u8);
    writer.zeros(3);
    writer.u64(value.client_instance);
    writer.u64(value.capability_mask);
    write_cursors(&mut writer, value.cursors);
    writer.finish()
}

fn decode_handshake(input: &[u8]) -> Result<PresentationHandshake, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PHS1")?;
    let role = PresentationRole::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    reader.reserved(3)?;
    let client_instance = reader.u64()?;
    let capability_mask = reader.u64()?;
    let cursors = read_cursors(&mut reader)?;
    reader.finish()?;
    if client_instance == 0 {
        return Err(Kps1Error::Identity);
    }
    cursors.validate().map_err(|_| Kps1Error::Cursor)?;
    Ok(PresentationHandshake {
        role,
        client_instance,
        capability_mask,
        cursors,
    })
}

fn encode_lifecycle_control(value: LifecycleControl) -> Result<Vec<u8>, Kps1Error> {
    let mut writer = PayloadWriter::new(*b"PLC1");
    writer.u8(value.requested as u8);
    writer.zeros(3);
    writer.u32(value.bounded_releases);
    writer.finish()
}

fn decode_lifecycle_control(input: &[u8]) -> Result<LifecycleControl, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PLC1")?;
    let requested = PresentationLifecycle::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    reader.reserved(3)?;
    let bounded_releases = reader.u32()?;
    reader.finish()?;
    Ok(LifecycleControl {
        requested,
        bounded_releases,
    })
}

fn encode_pace_control(value: PaceControl) -> Result<Vec<u8>, Kps1Error> {
    let mut writer = PayloadWriter::new(*b"PPC1");
    writer.u8(value.requested as u8);
    writer.zeros(3);
    writer.u32(value.bounded_releases);
    writer.finish()
}

fn decode_pace_control(input: &[u8]) -> Result<PaceControl, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PPC1")?;
    let requested = PresentationPace::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    reader.reserved(3)?;
    let bounded_releases = reader.u32()?;
    reader.finish()?;
    Ok(PaceControl {
        requested,
        bounded_releases,
    })
}

fn encode_snapshot(
    value: &OperationalSnapshot,
    expected_role: PresentationRole,
) -> Result<Vec<u8>, Kps1Error> {
    if value.role != expected_role {
        return Err(Kps1Error::Enum);
    }
    validate_snapshot(value)?;
    let mut writer = PayloadWriter::new(*b"POS1");
    writer.u32(value.presentation_model_identity);
    writer.u32(value.session_definition_identity);
    writer.u64(value.publication_sequence);
    writer.u64(value.validity_mask);
    writer.u8(value.role as u8);
    writer.u8(value.lifecycle as u8);
    writer.u8(value.pace as u8);
    writer.u8(value.gnss_state);
    writer.bool(value.safe);
    writer.bool(value.truth.is_some());
    writer.zeros(2);
    writer.u32(value.release_epoch);
    writer.u32(value.release_period_micros);
    writer.u32(value.frame_identity);
    writer.u32(value.mission_time_q16);
    write_navigation(&mut writer, value.onboard);
    write_navigation(&mut writer, value.ground);
    write_prediction_summary(&mut writer, value.prediction);
    writer.u32(value.flight_checksum);
    writer.u32(value.command_checksum);
    writer.u32(value.procedure_chain);
    writer.u32(value.journal_chain);
    writer.u32(value.action_chain);
    writer.u32(value.staged_load_identity);
    writer.u32(value.action_count);
    writer.u16(value.rejected_loads);
    writer.zeros(2);
    if let Some(truth) = value.truth {
        write_truth(&mut writer, truth);
    }
    writer.finish()
}

fn decode_snapshot(
    input: &[u8],
    expected_role: PresentationRole,
) -> Result<OperationalSnapshot, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"POS1")?;
    let presentation_model_identity = reader.u32()?;
    let session_definition_identity = reader.u32()?;
    let publication_sequence = reader.u64()?;
    let validity_mask = reader.u64()?;
    let role = PresentationRole::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    let lifecycle = PresentationLifecycle::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    let pace = PresentationPace::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    let gnss_state = reader.u8()?;
    let safe = reader.bool()?;
    let truth_present = reader.bool()?;
    reader.reserved(2)?;
    let release_epoch = reader.u32()?;
    let release_period_micros = reader.u32()?;
    let frame_identity = reader.u32()?;
    let mission_time_q16 = reader.u32()?;
    let onboard = read_navigation(&mut reader)?;
    let ground = read_navigation(&mut reader)?;
    let prediction = read_prediction_summary(&mut reader)?;
    let flight_checksum = reader.u32()?;
    let command_checksum = reader.u32()?;
    let procedure_chain = reader.u32()?;
    let journal_chain = reader.u32()?;
    let action_chain = reader.u32()?;
    let staged_load_identity = reader.u32()?;
    let action_count = reader.u32()?;
    let rejected_loads = reader.u16()?;
    reader.reserved(2)?;
    let truth = if truth_present {
        Some(read_truth(&mut reader)?)
    } else {
        None
    };
    reader.finish()?;
    let value = OperationalSnapshot {
        presentation_model_identity,
        session_definition_identity,
        publication_sequence,
        validity_mask,
        role,
        lifecycle,
        pace,
        release_epoch,
        release_period_micros,
        frame_identity,
        mission_time_q16,
        onboard,
        ground,
        prediction,
        flight_checksum,
        command_checksum,
        procedure_chain,
        journal_chain,
        action_chain,
        staged_load_identity,
        action_count,
        rejected_loads,
        gnss_state,
        safe,
        truth,
    };
    if role != expected_role {
        return Err(Kps1Error::Enum);
    }
    validate_snapshot(&value)?;
    Ok(value)
}

fn validate_snapshot(value: &OperationalSnapshot) -> Result<(), Kps1Error> {
    if value.presentation_model_identity == 0
        || value.session_definition_identity == 0
        || value.publication_sequence == 0
    {
        return Err(Kps1Error::Identity);
    }
    if value.gnss_state > 2 {
        return Err(Kps1Error::Enum);
    }
    if value.validity_mask & !SNAPSHOT_VALID_MASK != 0 {
        return Err(Kps1Error::Reserved);
    }
    if value.validity_mask & SNAPSHOT_VALID_PREDICTION != 0
        && (value.prediction.prediction_identity == 0
            || value.prediction.terminal_reason == 0
            || value.prediction.terminal_reason > 5)
    {
        return Err(Kps1Error::Identity);
    }
    if value.truth.is_some() != (value.validity_mask & SNAPSHOT_VALID_TRUTH != 0)
        || (value.truth.is_some() && !value.role.permits_private_truth())
    {
        return Err(Kps1Error::Enum);
    }
    Ok(())
}

fn write_navigation(writer: &mut PayloadWriter, value: NavigationView) {
    writer.i32_array(&value.position_q12_km);
    writer.i32_array(&value.velocity_q24_km_s);
    writer.u32(value.checksum);
}

fn read_navigation(reader: &mut PayloadReader<'_>) -> Result<NavigationView, Kps1Error> {
    Ok(NavigationView {
        position_q12_km: reader.i32_array()?,
        velocity_q24_km_s: reader.i32_array()?,
        checksum: reader.u32()?,
    })
}

fn write_prediction_summary(writer: &mut PayloadWriter, value: PredictionSummaryView) {
    writer.u32(value.prediction_identity);
    writer.u32(value.prediction_checksum);
    writer.u32(value.source_estimate_identity);
    writer.u32(value.frame_identity);
    writer.i32(value.apogee_q12_km);
    writer.i32(value.perigee_q12_km);
    writer.u32(value.time_to_apogee_q16);
    writer.u32(value.time_to_impact_q16);
    writer.i32_array(&value.impact_position_q12_km);
    writer.u8(value.terminal_reason);
    writer.zeros(3);
}

fn read_prediction_summary(
    reader: &mut PayloadReader<'_>,
) -> Result<PredictionSummaryView, Kps1Error> {
    let value = PredictionSummaryView {
        prediction_identity: reader.u32()?,
        prediction_checksum: reader.u32()?,
        source_estimate_identity: reader.u32()?,
        frame_identity: reader.u32()?,
        apogee_q12_km: reader.i32()?,
        perigee_q12_km: reader.i32()?,
        time_to_apogee_q16: reader.u32()?,
        time_to_impact_q16: reader.u32()?,
        impact_position_q12_km: reader.i32_array()?,
        terminal_reason: reader.u8()?,
    };
    reader.reserved(3)?;
    if value.terminal_reason > 5 {
        return Err(Kps1Error::Enum);
    }
    Ok(value)
}

fn write_truth(writer: &mut PayloadWriter, value: SimTruthView) {
    writer.i32_array(&value.position_q12_km);
    writer.i32_array(&value.velocity_q24_km_s);
    writer.i32_array(&value.attitude_q30);
    writer.u32(value.physical_checksum);
    writer.u32(value.injected_fault_mask);
}

fn read_truth(reader: &mut PayloadReader<'_>) -> Result<SimTruthView, Kps1Error> {
    Ok(SimTruthView {
        position_q12_km: reader.i32_array()?,
        velocity_q24_km_s: reader.i32_array()?,
        attitude_q30: reader.i32_array()?,
        physical_checksum: reader.u32()?,
        injected_fault_mask: reader.u32()?,
    })
}

fn encode_procedure(value: &ProcedureView) -> Result<Vec<u8>, Kps1Error> {
    if value.procedure_identity == 0
        || value.step_count == 0
        || value.active_step == 0
        || value.active_step > value.step_count
        || value.predicates.len() > PRESENTATION_PREDICATE_MAX_COUNT
    {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PPR1");
    writer.u32(value.procedure_identity);
    writer.u16(value.active_step);
    writer.u16(value.step_count);
    writer.u8(value.state as u8);
    writer.bool(value.hints_available);
    writer.zeros(2);
    writer.u32(value.entered_epoch);
    writer.u32(value.deadline_epoch);
    writer.u16(value.predicates.len() as u16);
    writer.zeros(2);
    writer.text(&value.title, PRESENTATION_TEXT_MAX_LENGTH)?;
    writer.text(&value.instruction, PRESENTATION_TEXT_MAX_LENGTH)?;
    for predicate in &value.predicates {
        if predicate.identity == 0 {
            return Err(Kps1Error::Identity);
        }
        writer.u32(predicate.identity);
        writer.bool(predicate.satisfied);
        writer.zeros(3);
    }
    writer.finish()
}

fn decode_procedure(input: &[u8]) -> Result<ProcedureView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PPR1")?;
    let procedure_identity = reader.u32()?;
    let active_step = reader.u16()?;
    let step_count = reader.u16()?;
    let state = ProcedureStepState::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    let hints_available = reader.bool()?;
    reader.reserved(2)?;
    let entered_epoch = reader.u32()?;
    let deadline_epoch = reader.u32()?;
    let count = reader.u16()? as usize;
    reader.reserved(2)?;
    if procedure_identity == 0
        || step_count == 0
        || active_step == 0
        || active_step > step_count
        || count > PRESENTATION_PREDICATE_MAX_COUNT
    {
        return Err(Kps1Error::Identity);
    }
    let title = reader.text(PRESENTATION_TEXT_MAX_LENGTH)?;
    let instruction = reader.text(PRESENTATION_TEXT_MAX_LENGTH)?;
    let mut predicates = Vec::with_capacity(count);
    for _ in 0..count {
        let identity = reader.u32()?;
        let satisfied = reader.bool()?;
        reader.reserved(3)?;
        if identity == 0 {
            return Err(Kps1Error::Identity);
        }
        predicates.push(ProcedurePredicateView {
            identity,
            satisfied,
        });
    }
    reader.finish()?;
    Ok(ProcedureView {
        procedure_identity,
        active_step,
        step_count,
        state,
        entered_epoch,
        deadline_epoch,
        title,
        instruction,
        predicates,
        hints_available,
    })
}

fn encode_disposition(value: DispositionView) -> Result<Vec<u8>, Kps1Error> {
    validate_disposition(value)?;
    let mut writer = PayloadWriter::new(*b"PDS1");
    writer.u8(value.overall as u8);
    writer.u8(value.axes.objective);
    writer.u8(value.axes.vehicle);
    writer.u8(value.axes.procedure);
    writer.u8(value.axes.operator);
    writer.u8(value.axes.avionics);
    writer.u8(value.axes.evidence);
    writer.zeros(1);
    writer.u32(value.reason_identity);
    writer.finish()
}

fn decode_disposition(input: &[u8]) -> Result<DispositionView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PDS1")?;
    let value = DispositionView {
        overall: OverallDisposition::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?,
        axes: DispositionAxes {
            objective: reader.u8()?,
            vehicle: reader.u8()?,
            procedure: reader.u8()?,
            operator: reader.u8()?,
            avionics: reader.u8()?,
            evidence: reader.u8()?,
        },
        reason_identity: {
            reader.reserved(1)?;
            reader.u32()?
        },
    };
    reader.finish()?;
    validate_disposition(value)?;
    Ok(value)
}

fn validate_disposition(value: DispositionView) -> Result<(), Kps1Error> {
    if !(1..=5).contains(&value.axes.objective)
        || !(1..=6).contains(&value.axes.vehicle)
        || !(1..=6).contains(&value.axes.procedure)
        || !(1..=5).contains(&value.axes.operator)
        || !(1..=4).contains(&value.axes.avionics)
        || !(1..=5).contains(&value.axes.evidence)
    {
        return Err(Kps1Error::Enum);
    }
    Ok(())
}

fn encode_action_proposal(value: &ActionProposalView) -> Result<Vec<u8>, Kps1Error> {
    if value.proposal_identity == 0
        || !(1..=5).contains(&value.load_type)
        || value.permitted_operations == 0
        || value.permitted_operations & !ACTION_PERMIT_MASK != 0
    {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PAP1");
    writer.u32(value.proposal_identity);
    writer.u32(value.load_identity);
    writer.u8(value.load_type);
    writer.u8(value.permitted_operations);
    writer.zeros(2);
    writer.u32(value.stage_epoch);
    writer.u32(value.earliest_commit_epoch);
    writer.u32(value.activation_epoch);
    writer.u32(value.expires_epoch);
    writer.u32(value.payload_checksum);
    writer.u32(value.completed_event_mask);
    writer.text(&value.label, PRESENTATION_TEXT_MAX_LENGTH)?;
    writer.finish()
}

fn decode_action_proposal(input: &[u8]) -> Result<ActionProposalView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PAP1")?;
    let proposal_identity = reader.u32()?;
    let load_identity = reader.u32()?;
    let load_type = reader.u8()?;
    let permitted_operations = reader.u8()?;
    reader.reserved(2)?;
    let stage_epoch = reader.u32()?;
    let earliest_commit_epoch = reader.u32()?;
    let activation_epoch = reader.u32()?;
    let expires_epoch = reader.u32()?;
    let payload_checksum = reader.u32()?;
    let completed_event_mask = reader.u32()?;
    let label = reader.text(PRESENTATION_TEXT_MAX_LENGTH)?;
    reader.finish()?;
    if proposal_identity == 0
        || !(1..=5).contains(&load_type)
        || permitted_operations == 0
        || permitted_operations & !ACTION_PERMIT_MASK != 0
    {
        return Err(Kps1Error::Identity);
    }
    Ok(ActionProposalView {
        proposal_identity,
        load_identity,
        load_type,
        permitted_operations,
        stage_epoch,
        earliest_commit_epoch,
        activation_epoch,
        expires_epoch,
        payload_checksum,
        completed_event_mask,
        label,
    })
}

fn encode_action_receipt(value: ActionReceiptView) -> Result<Vec<u8>, Kps1Error> {
    if value.publication_sequence == 0 || value.proposal_identity == 0 {
        return Err(Kps1Error::Identity);
    }
    if value.state > 6 || value.reason > 13 {
        return Err(Kps1Error::Enum);
    }
    let mut writer = PayloadWriter::new(*b"PAR1");
    writer.u64(value.publication_sequence);
    writer.u32(value.proposal_identity);
    writer.u32(value.load_identity);
    writer.u32(value.control_identity);
    writer.u32(value.receipt_epoch);
    writer.u32(value.effective_epoch);
    writer.u8(value.state);
    writer.u8(value.reason);
    writer.bool(value.accepted);
    writer.u8(value.operation as u8);
    writer.u32(value.receipt_checksum);
    writer.finish()
}

fn decode_action_receipt(input: &[u8]) -> Result<ActionReceiptView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PAR1")?;
    let value = ActionReceiptView {
        publication_sequence: reader.u64()?,
        proposal_identity: reader.u32()?,
        load_identity: reader.u32()?,
        control_identity: reader.u32()?,
        receipt_epoch: reader.u32()?,
        effective_epoch: reader.u32()?,
        state: reader.u8()?,
        reason: reader.u8()?,
        accepted: reader.bool()?,
        operation: PresentationActionOperation::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?,
        receipt_checksum: reader.u32()?,
    };
    reader.finish()?;
    if value.publication_sequence == 0 || value.proposal_identity == 0 {
        return Err(Kps1Error::Identity);
    }
    if value.state > 6 || value.reason > 13 {
        return Err(Kps1Error::Enum);
    }
    Ok(value)
}

fn encode_event_batch(value: &[PresentationEventView]) -> Result<Vec<u8>, Kps1Error> {
    if value.len() > PRESENTATION_SAMPLE_BATCH_MAX_COUNT {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let mut writer = PayloadWriter::new(*b"PEV1");
    writer.u16(value.len() as u16);
    writer.zeros(2);
    let mut previous = 0;
    for event in value {
        if event.sequence == 0 || (previous != 0 && event.sequence <= previous) || event.kind == 0 {
            return Err(Kps1Error::Sequence);
        }
        previous = event.sequence;
        writer.u64(event.sequence);
        writer.u32(event.release_epoch);
        writer.u16(event.kind);
        writer.zeros(2);
        writer.u32(event.detail_identity);
    }
    writer.finish()
}

fn decode_event_batch(input: &[u8]) -> Result<Vec<PresentationEventView>, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PEV1")?;
    let count = reader.u16()? as usize;
    reader.reserved(2)?;
    if count > PRESENTATION_SAMPLE_BATCH_MAX_COUNT {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let mut events = Vec::with_capacity(count);
    let mut previous = 0;
    for _ in 0..count {
        let sequence = reader.u64()?;
        let release_epoch = reader.u32()?;
        let kind = reader.u16()?;
        reader.reserved(2)?;
        let detail_identity = reader.u32()?;
        if sequence == 0 || (previous != 0 && sequence <= previous) || kind == 0 {
            return Err(Kps1Error::Sequence);
        }
        previous = sequence;
        events.push(PresentationEventView {
            sequence,
            release_epoch,
            kind,
            detail_identity,
        });
    }
    reader.finish()?;
    Ok(events)
}

fn encode_timeline_event(value: &TimelineEventView) -> Result<Vec<u8>, Kps1Error> {
    if value.sequence == 0 || value.event_identity == 0 {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PTE1");
    writer.u64(value.sequence);
    writer.u32(value.release_epoch);
    writer.u32(value.source_identity);
    writer.u8(value.severity as u8);
    writer.zeros(3);
    writer.u32(value.event_identity);
    writer.u32(value.detail_identity);
    writer.text(&value.label, PRESENTATION_TEXT_MAX_LENGTH)?;
    writer.finish()
}

fn decode_timeline_event(input: &[u8]) -> Result<TimelineEventView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PTE1")?;
    let sequence = reader.u64()?;
    let release_epoch = reader.u32()?;
    let source_identity = reader.u32()?;
    let severity = TimelineSeverity::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    reader.reserved(3)?;
    let event_identity = reader.u32()?;
    let detail_identity = reader.u32()?;
    let label = reader.text(PRESENTATION_TEXT_MAX_LENGTH)?;
    reader.finish()?;
    if sequence == 0 || event_identity == 0 {
        return Err(Kps1Error::Identity);
    }
    Ok(TimelineEventView {
        sequence,
        release_epoch,
        source_identity,
        severity,
        event_identity,
        detail_identity,
        label,
    })
}

fn encode_release_samples(value: &[ReleaseSampleView]) -> Result<Vec<u8>, Kps1Error> {
    if value.len() > PRESENTATION_SAMPLE_BATCH_MAX_COUNT {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let mut writer = PayloadWriter::new(*b"PRS1");
    writer.u16(value.len() as u16);
    writer.zeros(2);
    for sample in value {
        write_release_sample(&mut writer, *sample)?;
    }
    writer.finish()
}

fn decode_release_samples(input: &[u8]) -> Result<Vec<ReleaseSampleView>, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PRS1")?;
    let count = reader.u16()? as usize;
    reader.reserved(2)?;
    if count > PRESENTATION_SAMPLE_BATCH_MAX_COUNT {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let mut samples = Vec::with_capacity(count);
    let mut previous = 0;
    for _ in 0..count {
        let sample = read_release_sample(&mut reader)?;
        if sample.sequence == 0 || (previous != 0 && sample.sequence <= previous) {
            return Err(Kps1Error::Sequence);
        }
        previous = sample.sequence;
        samples.push(sample);
    }
    reader.finish()?;
    Ok(samples)
}

fn write_release_sample(
    writer: &mut PayloadWriter,
    value: ReleaseSampleView,
) -> Result<(), Kps1Error> {
    if value.sequence == 0 {
        return Err(Kps1Error::Sequence);
    }
    writer.u64(value.sequence);
    writer.u64(value.validity_mask);
    writer.u32(value.release_epoch);
    writer.u32(value.mission_time_q16);
    writer.u32(value.frame_identity);
    writer.i32_array(&value.onboard_position_q12_km);
    writer.i32_array(&value.onboard_velocity_q24_km_s);
    writer.i32_array(&value.ground_position_q12_km);
    writer.i32_array(&value.ground_velocity_q24_km_s);
    writer.i32_array(&value.predicted_impact_q12_km);
    writer.i32(value.predicted_apogee_q12_km);
    writer.i32(value.altitude_q12_km);
    writer.i32(value.speed_q24_km_s);
    writer.i32(value.downrange_q12_km);
    writer.i32(value.crossrange_q12_km);
    Ok(())
}

fn read_release_sample(reader: &mut PayloadReader<'_>) -> Result<ReleaseSampleView, Kps1Error> {
    Ok(ReleaseSampleView {
        sequence: reader.u64()?,
        validity_mask: reader.u64()?,
        release_epoch: reader.u32()?,
        mission_time_q16: reader.u32()?,
        frame_identity: reader.u32()?,
        onboard_position_q12_km: reader.i32_array()?,
        onboard_velocity_q24_km_s: reader.i32_array()?,
        ground_position_q12_km: reader.i32_array()?,
        ground_velocity_q24_km_s: reader.i32_array()?,
        predicted_impact_q12_km: reader.i32_array()?,
        predicted_apogee_q12_km: reader.i32()?,
        altitude_q12_km: reader.i32()?,
        speed_q24_km_s: reader.i32()?,
        downrange_q12_km: reader.i32()?,
        crossrange_q12_km: reader.i32()?,
    })
}

fn encode_prediction_path(value: &PredictionPathView) -> Result<Vec<u8>, Kps1Error> {
    if value.path_identity == 0
        || value.model_identity == 0
        || !(1..=4).contains(&value.product_identity)
        || value.points.len() > PRESENTATION_PATH_MAX_POINTS
        || value.terminal_reason == 0
        || value.terminal_reason > 5
    {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PPP1");
    writer.u32(value.path_identity);
    writer.u32(value.product_identity);
    writer.u32(value.model_identity);
    writer.u32(value.source_estimate_identity);
    writer.u32(value.source_estimate_checksum);
    writer.u32(value.source_epoch);
    writer.u32(value.generation_epoch);
    writer.u32(value.frame_identity);
    writer.u8(value.terminal_reason);
    writer.zeros(3);
    writer.u32(value.cadence_releases);
    writer.u32(value.path_checksum);
    writer.u16(value.points.len() as u16);
    writer.zeros(2);
    let mut previous_epoch = None;
    for point in &value.points {
        if previous_epoch.is_some_and(|previous| point.release_epoch <= previous) {
            return Err(Kps1Error::Sequence);
        }
        previous_epoch = Some(point.release_epoch);
        writer.u32(point.release_epoch);
        writer.u32(point.frame_identity);
        writer.i32_array(&point.position_q12_km);
        writer.i32(point.altitude_q12_km);
        writer.i32(point.downrange_q12_km);
        writer.i32(point.crossrange_q12_km);
    }
    writer.finish()
}

fn decode_prediction_path(input: &[u8]) -> Result<PredictionPathView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PPP1")?;
    let path_identity = reader.u32()?;
    let product_identity = reader.u32()?;
    let model_identity = reader.u32()?;
    let source_estimate_identity = reader.u32()?;
    let source_estimate_checksum = reader.u32()?;
    let source_epoch = reader.u32()?;
    let generation_epoch = reader.u32()?;
    let frame_identity = reader.u32()?;
    let terminal_reason = reader.u8()?;
    reader.reserved(3)?;
    let cadence_releases = reader.u32()?;
    let path_checksum = reader.u32()?;
    let count = reader.u16()? as usize;
    reader.reserved(2)?;
    if path_identity == 0
        || model_identity == 0
        || !(1..=4).contains(&product_identity)
        || count > PRESENTATION_PATH_MAX_POINTS
        || terminal_reason == 0
        || terminal_reason > 5
    {
        return Err(Kps1Error::Identity);
    }
    let mut points = Vec::with_capacity(count);
    let mut previous_epoch = None;
    for _ in 0..count {
        let point = PredictionPathPoint {
            release_epoch: reader.u32()?,
            frame_identity: reader.u32()?,
            position_q12_km: reader.i32_array()?,
            altitude_q12_km: reader.i32()?,
            downrange_q12_km: reader.i32()?,
            crossrange_q12_km: reader.i32()?,
        };
        if previous_epoch.is_some_and(|previous| point.release_epoch <= previous) {
            return Err(Kps1Error::Sequence);
        }
        previous_epoch = Some(point.release_epoch);
        points.push(point);
    }
    reader.finish()?;
    Ok(PredictionPathView {
        path_identity,
        product_identity,
        model_identity,
        source_estimate_identity,
        source_estimate_checksum,
        source_epoch,
        generation_epoch,
        frame_identity,
        terminal_reason,
        cadence_releases,
        path_checksum,
        points,
    })
}

fn encode_transport_status(value: TransportStatusView) -> Result<Vec<u8>, Kps1Error> {
    validate_queue(value.queue)?;
    if value.worker_state > 3 || value.finalization_state > 2 {
        return Err(Kps1Error::Enum);
    }
    let mut writer = PayloadWriter::new(*b"PTS1");
    writer.u8(value.staleness as u8);
    writer.u8(value.worker_state);
    writer.u8(value.finalization_state);
    writer.zeros(1);
    writer.u32(value.queue.command_capacity);
    writer.u32(value.queue.commands_pending);
    writer.u32(value.queue.event_capacity);
    writer.u32(value.queue.events_pending);
    writer.u32(value.queue.timeline_capacity);
    writer.u32(value.queue.timeline_pending);
    writer.u32(value.queue.sample_capacity);
    writer.u32(value.queue.samples_pending);
    writer.bool(value.queue.event_overflow);
    writer.bool(value.queue.timeline_overflow);
    writer.bool(value.queue.sample_overflow);
    writer.zeros(1);
    writer.i32(value.last_command_result);
    writer.finish()
}

fn decode_transport_status(input: &[u8]) -> Result<TransportStatusView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PTS1")?;
    let staleness = PresentationStaleness::from_raw(reader.u8()?).ok_or(Kps1Error::Enum)?;
    let worker_state = reader.u8()?;
    let finalization_state = reader.u8()?;
    reader.reserved(1)?;
    let queue = PresentationQueueStatus {
        command_capacity: reader.u32()?,
        commands_pending: reader.u32()?,
        event_capacity: reader.u32()?,
        events_pending: reader.u32()?,
        timeline_capacity: reader.u32()?,
        timeline_pending: reader.u32()?,
        sample_capacity: reader.u32()?,
        samples_pending: reader.u32()?,
        event_overflow: reader.bool()?,
        timeline_overflow: reader.bool()?,
        sample_overflow: reader.bool()?,
    };
    reader.reserved(1)?;
    let last_command_result = reader.i32()?;
    reader.finish()?;
    validate_queue(queue)?;
    if worker_state > 3 || finalization_state > 2 {
        return Err(Kps1Error::Enum);
    }
    Ok(TransportStatusView {
        staleness,
        worker_state,
        finalization_state,
        queue,
        last_command_result,
    })
}

fn validate_queue(value: PresentationQueueStatus) -> Result<(), Kps1Error> {
    if value.commands_pending > value.command_capacity
        || value.events_pending > value.event_capacity
        || value.timeline_pending > value.timeline_capacity
        || value.samples_pending > value.sample_capacity
    {
        return Err(Kps1Error::Length);
    }
    Ok(())
}

fn encode_evidence_chunk(value: &SealedEvidenceChunk) -> Result<Vec<u8>, Kps1Error> {
    validate_evidence_chunk(value)?;
    let mut writer = PayloadWriter::new(*b"PEC1");
    writer.u32(value.evidence_identity);
    writer.u32(value.chunk_index);
    writer.u32(value.chunk_count);
    writer.u64(value.logical_offset);
    writer.u32(value.bytes.len() as u32);
    writer.bytes.extend_from_slice(&value.bytes);
    let bytes = writer.finish()?;
    if bytes.len() > KPS1_EVIDENCE_CHUNK_MAX_LENGTH {
        return Err(Kps1Error::EvidenceChunkTooLarge);
    }
    Ok(bytes)
}

fn decode_evidence_chunk(input: &[u8]) -> Result<SealedEvidenceChunk, Kps1Error> {
    if input.len() > KPS1_EVIDENCE_CHUNK_MAX_LENGTH {
        return Err(Kps1Error::EvidenceChunkTooLarge);
    }
    let mut reader = PayloadReader::new(input, *b"PEC1")?;
    let evidence_identity = reader.u32()?;
    let chunk_index = reader.u32()?;
    let chunk_count = reader.u32()?;
    let logical_offset = reader.u64()?;
    let data_length = reader.u32()? as usize;
    if data_length > KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH {
        return Err(Kps1Error::EvidenceChunkTooLarge);
    }
    let bytes = reader.take(data_length)?.to_vec();
    reader.finish()?;
    let value = SealedEvidenceChunk {
        evidence_identity,
        chunk_index,
        chunk_count,
        logical_offset,
        bytes,
    };
    validate_evidence_chunk(&value)?;
    Ok(value)
}

fn validate_evidence_chunk(value: &SealedEvidenceChunk) -> Result<(), Kps1Error> {
    if value.evidence_identity == 0 {
        return Err(Kps1Error::Identity);
    }
    if value.chunk_count == 0 || value.chunk_index >= value.chunk_count {
        return Err(Kps1Error::ChunkCount);
    }
    if value.bytes.is_empty() || value.bytes.len() > KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH {
        return Err(Kps1Error::EvidenceChunkTooLarge);
    }
    Ok(())
}

fn encode_error(value: &PresentationErrorView) -> Result<Vec<u8>, Kps1Error> {
    if value.code == 0 {
        return Err(Kps1Error::Identity);
    }
    let mut writer = PayloadWriter::new(*b"PER1");
    writer.u16(value.code);
    writer.bool(value.fatal);
    writer.zeros(1);
    writer.u32(value.detail_identity);
    writer.text(&value.message, PRESENTATION_TEXT_MAX_LENGTH)?;
    writer.finish()
}

fn decode_error(input: &[u8]) -> Result<PresentationErrorView, Kps1Error> {
    let mut reader = PayloadReader::new(input, *b"PER1")?;
    let code = reader.u16()?;
    let fatal = reader.bool()?;
    reader.reserved(1)?;
    let detail_identity = reader.u32()?;
    let message = reader.text(PRESENTATION_TEXT_MAX_LENGTH)?;
    reader.finish()?;
    if code == 0 {
        return Err(Kps1Error::Identity);
    }
    Ok(PresentationErrorView {
        code,
        fatal,
        detail_identity,
        message,
    })
}

fn write_cursors(writer: &mut PayloadWriter, value: PresentationCursors) {
    writer.u64(value.snapshots);
    writer.u64(value.events);
    writer.u64(value.timeline);
    writer.u64(value.action_receipts);
    writer.u64(value.release_samples);
}

fn read_cursors(reader: &mut PayloadReader<'_>) -> Result<PresentationCursors, Kps1Error> {
    Ok(PresentationCursors {
        snapshots: reader.u64()?,
        events: reader.u64()?,
        timeline: reader.u64()?,
        action_receipts: reader.u64()?,
        release_samples: reader.u64()?,
    })
}

struct PayloadWriter {
    bytes: Vec<u8>,
}

impl PayloadWriter {
    fn new(magic: [u8; 4]) -> Self {
        let mut bytes = alloc::vec![0; PAYLOAD_HEADER_LENGTH];
        bytes[..4].copy_from_slice(&magic);
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&(PAYLOAD_HEADER_LENGTH as u16).to_le_bytes());
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i32_array<const N: usize>(&mut self, values: &[i32; N]) {
        for value in values {
            self.i32(*value);
        }
    }

    fn zeros(&mut self, count: usize) {
        self.bytes.resize(self.bytes.len() + count, 0);
    }

    fn text(&mut self, value: &str, max_length: usize) -> Result<(), Kps1Error> {
        if value.len() > max_length || value.len() > u16::MAX as usize {
            return Err(Kps1Error::PayloadTooLarge);
        }
        self.u16(value.len() as u16);
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>, Kps1Error> {
        if self.bytes.len() > KPS1_MAX_PAYLOAD_LENGTH {
            return Err(Kps1Error::PayloadTooLarge);
        }
        let length = u32::try_from(self.bytes.len()).map_err(|_| Kps1Error::PayloadTooLarge)?;
        self.bytes[8..12].copy_from_slice(&length.to_le_bytes());
        Ok(self.bytes)
    }
}

struct PayloadReader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(input: &'a [u8], magic: [u8; 4]) -> Result<Self, Kps1Error> {
        if input.len() < PAYLOAD_HEADER_LENGTH || input.len() > KPS1_MAX_PAYLOAD_LENGTH {
            return Err(Kps1Error::Length);
        }
        if input[..4] != magic {
            return Err(Kps1Error::Magic);
        }
        if u16::from_le_bytes([input[4], input[5]]) != 1 {
            return Err(Kps1Error::Version);
        }
        if u16::from_le_bytes([input[6], input[7]]) as usize != PAYLOAD_HEADER_LENGTH {
            return Err(Kps1Error::HeaderLength);
        }
        let declared = u32::from_le_bytes([input[8], input[9], input[10], input[11]]) as usize;
        if declared != input.len() {
            return Err(Kps1Error::Length);
        }
        Ok(Self {
            input,
            at: PAYLOAD_HEADER_LENGTH,
        })
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], Kps1Error> {
        let end = self.at.checked_add(count).ok_or(Kps1Error::Length)?;
        let value = self.input.get(self.at..end).ok_or(Kps1Error::Length)?;
        self.at = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, Kps1Error> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, Kps1Error> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Kps1Error::Enum),
        }
    }

    fn u16(&mut self) -> Result<u16, Kps1Error> {
        let value = self.take(2)?;
        Ok(u16::from_le_bytes([value[0], value[1]]))
    }

    fn u32(&mut self) -> Result<u32, Kps1Error> {
        let value = self.take(4)?;
        Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn i32(&mut self) -> Result<i32, Kps1Error> {
        Ok(self.u32()? as i32)
    }

    fn u64(&mut self) -> Result<u64, Kps1Error> {
        let value = self.take(8)?;
        Ok(u64::from_le_bytes([
            value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
        ]))
    }

    fn i32_array<const N: usize>(&mut self) -> Result<[i32; N], Kps1Error> {
        let mut values = [0; N];
        for value in &mut values {
            *value = self.i32()?;
        }
        Ok(values)
    }

    fn reserved(&mut self, count: usize) -> Result<(), Kps1Error> {
        if self.take(count)?.iter().any(|byte| *byte != 0) {
            return Err(Kps1Error::Reserved);
        }
        Ok(())
    }

    fn text(&mut self, max_length: usize) -> Result<String, Kps1Error> {
        let length = self.u16()? as usize;
        if length > max_length {
            return Err(Kps1Error::PayloadTooLarge);
        }
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| Kps1Error::Enum)
    }

    fn finish(self) -> Result<(), Kps1Error> {
        if self.at != self.input.len() {
            return Err(Kps1Error::Length);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(role: PresentationRole) -> OperationalSnapshot {
        OperationalSnapshot {
            presentation_model_identity: PRESENTATION_MODEL_ID,
            session_definition_identity: 0x12b0_1001,
            publication_sequence: 77,
            validity_mask: SNAPSHOT_VALID_PUBLIC_MASK,
            role,
            lifecycle: PresentationLifecycle::Running,
            pace: PresentationPace::Realtime,
            release_epoch: 5_824,
            release_period_micros: 31_250,
            frame_identity: 2,
            mission_time_q16: 11_927_552,
            onboard: NavigationView {
                position_q12_km: [1, -2, 3],
                velocity_q24_km_s: [-4, 5, -6],
                checksum: 7,
            },
            ground: NavigationView {
                position_q12_km: [8, 9, 10],
                velocity_q24_km_s: [11, 12, 13],
                checksum: 14,
            },
            prediction: PredictionSummaryView {
                prediction_identity: 15,
                prediction_checksum: 16,
                source_estimate_identity: 17,
                frame_identity: 2,
                apogee_q12_km: 18,
                perigee_q12_km: -19,
                time_to_apogee_q16: 20,
                time_to_impact_q16: 21,
                impact_position_q12_km: [22, 23, 24],
                terminal_reason: 2,
            },
            flight_checksum: 25,
            command_checksum: 26,
            procedure_chain: 27,
            journal_chain: 28,
            action_chain: 29,
            staged_load_identity: 0,
            action_count: 2,
            rejected_loads: 1,
            gnss_state: 2,
            safe: false,
            truth: None,
        }
    }

    fn round_trip(value: PresentationPayload, role: PresentationRole) {
        let kind = value.kind();
        let bytes = encode_typed_payload(&value, role).unwrap();
        assert!(bytes.len() <= KPS1_MAX_PAYLOAD_LENGTH);
        assert_eq!(decode_typed_payload(kind, &bytes, role), Ok(value));
    }

    #[test]
    fn procedure_wire_format_accepts_one_based_terminal_step_ids() {
        let value = PresentationPayload::Procedure(ProcedureView {
            procedure_identity: 0x12b3_1001,
            active_step: 7,
            step_count: 7,
            state: ProcedureStepState::Failed,
            entered_epoch: 7_800,
            deadline_epoch: 7_872,
            title: String::from("GNSS LOSS / FAIL"),
            instruction: String::from("Procedure window expired."),
            predicates: Vec::new(),
            hints_available: true,
        });
        round_trip(value, PresentationRole::GuidedOperator);
    }

    #[test]
    fn every_typed_message_has_a_deterministic_round_trip() {
        let role = PresentationRole::GuidedOperator;
        let cursors = PresentationCursors::default();
        let handshake = PresentationHandshake {
            role,
            client_instance: 0x0102_0304_0506_0708,
            capability_mask: 0x55aa,
            cursors,
        };
        let procedure = ProcedureView {
            procedure_identity: 1,
            active_step: 2,
            step_count: 9,
            state: ProcedureStepState::Active,
            entered_epoch: 10,
            deadline_epoch: 20,
            title: String::from("GNSS LOSS / VERIFY"),
            instruction: String::from("Compare onboard and ground navigation."),
            predicates: alloc::vec![ProcedurePredicateView {
                identity: 3,
                satisfied: true,
            }],
            hints_available: true,
        };
        let disposition = DispositionView {
            overall: OverallDisposition::DegradedSuccess,
            axes: DispositionAxes {
                objective: 1,
                vehicle: 2,
                procedure: 1,
                operator: 1,
                avionics: 2,
                evidence: 1,
            },
            reason_identity: 4,
        };
        let proposal = ActionProposalView {
            proposal_identity: 5,
            load_identity: 6,
            load_type: 1,
            permitted_operations: ACTION_PERMIT_REVIEW | ACTION_PERMIT_STAGE,
            stage_epoch: 7,
            earliest_commit_epoch: 8,
            activation_epoch: 9,
            expires_epoch: 10,
            payload_checksum: 11,
            completed_event_mask: 12,
            label: String::from("Review ground update"),
        };
        let intent = PresentationActionIntent {
            proposal_identity: 5,
            expected_load_identity: 6,
            operation: PresentationActionOperation::Stage,
            requested_activation_epoch: 9,
            client_action_sequence: 13,
        };
        let receipt = ActionReceiptView {
            publication_sequence: 14,
            proposal_identity: 5,
            load_identity: 6,
            control_identity: 15,
            receipt_epoch: 16,
            effective_epoch: 17,
            state: 1,
            reason: 1,
            accepted: true,
            operation: PresentationActionOperation::Stage,
            receipt_checksum: 18,
        };
        let timeline = TimelineEventView {
            sequence: 19,
            release_epoch: 20,
            source_identity: 21,
            severity: TimelineSeverity::Caution,
            event_identity: 22,
            detail_identity: 23,
            label: String::from("GNSS unavailable"),
        };
        let event = PresentationEventView {
            sequence: 20,
            release_epoch: 21,
            kind: 1,
            detail_identity: 22,
        };
        let sample = ReleaseSampleView {
            sequence: 21,
            release_epoch: 22,
            ..ReleaseSampleView::default()
        };
        let path = PredictionPathView {
            path_identity: 24,
            product_identity: 1,
            model_identity: 25,
            source_estimate_identity: 26,
            source_estimate_checksum: 27,
            source_epoch: 28,
            generation_epoch: 29,
            frame_identity: 2,
            terminal_reason: 1,
            cadence_releases: 32,
            path_checksum: 30,
            points: alloc::vec![PredictionPathPoint {
                release_epoch: 31,
                frame_identity: 2,
                position_q12_km: [1, 2, 3],
                altitude_q12_km: 4,
                downrange_q12_km: 5,
                crossrange_q12_km: 6,
            }],
        };
        let transport = TransportStatusView {
            staleness: PresentationStaleness::Current,
            worker_state: 1,
            finalization_state: 0,
            queue: PresentationQueueStatus {
                command_capacity: 4,
                commands_pending: 1,
                event_capacity: 8,
                events_pending: 2,
                timeline_capacity: 8,
                timeline_pending: 3,
                sample_capacity: 16,
                samples_pending: 4,
                event_overflow: false,
                timeline_overflow: false,
                sample_overflow: true,
            },
            last_command_result: 0,
        };
        let evidence = SealedEvidenceMetadata {
            evidence_identity: 32,
            evidence_crc32: 33,
            total_length: 100,
            chunk_length: 64,
            chunk_count: 2,
            complete: true,
            content_kind: 1,
        };
        let error = PresentationErrorView {
            code: 34,
            fatal: false,
            detail_identity: 35,
            message: String::from("resynchronization required"),
        };

        for value in [
            PresentationPayload::HandshakeRequest(handshake),
            PresentationPayload::HandshakeResponse(handshake),
            PresentationPayload::LifecycleControl(LifecycleControl {
                requested: PresentationLifecycle::Paused,
                bounded_releases: 0,
            }),
            PresentationPayload::PaceControl(PaceControl {
                requested: PresentationPace::SingleStep,
                bounded_releases: 1,
            }),
            PresentationPayload::ReplayControl(cursors),
            PresentationPayload::Snapshot(snapshot(role)),
            PresentationPayload::Procedure(procedure),
            PresentationPayload::Disposition(disposition),
            PresentationPayload::PredictionPath(path),
            PresentationPayload::TimelineEvent(timeline),
            PresentationPayload::EventBatch(alloc::vec![event]),
            PresentationPayload::ReleaseSampleBatch(alloc::vec![sample]),
            PresentationPayload::TransportStatus(transport),
            PresentationPayload::ActionIntent(intent),
            PresentationPayload::ActionReceipt(receipt),
            PresentationPayload::ActionProposal(proposal),
            PresentationPayload::EvidenceMetadata(evidence),
            PresentationPayload::EvidenceChunk(SealedEvidenceChunk {
                evidence_identity: 32,
                chunk_index: 0,
                chunk_count: 1,
                logical_offset: 0,
                bytes: alloc::vec![1, 2, 3],
            }),
            PresentationPayload::Error(error),
        ] {
            round_trip(value, role);
        }
    }

    #[test]
    fn typed_payload_header_and_utf8_are_strict() {
        let value = PresentationPayload::Error(PresentationErrorView {
            code: 1,
            fatal: false,
            detail_identity: 2,
            message: String::from("ok"),
        });
        let mut bytes = encode_typed_payload(&value, PresentationRole::Observer).unwrap();
        bytes[6] = 11;
        assert_eq!(
            decode_typed_payload(
                PresentationMessageKind::Error,
                &bytes,
                PresentationRole::Observer
            ),
            Err(Kps1Error::HeaderLength)
        );
        bytes[6] = 12;
        let last = bytes.len() - 1;
        bytes[last] = 0xff;
        assert_eq!(
            decode_typed_payload(
                PresentationMessageKind::Error,
                &bytes,
                PresentationRole::Observer
            ),
            Err(Kps1Error::Enum)
        );
    }

    #[test]
    fn typed_snapshot_is_bound_to_the_immutable_session_role() {
        let value = snapshot(PresentationRole::GuidedOperator);
        assert_eq!(
            encode_typed_payload(
                &PresentationPayload::Snapshot(value),
                PresentationRole::Observer,
            ),
            Err(Kps1Error::Enum)
        );
    }

    #[test]
    fn typed_snapshot_refuses_truth_for_an_unauthorized_role() {
        let mut value = snapshot(PresentationRole::GuidedOperator);
        value.truth = Some(SimTruthView::default());
        value.validity_mask |= SNAPSHOT_VALID_TRUTH;
        assert_eq!(
            encode_typed_payload(
                &PresentationPayload::Snapshot(value),
                PresentationRole::GuidedOperator
            ),
            Err(Kps1Error::Enum)
        );
    }
}
