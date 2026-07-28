use crate::{
    PresentationActionIntent, PresentationActionOperation, PresentationCursors, PresentationRole,
};

pub const KPS1_MAGIC: [u8; 4] = *b"KPS1";
pub const KPS1_MAJOR: u16 = 1;
pub const KPS1_MINOR: u16 = 0;
pub const KPS1_HEADER_LENGTH: usize = 48;
pub const KPS1_MAX_PAYLOAD_LENGTH: usize = 256 * 1024;
pub const KPS1_EVIDENCE_CHUNK_MAX_LENGTH: usize = 64 * 1024;
pub const KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH: usize = KPS1_EVIDENCE_CHUNK_MAX_LENGTH - 36;
pub const KPS1_EVIDENCE_OBJECT_MAX_LENGTH: u64 = 16 * 1024 * 1024;

pub const KPS1_FLAG_RESPONSE: u32 = 1 << 0;
pub const KPS1_FLAG_FINAL: u32 = 1 << 1;
pub const KPS1_FLAG_COALESCED: u32 = 1 << 2;
pub const KPS1_FLAG_LOSSY: u32 = 1 << 3;
pub const KPS1_FLAG_RESYNC_REQUIRED: u32 = 1 << 4;
pub const KPS1_OPTIONAL_FLAG_MASK: u32 = 0x0000_ffff;
pub const KPS1_REQUIRED_FLAG_MASK: u32 = 0xffff_0000;
pub const KPS1_SUPPORTED_REQUIRED_FLAGS: u32 = 0;
/// Optional global-display messages. Servers publish these only after both
/// peers negotiate the capability in the existing KPS1 1.0 handshake.
pub const KPS1_CAPABILITY_GLOBAL_DISPLAY_V1: u64 = 1 << 8;

pub const ACTION_INTENT_PAYLOAD_LENGTH: usize = 32;
pub const CURSORS_PAYLOAD_LENGTH: usize = 48;
pub const EVIDENCE_METADATA_PAYLOAD_LENGTH: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum PresentationMessageKind {
    HandshakeRequest = 0x0001,
    HandshakeResponse = 0x0002,
    LifecycleControl = 0x0010,
    PaceControl = 0x0011,
    ReplayControl = 0x0012,
    Snapshot = 0x0100,
    Procedure = 0x0101,
    Disposition = 0x0102,
    PredictionPath = 0x0103,
    TimelineEvent = 0x0104,
    ReleaseSampleBatch = 0x0105,
    TransportStatus = 0x0106,
    EventBatch = 0x0107,
    GlobalDisplayDefinition = 0x0110,
    GlobalDisplaySampleBatch = 0x0111,
    GlobalDisplayPathChunk = 0x0112,
    GlobalDisplayTransition = 0x0113,
    GlobalReplayIndex = 0x0114,
    GlobalDisplayCursorState = 0x0115,
    ActionIntent = 0x0200,
    ActionReceipt = 0x0201,
    ActionProposal = 0x0202,
    GlobalDisplayRangeRequest = 0x0210,
    EvidenceMetadata = 0x0300,
    EvidenceChunk = 0x0301,
    Error = 0x7fff,
}

impl PresentationMessageKind {
    pub const fn from_raw(raw: u16) -> Option<Self> {
        match raw {
            0x0001 => Some(Self::HandshakeRequest),
            0x0002 => Some(Self::HandshakeResponse),
            0x0010 => Some(Self::LifecycleControl),
            0x0011 => Some(Self::PaceControl),
            0x0012 => Some(Self::ReplayControl),
            0x0100 => Some(Self::Snapshot),
            0x0101 => Some(Self::Procedure),
            0x0102 => Some(Self::Disposition),
            0x0103 => Some(Self::PredictionPath),
            0x0104 => Some(Self::TimelineEvent),
            0x0105 => Some(Self::ReleaseSampleBatch),
            0x0106 => Some(Self::TransportStatus),
            0x0107 => Some(Self::EventBatch),
            0x0110 => Some(Self::GlobalDisplayDefinition),
            0x0111 => Some(Self::GlobalDisplaySampleBatch),
            0x0112 => Some(Self::GlobalDisplayPathChunk),
            0x0113 => Some(Self::GlobalDisplayTransition),
            0x0114 => Some(Self::GlobalReplayIndex),
            0x0115 => Some(Self::GlobalDisplayCursorState),
            0x0200 => Some(Self::ActionIntent),
            0x0201 => Some(Self::ActionReceipt),
            0x0202 => Some(Self::ActionProposal),
            0x0210 => Some(Self::GlobalDisplayRangeRequest),
            0x0300 => Some(Self::EvidenceMetadata),
            0x0301 => Some(Self::EvidenceChunk),
            0x7fff => Some(Self::Error),
            _ => None,
        }
    }

    const fn correlation_rule(self) -> CorrelationRule {
        match self {
            Self::HandshakeRequest
            | Self::HandshakeResponse
            | Self::LifecycleControl
            | Self::PaceControl
            | Self::ReplayControl
            | Self::ActionIntent
            | Self::GlobalDisplayRangeRequest => CorrelationRule::Required,
            Self::Snapshot
            | Self::Procedure
            | Self::Disposition
            | Self::PredictionPath
            | Self::TimelineEvent
            | Self::ReleaseSampleBatch
            | Self::TransportStatus
            | Self::EventBatch
            | Self::GlobalDisplayDefinition
            | Self::GlobalDisplaySampleBatch
            | Self::GlobalDisplayPathChunk
            | Self::GlobalDisplayTransition
            | Self::GlobalReplayIndex
            | Self::ActionProposal
            | Self::EvidenceMetadata
            | Self::EvidenceChunk => CorrelationRule::Zero,
            Self::ActionReceipt | Self::GlobalDisplayCursorState | Self::Error => {
                CorrelationRule::Either
            }
        }
    }

    const fn permits_zero_session(self) -> bool {
        matches!(self, Self::HandshakeRequest)
    }

    pub const fn required_capability_mask(self) -> u64 {
        match self {
            Self::GlobalDisplayDefinition
            | Self::GlobalDisplaySampleBatch
            | Self::GlobalDisplayPathChunk
            | Self::GlobalDisplayTransition
            | Self::GlobalReplayIndex
            | Self::GlobalDisplayCursorState
            | Self::GlobalDisplayRangeRequest => KPS1_CAPABILITY_GLOBAL_DISPLAY_V1,
            _ => 0,
        }
    }

    pub const fn is_negotiated_by(self, capability_mask: u64) -> bool {
        let required = self.required_capability_mask();
        required == 0 || capability_mask & required == required
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CorrelationRule {
    Required,
    Zero,
    Either,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kps1Header {
    pub kind: PresentationMessageKind,
    pub flags: u32,
    pub session_nonce: u64,
    pub sequence: u64,
    pub correlation_id: u64,
    pub payload_length: u32,
}

impl Kps1Header {
    pub const fn encoded_length(self) -> Option<usize> {
        KPS1_HEADER_LENGTH.checked_add(self.payload_length as usize)
    }

    pub fn validate(self) -> Result<(), Kps1Error> {
        if self.flags & KPS1_REQUIRED_FLAG_MASK & !KPS1_SUPPORTED_REQUIRED_FLAGS != 0 {
            return Err(Kps1Error::UnsupportedRequiredFlags);
        }
        if self.sequence == 0 {
            return Err(Kps1Error::Sequence);
        }
        if self.session_nonce == 0 && !self.kind.permits_zero_session() {
            return Err(Kps1Error::Session);
        }
        match self.kind.correlation_rule() {
            CorrelationRule::Required if self.correlation_id == 0 => {
                return Err(Kps1Error::Correlation)
            }
            CorrelationRule::Zero if self.correlation_id != 0 => {
                return Err(Kps1Error::Correlation)
            }
            _ => {}
        }
        validate_payload_length(self.kind, self.payload_length as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodedKps1Frame<'a> {
    pub header: Kps1Header,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kps1Error {
    Length,
    Buffer,
    Magic,
    Version,
    HeaderLength,
    MessageKind,
    UnsupportedRequiredFlags,
    Session,
    Sequence,
    Correlation,
    PayloadTooLarge,
    EvidenceChunkTooLarge,
    EvidenceObjectTooLarge,
    Checksum,
    Reserved,
    Enum,
    Identity,
    Cursor,
    ChunkCount,
}

pub fn write_kps1_frame(
    header: Kps1Header,
    payload: &[u8],
    output: &mut [u8],
) -> Result<usize, Kps1Error> {
    header.validate()?;
    if payload.len() != header.payload_length as usize {
        return Err(Kps1Error::Length);
    }
    let total = KPS1_HEADER_LENGTH
        .checked_add(payload.len())
        .ok_or(Kps1Error::Length)?;
    if output.len() < total {
        return Err(Kps1Error::Buffer);
    }
    output[..total].fill(0);
    output[..4].copy_from_slice(&KPS1_MAGIC);
    put_u16(output, 4, KPS1_MAJOR);
    put_u16(output, 6, KPS1_MINOR);
    put_u16(output, 8, KPS1_HEADER_LENGTH as u16);
    put_u16(output, 10, header.kind as u16);
    put_u32(output, 12, header.flags);
    put_u64(output, 16, header.session_nonce);
    put_u64(output, 24, header.sequence);
    put_u64(output, 32, header.correlation_id);
    put_u32(output, 40, header.payload_length);
    output[KPS1_HEADER_LENGTH..total].copy_from_slice(payload);
    let checksum = crc32_parts(&output[..44], payload);
    put_u32(output, 44, checksum);
    Ok(total)
}

pub fn parse_kps1_frame(input: &[u8]) -> Result<DecodedKps1Frame<'_>, Kps1Error> {
    if input.len() < KPS1_HEADER_LENGTH {
        return Err(Kps1Error::Length);
    }
    if input[..4] != KPS1_MAGIC {
        return Err(Kps1Error::Magic);
    }
    if get_u16(input, 4) != KPS1_MAJOR || get_u16(input, 6) != KPS1_MINOR {
        return Err(Kps1Error::Version);
    }
    if get_u16(input, 8) as usize != KPS1_HEADER_LENGTH {
        return Err(Kps1Error::HeaderLength);
    }
    let kind =
        PresentationMessageKind::from_raw(get_u16(input, 10)).ok_or(Kps1Error::MessageKind)?;
    let header = Kps1Header {
        kind,
        flags: get_u32(input, 12),
        session_nonce: get_u64(input, 16),
        sequence: get_u64(input, 24),
        correlation_id: get_u64(input, 32),
        payload_length: get_u32(input, 40),
    };
    header.validate()?;
    let total = KPS1_HEADER_LENGTH
        .checked_add(header.payload_length as usize)
        .ok_or(Kps1Error::Length)?;
    if input.len() != total {
        return Err(Kps1Error::Length);
    }
    let payload = &input[KPS1_HEADER_LENGTH..];
    if get_u32(input, 44) != crc32_parts(&input[..44], payload) {
        return Err(Kps1Error::Checksum);
    }
    Ok(DecodedKps1Frame { header, payload })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kps1SequenceCursor {
    session_nonce: u64,
    next_sequence: u64,
}

impl Kps1SequenceCursor {
    pub const fn new(session_nonce: u64, next_sequence: u64) -> Result<Self, Kps1Error> {
        if session_nonce == 0 {
            return Err(Kps1Error::Session);
        }
        if next_sequence == 0 {
            return Err(Kps1Error::Sequence);
        }
        Ok(Self {
            session_nonce,
            next_sequence,
        })
    }

    pub const fn next_sequence(self) -> u64 {
        self.next_sequence
    }

    pub fn accept(&mut self, header: Kps1Header) -> Result<(), Kps1Error> {
        if header.session_nonce != self.session_nonce {
            return Err(Kps1Error::Session);
        }
        if header.sequence != self.next_sequence {
            return Err(Kps1Error::Sequence);
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(Kps1Error::Sequence)?;
        Ok(())
    }
}

pub fn write_action_intent_payload(
    value: PresentationActionIntent,
    role: PresentationRole,
    output: &mut [u8],
) -> Result<(), Kps1Error> {
    value.validate(role).map_err(|error| match error {
        crate::PresentationValueError::Role => Kps1Error::Enum,
        _ => Kps1Error::Identity,
    })?;
    if output.len() != ACTION_INTENT_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"PAI1");
    put_u16(output, 4, 1);
    put_u16(output, 6, ACTION_INTENT_PAYLOAD_LENGTH as u16);
    put_u32(output, 8, value.proposal_identity);
    put_u32(output, 12, value.expected_load_identity);
    output[16] = value.operation as u8;
    put_u32(output, 20, value.requested_activation_epoch);
    put_u64(output, 24, value.client_action_sequence);
    Ok(())
}

pub fn parse_action_intent_payload(
    input: &[u8],
    role: PresentationRole,
) -> Result<PresentationActionIntent, Kps1Error> {
    if input.len() != ACTION_INTENT_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    if input[..4] != *b"PAI1"
        || get_u16(input, 4) != 1
        || get_u16(input, 6) as usize != ACTION_INTENT_PAYLOAD_LENGTH
    {
        return Err(Kps1Error::Version);
    }
    if input[17..20].iter().any(|byte| *byte != 0) {
        return Err(Kps1Error::Reserved);
    }
    let value = PresentationActionIntent {
        proposal_identity: get_u32(input, 8),
        expected_load_identity: get_u32(input, 12),
        operation: PresentationActionOperation::from_raw(input[16]).ok_or(Kps1Error::Enum)?,
        requested_activation_epoch: get_u32(input, 20),
        client_action_sequence: get_u64(input, 24),
    };
    value.validate(role).map_err(|error| match error {
        crate::PresentationValueError::Role => Kps1Error::Enum,
        _ => Kps1Error::Identity,
    })?;
    Ok(value)
}

pub fn write_cursors_payload(
    value: PresentationCursors,
    output: &mut [u8],
) -> Result<(), Kps1Error> {
    value.validate().map_err(|_| Kps1Error::Cursor)?;
    if output.len() != CURSORS_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KCR1");
    put_u16(output, 4, 1);
    put_u16(output, 6, CURSORS_PAYLOAD_LENGTH as u16);
    put_u64(output, 8, value.snapshots);
    put_u64(output, 16, value.events);
    put_u64(output, 24, value.timeline);
    put_u64(output, 32, value.action_receipts);
    put_u64(output, 40, value.release_samples);
    Ok(())
}

pub fn parse_cursors_payload(input: &[u8]) -> Result<PresentationCursors, Kps1Error> {
    if input.len() != CURSORS_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    if input[..4] != *b"KCR1"
        || get_u16(input, 4) != 1
        || get_u16(input, 6) as usize != CURSORS_PAYLOAD_LENGTH
    {
        return Err(Kps1Error::Version);
    }
    let value = PresentationCursors {
        snapshots: get_u64(input, 8),
        events: get_u64(input, 16),
        timeline: get_u64(input, 24),
        action_receipts: get_u64(input, 32),
        release_samples: get_u64(input, 40),
    };
    value.validate().map_err(|_| Kps1Error::Cursor)?;
    Ok(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealedEvidenceMetadata {
    pub evidence_identity: u32,
    pub evidence_crc32: u32,
    pub total_length: u64,
    pub chunk_length: u32,
    pub chunk_count: u32,
    pub complete: bool,
    pub content_kind: u8,
}

impl SealedEvidenceMetadata {
    pub fn validate(self) -> Result<(), Kps1Error> {
        if self.evidence_identity == 0 || self.content_kind == 0 {
            return Err(Kps1Error::Identity);
        }
        if self.total_length == 0 || self.total_length > KPS1_EVIDENCE_OBJECT_MAX_LENGTH {
            return Err(Kps1Error::EvidenceObjectTooLarge);
        }
        if self.chunk_length == 0
            || self.chunk_length as usize > KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH
        {
            return Err(Kps1Error::EvidenceChunkTooLarge);
        }
        let expected = self.total_length.div_ceil(u64::from(self.chunk_length));
        if expected != u64::from(self.chunk_count) || self.chunk_count == 0 {
            return Err(Kps1Error::ChunkCount);
        }
        Ok(())
    }
}

pub fn write_evidence_metadata_payload(
    value: SealedEvidenceMetadata,
    output: &mut [u8],
) -> Result<(), Kps1Error> {
    value.validate()?;
    if output.len() != EVIDENCE_METADATA_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"PEM1");
    put_u16(output, 4, 1);
    put_u16(output, 6, EVIDENCE_METADATA_PAYLOAD_LENGTH as u16);
    put_u32(output, 8, value.evidence_identity);
    put_u32(output, 12, value.evidence_crc32);
    put_u64(output, 16, value.total_length);
    put_u32(output, 24, value.chunk_length);
    put_u32(output, 28, value.chunk_count);
    output[32] = u8::from(value.complete);
    output[33] = value.content_kind;
    Ok(())
}

pub fn parse_evidence_metadata_payload(input: &[u8]) -> Result<SealedEvidenceMetadata, Kps1Error> {
    if input.len() != EVIDENCE_METADATA_PAYLOAD_LENGTH {
        return Err(Kps1Error::Length);
    }
    if input[..4] != *b"PEM1"
        || get_u16(input, 4) != 1
        || get_u16(input, 6) as usize != EVIDENCE_METADATA_PAYLOAD_LENGTH
    {
        return Err(Kps1Error::Version);
    }
    if input[32] > 1 || input[34..].iter().any(|byte| *byte != 0) {
        return Err(Kps1Error::Reserved);
    }
    let value = SealedEvidenceMetadata {
        evidence_identity: get_u32(input, 8),
        evidence_crc32: get_u32(input, 12),
        total_length: get_u64(input, 16),
        chunk_length: get_u32(input, 24),
        chunk_count: get_u32(input, 28),
        complete: input[32] != 0,
        content_kind: input[33],
    };
    value.validate()?;
    Ok(value)
}

fn validate_payload_length(
    kind: PresentationMessageKind,
    payload_length: usize,
) -> Result<(), Kps1Error> {
    if payload_length > KPS1_MAX_PAYLOAD_LENGTH {
        return Err(Kps1Error::PayloadTooLarge);
    }
    if matches!(kind, PresentationMessageKind::EvidenceChunk)
        && payload_length > KPS1_EVIDENCE_CHUNK_MAX_LENGTH
    {
        return Err(Kps1Error::EvidenceChunkTooLarge);
    }
    Ok(())
}

fn crc32_parts(first: &[u8], second: &[u8]) -> u32 {
    let mut state = 0xffff_ffff_u32;
    for byte in first.iter().chain(second.iter()) {
        state ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(state & 1);
            state = (state >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !state
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

fn get_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
        input[offset + 4],
        input[offset + 5],
        input[offset + 6],
        input[offset + 7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_header(payload_length: usize) -> Kps1Header {
        Kps1Header {
            kind: PresentationMessageKind::Snapshot,
            flags: KPS1_FLAG_COALESCED,
            session_nonce: 0x0102_0304_0506_0708,
            sequence: 42,
            correlation_id: 0,
            payload_length: payload_length as u32,
        }
    }

    #[test]
    fn global_display_kinds_require_explicit_capability_negotiation() {
        assert_eq!(
            PresentationMessageKind::GlobalDisplaySampleBatch.required_capability_mask(),
            KPS1_CAPABILITY_GLOBAL_DISPLAY_V1
        );
        assert!(!PresentationMessageKind::GlobalDisplaySampleBatch.is_negotiated_by(0));
        assert!(PresentationMessageKind::GlobalDisplaySampleBatch
            .is_negotiated_by(KPS1_CAPABILITY_GLOBAL_DISPLAY_V1));
        for kind in [
            PresentationMessageKind::GlobalDisplayDefinition,
            PresentationMessageKind::GlobalDisplaySampleBatch,
            PresentationMessageKind::GlobalDisplayPathChunk,
            PresentationMessageKind::GlobalDisplayTransition,
            PresentationMessageKind::GlobalReplayIndex,
            PresentationMessageKind::GlobalDisplayCursorState,
            PresentationMessageKind::GlobalDisplayRangeRequest,
        ] {
            assert!(!kind.is_negotiated_by(0));
            assert!(kind.is_negotiated_by(KPS1_CAPABILITY_GLOBAL_DISPLAY_V1));
        }
        assert!(PresentationMessageKind::Snapshot.is_negotiated_by(0));
    }

    #[test]
    fn crc32_matches_the_standard_check_vector() {
        assert_eq!(crc32_parts(b"1234", b"56789"), 0xcbf4_3926);
    }

    #[test]
    fn frame_round_trip_is_strict_and_little_endian() {
        let payload = [0x10, 0x20, 0x30, 0x40];
        let mut bytes = [0_u8; 52];
        assert_eq!(
            write_kps1_frame(snapshot_header(payload.len()), &payload, &mut bytes),
            Ok(52)
        );
        assert_eq!(&bytes[..4], b"KPS1");
        assert_eq!(&bytes[4..12], &[1, 0, 0, 0, 48, 0, 0, 1]);
        assert_eq!(&bytes[16..24], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(&bytes[24..32], &[42, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[48..], &payload);
        const GOLDEN: [u8; 52] = [
            75, 80, 83, 49, 1, 0, 0, 0, 48, 0, 0, 1, 4, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 42, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 160, 237, 10, 12, 16, 32, 48, 64,
        ];
        assert_eq!(bytes, GOLDEN);
        let decoded = parse_kps1_frame(&bytes).unwrap();
        assert_eq!(decoded.header, snapshot_header(payload.len()));
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn frame_parser_rejects_every_structural_boundary() {
        let payload = [1, 2, 3];
        let mut bytes = [0_u8; 51];
        write_kps1_frame(snapshot_header(payload.len()), &payload, &mut bytes).unwrap();

        let mut bad = bytes;
        bad[0] = b'X';
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Magic));
        bad = bytes;
        bad[4] = 2;
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Version));
        bad = bytes;
        bad[8] = 47;
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::HeaderLength));
        bad = bytes;
        bad[10..12].copy_from_slice(&0x6666_u16.to_le_bytes());
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::MessageKind));
        bad = bytes;
        bad[12..16].copy_from_slice(&0x0001_0000_u32.to_le_bytes());
        assert_eq!(
            parse_kps1_frame(&bad),
            Err(Kps1Error::UnsupportedRequiredFlags)
        );
        bad = bytes;
        bad[16..24].fill(0);
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Session));
        bad = bytes;
        bad[24..32].fill(0);
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Sequence));
        bad = bytes;
        bad[32] = 1;
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Correlation));
        bad = bytes;
        bad[50] ^= 1;
        assert_eq!(parse_kps1_frame(&bad), Err(Kps1Error::Checksum));
        assert_eq!(parse_kps1_frame(&bytes[..50]), Err(Kps1Error::Length));
    }

    #[test]
    fn frame_sizes_and_evidence_limits_fail_closed() {
        let oversized = Kps1Header {
            payload_length: (KPS1_MAX_PAYLOAD_LENGTH + 1) as u32,
            ..snapshot_header(0)
        };
        assert_eq!(oversized.validate(), Err(Kps1Error::PayloadTooLarge));
        let chunk = Kps1Header {
            kind: PresentationMessageKind::EvidenceChunk,
            payload_length: (KPS1_EVIDENCE_CHUNK_MAX_LENGTH + 1) as u32,
            ..snapshot_header(0)
        };
        assert_eq!(chunk.validate(), Err(Kps1Error::EvidenceChunkTooLarge));
    }

    #[test]
    fn sequence_cursor_rejects_stale_session_duplicates_and_reordering() {
        let mut cursor = Kps1SequenceCursor::new(9, 4).unwrap();
        let base = Kps1Header {
            kind: PresentationMessageKind::Snapshot,
            flags: 0,
            session_nonce: 9,
            sequence: 4,
            correlation_id: 0,
            payload_length: 0,
        };
        assert_eq!(cursor.accept(base), Ok(()));
        assert_eq!(cursor.accept(base), Err(Kps1Error::Sequence));
        assert_eq!(
            cursor.accept(Kps1Header {
                sequence: 5,
                session_nonce: 10,
                ..base
            }),
            Err(Kps1Error::Session)
        );
        assert_eq!(cursor.next_sequence(), 5);
    }

    #[test]
    fn action_intent_payload_has_strict_reserved_bytes_and_role_policy() {
        let intent = PresentationActionIntent {
            proposal_identity: 0x120b_0001,
            expected_load_identity: 0x11c0_0001,
            operation: PresentationActionOperation::Commit,
            requested_activation_epoch: 5_123,
            client_action_sequence: 9,
        };
        let mut bytes = [0_u8; ACTION_INTENT_PAYLOAD_LENGTH];
        write_action_intent_payload(intent, PresentationRole::GuidedOperator, &mut bytes).unwrap();
        const ACTION_GOLDEN: [u8; 32] = [
            80, 65, 73, 49, 1, 0, 32, 0, 1, 0, 11, 18, 1, 0, 192, 17, 3, 0, 0, 0, 3, 20, 0, 0, 9,
            0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, ACTION_GOLDEN);
        assert_eq!(
            parse_action_intent_payload(&bytes, PresentationRole::GuidedOperator),
            Ok(intent)
        );
        bytes[18] = 1;
        assert_eq!(
            parse_action_intent_payload(&bytes, PresentationRole::GuidedOperator),
            Err(Kps1Error::Reserved)
        );
        bytes[18] = 0;
        assert_eq!(
            parse_action_intent_payload(&bytes, PresentationRole::Observer),
            Err(Kps1Error::Enum)
        );
    }

    #[test]
    fn independent_cursor_payload_round_trips() {
        let cursors = PresentationCursors {
            snapshots: 2,
            events: 3,
            timeline: 4,
            action_receipts: 5,
            release_samples: 6,
        };
        let mut bytes = [0_u8; CURSORS_PAYLOAD_LENGTH];
        write_cursors_payload(cursors, &mut bytes).unwrap();
        const CURSORS_GOLDEN: [u8; 48] = [
            75, 67, 82, 49, 1, 0, 48, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0,
            0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, CURSORS_GOLDEN);
        assert_eq!(parse_cursors_payload(&bytes), Ok(cursors));
        bytes[40..48].fill(0);
        assert_eq!(parse_cursors_payload(&bytes), Err(Kps1Error::Cursor));
    }

    #[test]
    fn published_cross_language_vectors_match_the_frozen_bytes() {
        let vectors = include_str!("../vectors/kps1-v1.json");
        assert!(vectors.contains("4b50533101000000300000010400000008070605040302012a00000000000000000000000000000004000000a0ed0a0c10203040"));
        assert!(
            vectors.contains("504149310100200001000b120100c01103000000031400000900000000000000")
        );
        assert!(vectors.contains("4b4352310100300002000000000000000300000000000000040000000000000005000000000000000600000000000000"));
        assert!(vectors.contains("50454d310100300001e0b51278563412e86c2c0000000000dcff00002d00000001010000000000000000000000000000"));
    }

    #[test]
    fn sealed_evidence_metadata_enforces_object_and_chunk_contracts() {
        let metadata = SealedEvidenceMetadata {
            evidence_identity: 0x12b5_e001,
            evidence_crc32: 0x1234_5678,
            total_length: 2_911_464,
            chunk_length: KPS1_EVIDENCE_CHUNK_DATA_MAX_LENGTH as u32,
            chunk_count: 45,
            complete: true,
            content_kind: 1,
        };
        let mut bytes = [0_u8; EVIDENCE_METADATA_PAYLOAD_LENGTH];
        write_evidence_metadata_payload(metadata, &mut bytes).unwrap();
        const METADATA_GOLDEN: [u8; 48] = [
            80, 69, 77, 49, 1, 0, 48, 0, 1, 224, 181, 18, 120, 86, 52, 18, 232, 108, 44, 0, 0, 0,
            0, 0, 220, 255, 0, 0, 45, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, METADATA_GOLDEN);
        assert_eq!(parse_evidence_metadata_payload(&bytes), Ok(metadata));

        let too_large = SealedEvidenceMetadata {
            total_length: KPS1_EVIDENCE_OBJECT_MAX_LENGTH + 1,
            ..metadata
        };
        assert_eq!(too_large.validate(), Err(Kps1Error::EvidenceObjectTooLarge));
        let wrong_count = SealedEvidenceMetadata {
            chunk_count: 44,
            ..metadata
        };
        assert_eq!(wrong_count.validate(), Err(Kps1Error::ChunkCount));
        bytes[47] = 1;
        assert_eq!(
            parse_evidence_metadata_payload(&bytes),
            Err(Kps1Error::Reserved)
        );
    }
}
