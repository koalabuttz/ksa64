//! KSB11 segmented, identity-bound mission-session archives.

use ksa64_interface::crc32_ieee;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const KSB11_HEADER_LENGTH: usize = 64;
pub const KSB11_TRAILER_LENGTH: usize = 4;
pub const KSB11_MANIFEST_LENGTH: usize = 44;
pub const KSB11_MAX_SEGMENT_PAYLOAD: usize = 16 * 1024 * 1024;
pub const KSB11_FLAG_FINAL: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum SessionSegmentKind {
    SourceLedger = 1,
    EarthPack = 2,
    EnvironmentPack = 3,
    VehiclePack = 4,
    MissionPack = 5,
    AvionicsPack = 6,
    EffectorPack = 7,
    PackageManifest = 8,
    MissionPlan = 9,
    ProcedurePack = 10,
    FaultSchedule = 11,
    GroundObservations = 12,
    CanonicalTelemetry = 13,
    PredictionProducts = 14,
    ActionLog = 15,
    PackageJournal = 16,
    ProcedureEvidence = 17,
    Debrief = 18,
    IntegrityManifest = 19,
}

impl SessionSegmentKind {
    fn parse(value: u16) -> Result<Self, SessionBundleError> {
        match value {
            1 => Ok(Self::SourceLedger),
            2 => Ok(Self::EarthPack),
            3 => Ok(Self::EnvironmentPack),
            4 => Ok(Self::VehiclePack),
            5 => Ok(Self::MissionPack),
            6 => Ok(Self::AvionicsPack),
            7 => Ok(Self::EffectorPack),
            8 => Ok(Self::PackageManifest),
            9 => Ok(Self::MissionPlan),
            10 => Ok(Self::ProcedurePack),
            11 => Ok(Self::FaultSchedule),
            12 => Ok(Self::GroundObservations),
            13 => Ok(Self::CanonicalTelemetry),
            14 => Ok(Self::PredictionProducts),
            15 => Ok(Self::ActionLog),
            16 => Ok(Self::PackageJournal),
            17 => Ok(Self::ProcedureEvidence),
            18 => Ok(Self::Debrief),
            19 => Ok(Self::IntegrityManifest),
            _ => Err(SessionBundleError::Kind),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionBundleIdentity {
    pub definition: u32,
    pub actions: u32,
    pub completed_evidence: u32,
}

impl SessionBundleIdentity {
    pub fn is_valid(self) -> bool {
        self.definition != 0
            && ((self.actions == 0 && self.completed_evidence == 0)
                || (self.actions != 0 && self.completed_evidence != 0))
    }

    pub const fn is_completed(self) -> bool {
        self.actions != 0 && self.completed_evidence != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSegment {
    pub kind: SessionSegmentKind,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionBundleScan {
    pub identity: SessionBundleIdentity,
    pub segments: Vec<SessionSegment>,
    pub valid_length: usize,
    pub manifest_sha256: Option<[u8; 32]>,
    pub sealed: bool,
    pub completed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionBundleError {
    Io,
    Length,
    Magic,
    Version,
    Identity,
    Sequence,
    Kind,
    Flags,
    Reserved,
    Checksum,
    Manifest,
}

pub struct SessionBundleBuilder {
    identity: SessionBundleIdentity,
    segments: Vec<SessionSegment>,
}

impl SessionBundleBuilder {
    pub fn new(identity: SessionBundleIdentity) -> Result<Self, SessionBundleError> {
        if !identity.is_valid() {
            return Err(SessionBundleError::Identity);
        }
        Ok(Self {
            identity,
            segments: Vec::new(),
        })
    }

    pub fn push(
        &mut self,
        kind: SessionSegmentKind,
        payload: impl Into<Vec<u8>>,
    ) -> Result<(), SessionBundleError> {
        if kind == SessionSegmentKind::IntegrityManifest || self.segments.len() >= u16::MAX as usize
        {
            return Err(SessionBundleError::Kind);
        }
        let payload = payload.into();
        if payload.len() > KSB11_MAX_SEGMENT_PAYLOAD {
            return Err(SessionBundleError::Length);
        }
        self.segments.push(SessionSegment { kind, payload });
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, SessionBundleError> {
        if self.segments.is_empty() {
            return Err(SessionBundleError::Length);
        }
        let mut output = Vec::new();
        let mut prior_crc = 0u32;
        for (sequence, segment) in self.segments.iter().enumerate() {
            let bytes = encode_segment(
                self.identity,
                sequence as u32,
                segment.kind,
                0,
                prior_crc,
                &segment.payload,
            )?;
            prior_crc = read_u32(&bytes, bytes.len() - 4);
            output.extend_from_slice(&bytes);
        }
        let prefix_sha256 = sha256(&output);
        let mut manifest = [0; KSB11_MANIFEST_LENGTH];
        manifest[..4].copy_from_slice(b"KSM1");
        write_u32(&mut manifest, 4, output.len() as u32);
        write_u32(&mut manifest, 8, self.segments.len() as u32);
        manifest[12..44].copy_from_slice(&prefix_sha256);
        let final_segment = encode_segment(
            self.identity,
            self.segments.len() as u32,
            SessionSegmentKind::IntegrityManifest,
            KSB11_FLAG_FINAL,
            prior_crc,
            &manifest,
        )?;
        output.extend_from_slice(&final_segment);
        Ok(output)
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), SessionBundleError> {
        let bytes = self.encode()?;
        let temporary = temporary_path(path);
        {
            let mut file = File::create(&temporary).map_err(|_| SessionBundleError::Io)?;
            file.write_all(&bytes).map_err(|_| SessionBundleError::Io)?;
            file.sync_all().map_err(|_| SessionBundleError::Io)?;
        }
        fs::rename(temporary, path).map_err(|_| SessionBundleError::Io)
    }
}

pub fn scan_session_bundle(input: &[u8]) -> Result<SessionBundleScan, SessionBundleError> {
    let mut offset = 0usize;
    let mut expected_identity = None;
    let mut expected_sequence = 0u32;
    let mut expected_prior_crc = 0u32;
    let mut segments = Vec::new();
    let mut manifest_sha256 = None;
    let mut sealed = false;

    while offset < input.len() {
        if input.len() - offset < KSB11_HEADER_LENGTH {
            break;
        }
        if input[offset..offset + 4] != *b"KSB1" {
            return Err(SessionBundleError::Magic);
        }
        if read_u16(input, offset + 4) != 11
            || read_u16(input, offset + 6) as usize != KSB11_HEADER_LENGTH
        {
            return Err(SessionBundleError::Version);
        }
        let length = read_u32(input, offset + 8) as usize;
        let payload_length = read_u32(input, offset + 32) as usize;
        if payload_length > KSB11_MAX_SEGMENT_PAYLOAD
            || length != KSB11_HEADER_LENGTH + payload_length + KSB11_TRAILER_LENGTH
            || length < KSB11_HEADER_LENGTH + KSB11_TRAILER_LENGTH
        {
            return Err(SessionBundleError::Length);
        }
        if offset + length > input.len() {
            break;
        }
        let identity = SessionBundleIdentity {
            definition: read_u32(input, offset + 12),
            actions: read_u32(input, offset + 16),
            completed_evidence: read_u32(input, offset + 20),
        };
        if !identity.is_valid() {
            return Err(SessionBundleError::Identity);
        }
        if let Some(expected) = expected_identity {
            if identity != expected {
                return Err(SessionBundleError::Identity);
            }
        } else {
            expected_identity = Some(identity);
        }
        if read_u32(input, offset + 24) != expected_sequence {
            return Err(SessionBundleError::Sequence);
        }
        let kind = SessionSegmentKind::parse(read_u16(input, offset + 28))?;
        let flags = read_u16(input, offset + 30);
        if flags & !KSB11_FLAG_FINAL != 0 {
            return Err(SessionBundleError::Flags);
        }
        if input[offset + 44..offset + KSB11_HEADER_LENGTH]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(SessionBundleError::Reserved);
        }
        let payload_crc = read_u32(input, offset + 36);
        let prior_crc = read_u32(input, offset + 40);
        let payload_start = offset + KSB11_HEADER_LENGTH;
        let payload_end = payload_start + payload_length;
        if payload_crc != crc32_ieee(&input[payload_start..payload_end])
            || prior_crc != expected_prior_crc
            || read_u32(input, offset + length - 4)
                != crc32_ieee(&input[offset..offset + length - 4])
        {
            return Err(SessionBundleError::Checksum);
        }
        let segment_crc = read_u32(input, offset + length - 4);
        if kind == SessionSegmentKind::IntegrityManifest {
            if flags != KSB11_FLAG_FINAL
                || payload_length != KSB11_MANIFEST_LENGTH
                || input[payload_start..payload_start + 4] != *b"KSM1"
                || read_u32(input, payload_start + 4) as usize != offset
                || read_u32(input, payload_start + 8) != segments.len() as u32
            {
                return Err(SessionBundleError::Manifest);
            }
            let mut expected_hash = [0; 32];
            expected_hash.copy_from_slice(&input[payload_start + 12..payload_start + 44]);
            if expected_hash != sha256(&input[..offset]) || offset + length != input.len() {
                return Err(SessionBundleError::Manifest);
            }
            manifest_sha256 = Some(expected_hash);
            sealed = true;
            offset += length;
            break;
        }
        if flags != 0 {
            return Err(SessionBundleError::Flags);
        }
        segments.push(SessionSegment {
            kind,
            payload: input[payload_start..payload_end].to_vec(),
        });
        expected_prior_crc = segment_crc;
        expected_sequence = expected_sequence.saturating_add(1);
        offset += length;
    }

    let identity = expected_identity.unwrap_or(SessionBundleIdentity {
        definition: 0,
        actions: 0,
        completed_evidence: 0,
    });
    Ok(SessionBundleScan {
        identity,
        segments,
        valid_length: offset,
        manifest_sha256,
        sealed,
        completed: sealed && identity.is_completed(),
    })
}

pub fn verify_complete_session(input: &[u8]) -> Result<SessionBundleScan, SessionBundleError> {
    let scan = scan_session_bundle(input)?;
    if !scan.sealed || !scan.completed || scan.valid_length != input.len() {
        return Err(SessionBundleError::Manifest);
    }
    if !scan
        .segments
        .iter()
        .any(|segment| segment.kind == SessionSegmentKind::Debrief)
        || !scan
            .segments
            .iter()
            .any(|segment| segment.kind == SessionSegmentKind::ActionLog)
    {
        return Err(SessionBundleError::Manifest);
    }
    Ok(scan)
}

fn encode_segment(
    identity: SessionBundleIdentity,
    sequence: u32,
    kind: SessionSegmentKind,
    flags: u16,
    prior_crc: u32,
    payload: &[u8],
) -> Result<Vec<u8>, SessionBundleError> {
    if !identity.is_valid()
        || payload.len() > KSB11_MAX_SEGMENT_PAYLOAD
        || flags & !KSB11_FLAG_FINAL != 0
    {
        return Err(SessionBundleError::Length);
    }
    let length = KSB11_HEADER_LENGTH + payload.len() + KSB11_TRAILER_LENGTH;
    let mut output = vec![0; length];
    output[..4].copy_from_slice(b"KSB1");
    write_u16(&mut output, 4, 11);
    write_u16(&mut output, 6, KSB11_HEADER_LENGTH as u16);
    write_u32(&mut output, 8, length as u32);
    write_u32(&mut output, 12, identity.definition);
    write_u32(&mut output, 16, identity.actions);
    write_u32(&mut output, 20, identity.completed_evidence);
    write_u32(&mut output, 24, sequence);
    write_u16(&mut output, 28, kind as u16);
    write_u16(&mut output, 30, flags);
    write_u32(&mut output, 32, payload.len() as u32);
    write_u32(&mut output, 36, crc32_ieee(payload));
    write_u32(&mut output, 40, prior_crc);
    output[KSB11_HEADER_LENGTH..KSB11_HEADER_LENGTH + payload.len()].copy_from_slice(payload);
    let crc = crc32_ieee(&output[..length - 4]);
    write_u32(&mut output, length - 4, crc);
    Ok(output)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

pub fn sha256(input: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_length = (input.len() as u64).wrapping_mul(8);
    let padded_length = (input.len() + 9).div_ceil(64) * 64;
    let mut padded = vec![0; padded_length];
    padded[..input.len()].copy_from_slice(input);
    padded[input.len()] = 0x80;
    padded[padded_length - 8..].copy_from_slice(&bit_length.to_be_bytes());
    let mut state = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                block[index * 4],
                block[index * 4 + 1],
                block[index * 4 + 2],
                block[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (value, addition) in state.iter_mut().zip([a, b, c, d, e, f, g, h].into_iter()) {
            *value = value.wrapping_add(addition);
        }
    }
    let mut output = [0; 32];
    for (index, word) in state.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed_builder() -> SessionBundleBuilder {
        let mut builder = SessionBundleBuilder::new(SessionBundleIdentity {
            definition: 1,
            actions: 2,
            completed_evidence: 3,
        })
        .unwrap();
        builder
            .push(SessionSegmentKind::SourceLedger, b"source".to_vec())
            .unwrap();
        builder
            .push(SessionSegmentKind::ActionLog, b"actions".to_vec())
            .unwrap();
        builder
            .push(SessionSegmentKind::Debrief, b"debrief".to_vec())
            .unwrap();
        builder
    }

    #[test]
    fn sha256_matches_standard_vectors() {
        assert_eq!(
            sha256(b""),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ]
        );
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn completed_archive_is_strict_and_corruption_fails() {
        let bytes = completed_builder().encode().unwrap();
        let scan = verify_complete_session(&bytes).unwrap();
        assert!(scan.completed);
        assert_eq!(scan.segments.len(), 3);
        let mut corrupt = bytes;
        corrupt[KSB11_HEADER_LENGTH] ^= 1;
        assert_eq!(
            scan_session_bundle(&corrupt),
            Err(SessionBundleError::Checksum)
        );
    }

    #[test]
    fn truncated_archive_exposes_only_validated_prefix_and_never_completes() {
        let bytes = completed_builder().encode().unwrap();
        let first_length = read_u32(&bytes, 8) as usize;
        for length in [first_length, first_length + 7, bytes.len() - 1] {
            let scan = scan_session_bundle(&bytes[..length]).unwrap();
            assert!(!scan.completed);
            assert!(!scan.sealed);
            assert!(scan.valid_length <= length);
        }
        assert_eq!(
            verify_complete_session(&bytes[..bytes.len() - 1]),
            Err(SessionBundleError::Manifest)
        );
    }

    #[test]
    fn definition_only_bundle_is_sealed_but_not_completed() {
        let mut builder = SessionBundleBuilder::new(SessionBundleIdentity {
            definition: 9,
            actions: 0,
            completed_evidence: 0,
        })
        .unwrap();
        builder
            .push(SessionSegmentKind::SourceLedger, b"source".to_vec())
            .unwrap();
        let scan = scan_session_bundle(&builder.encode().unwrap()).unwrap();
        assert!(scan.sealed);
        assert!(!scan.completed);
    }
}
