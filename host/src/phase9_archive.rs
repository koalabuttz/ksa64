//! Segmented append-only Phase 9 archives and finalist packages.
use crate::phase9::CandidateEvaluation;
use crate::phase9_search::SearchGeneration;
use ksa64_core::phase8_5_contract::{parse_avionics_summary, KAS8_LENGTH};
use ksa64_core::phase9_contract::{CandidateAggregate, DesignVector, KDV9_LENGTH, KOE9_LENGTH};
use ksa64_interface::crc32_ieee;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const KRA9_HEADER: usize = 32;
pub const KRA9_RECORD: usize = KDV9_LENGTH + KOE9_LENGTH;
pub const KRE9_HEADER: usize = 32;
pub const KFP9_HEADER: usize = 64;
pub const KFP9_MAX_FINALISTS: usize = 32;
#[derive(Debug)]
pub enum ArchiveError {
    Io,
    Length,
    Magic,
    Version,
    Identity,
    Reserved,
    Checksum,
    Decode,
    Sequence,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveScan {
    pub manifest_identity: u32,
    pub generations: Vec<SearchGeneration>,
    pub valid_length: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedCandidateEvidence {
    pub candidate_identity: u32,
    pub uncertainty_tier: u8,
    pub records: Vec<[u8; KAS8_LENGTH]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchArchiveScan {
    pub manifest_identity: u32,
    pub generations: Vec<SearchGeneration>,
    pub evidence: Vec<ArchivedCandidateEvidence>,
    pub valid_length: u64,
}

fn segment(g: &SearchGeneration, manifest: u32) -> Result<Vec<u8>, ArchiveError> {
    if g.candidates.len() != g.aggregates.len() || g.candidates.len() > u16::MAX as usize {
        return Err(ArchiveError::Length);
    }
    let len = KRA9_HEADER + g.candidates.len() * KRA9_RECORD + 4;
    let mut b = vec![0; len];
    b[0..4].copy_from_slice(b"KRA9");
    w16(&mut b, 4, 9);
    w16(&mut b, 6, KRA9_HEADER as u16);
    w32(&mut b, 8, len as u32);
    w32(&mut b, 12, manifest);
    w16(&mut b, 16, g.index);
    w16(&mut b, 18, g.candidates.len() as u16);
    w32(&mut b, 20, g.crc32);
    let mut o = KRA9_HEADER;
    for (c, a) in g.candidates.iter().zip(&g.aggregates) {
        let cb = c.encode().map_err(|_| ArchiveError::Decode)?;
        let ab = a.encode().map_err(|_| ArchiveError::Decode)?;
        b[o..o + KDV9_LENGTH].copy_from_slice(&cb);
        o += KDV9_LENGTH;
        b[o..o + KOE9_LENGTH].copy_from_slice(&ab);
        o += KOE9_LENGTH
    }
    let crc = crc32_ieee(&b[..len - 4]);
    w32(&mut b, len - 4, crc);
    Ok(b)
}
pub fn encode_archive(
    manifest: u32,
    generations: &[SearchGeneration],
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    for g in generations {
        out.extend_from_slice(&segment(g, manifest)?)
    }
    Ok(out)
}
pub fn scan_archive(input: &[u8]) -> Result<ArchiveScan, ArchiveError> {
    let mut o = 0usize;
    let mut manifest = 0;
    let mut generations = Vec::new();
    while o < input.len() {
        if input.len() - o < KRA9_HEADER {
            return Err(ArchiveError::Length);
        }
        if &input[o..o + 4] != b"KRA9" {
            return Err(ArchiveError::Magic);
        }
        if r16(input, o + 4) != 9 || r16(input, o + 6) as usize != KRA9_HEADER {
            return Err(ArchiveError::Version);
        }
        let len = r32(input, o + 8) as usize;
        if len < KRA9_HEADER + 4 || o + len > input.len() {
            return Err(ArchiveError::Length);
        }
        let id = r32(input, o + 12);
        if manifest == 0 {
            manifest = id
        } else if manifest != id {
            return Err(ArchiveError::Identity);
        }
        let index = r16(input, o + 16);
        if index as usize != generations.len() {
            return Err(ArchiveError::Sequence);
        }
        let count = r16(input, o + 18) as usize;
        if len != KRA9_HEADER + count * KRA9_RECORD + 4 {
            return Err(ArchiveError::Length);
        }
        if input[o + 24..o + KRA9_HEADER].iter().any(|v| *v != 0) {
            return Err(ArchiveError::Reserved);
        }
        if r32(input, o + len - 4) != crc32_ieee(&input[o..o + len - 4]) {
            return Err(ArchiveError::Checksum);
        }
        let mut candidates = Vec::with_capacity(count);
        let mut aggregates = Vec::with_capacity(count);
        let mut p = o + KRA9_HEADER;
        for _ in 0..count {
            candidates.push(
                DesignVector::parse(&input[p..p + KDV9_LENGTH])
                    .map_err(|_| ArchiveError::Decode)?,
            );
            p += KDV9_LENGTH;
            aggregates.push(
                CandidateAggregate::parse(&input[p..p + KOE9_LENGTH])
                    .map_err(|_| ArchiveError::Decode)?,
            );
            p += KOE9_LENGTH
        }
        let crc = r32(input, o + 20);
        let actual = super::phase9_search::generation_fingerprint(index, &candidates, &aggregates);
        if crc != actual {
            return Err(ArchiveError::Checksum);
        }
        generations.push(SearchGeneration {
            index,
            candidates,
            aggregates,
            crc32: crc,
        });
        o += len
    }
    Ok(ArchiveScan {
        manifest_identity: manifest,
        generations,
        valid_length: o as u64,
    })
}
pub fn write_archive_atomic(
    path: &Path,
    manifest: u32,
    generations: &[SearchGeneration],
) -> Result<(), ArchiveError> {
    let bytes = encode_archive(manifest, generations)?;
    let temp = temp_path(path);
    {
        let mut f = File::create(&temp).map_err(|_| ArchiveError::Io)?;
        f.write_all(&bytes).map_err(|_| ArchiveError::Io)?;
        f.sync_all().map_err(|_| ArchiveError::Io)?
    }
    fs::rename(temp, path).map_err(|_| ArchiveError::Io)
}
pub fn resume_append(
    path: &Path,
    manifest: u32,
    generations: &[SearchGeneration],
) -> Result<(), ArchiveError> {
    let existing = if path.exists() {
        let mut b = Vec::new();
        File::open(path)
            .map_err(|_| ArchiveError::Io)?
            .read_to_end(&mut b)
            .map_err(|_| ArchiveError::Io)?;
        let scan = scan_archive(&b)?;
        if scan.manifest_identity != 0 && scan.manifest_identity != manifest {
            return Err(ArchiveError::Identity);
        }
        scan.generations.len()
    } else {
        0
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| ArchiveError::Io)?;
    for g in &generations[existing..] {
        f.write_all(&segment(g, manifest)?)
            .map_err(|_| ArchiveError::Io)?;
        f.sync_data().map_err(|_| ArchiveError::Io)?
    }
    Ok(())
}
fn temp_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(".tmp");
    PathBuf::from(p)
}

fn evidence_segment(manifest: u32, value: &CandidateEvaluation) -> Result<Vec<u8>, ArchiveError> {
    let count = value.cases.len();
    if count != usize::from(value.aggregate.uncertainty_tier) || !matches!(count, 1 | 8 | 64) {
        return Err(ArchiveError::Length);
    }
    let len = KRE9_HEADER + count * KAS8_LENGTH + 4;
    let mut bytes = vec![0; len];
    bytes[0..4].copy_from_slice(b"KRE9");
    w16(&mut bytes, 4, 9);
    w16(&mut bytes, 6, KRE9_HEADER as u16);
    w32(&mut bytes, 8, len as u32);
    w32(&mut bytes, 12, manifest);
    w32(&mut bytes, 16, value.aggregate.candidate_identity);
    bytes[20] = value.aggregate.uncertainty_tier;
    bytes[21] = count as u8;
    let mut offset = KRE9_HEADER;
    for case in &value.cases {
        parse_avionics_summary(&case.kas8).map_err(|_| ArchiveError::Decode)?;
        bytes[offset..offset + KAS8_LENGTH].copy_from_slice(&case.kas8);
        offset += KAS8_LENGTH;
    }
    let crc = crc32_ieee(&bytes[..len - 4]);
    w32(&mut bytes, len - 4, crc);
    Ok(bytes)
}

pub fn encode_search_archive(
    manifest: u32,
    generations: &[SearchGeneration],
    evidence: &[CandidateEvaluation],
) -> Result<Vec<u8>, ArchiveError> {
    let mut output = encode_archive(manifest, generations)?;
    let mut ordered: Vec<&CandidateEvaluation> = evidence.iter().collect();
    ordered.sort_by_key(|value| value.aggregate.candidate_identity);
    let mut previous = None;
    for value in ordered {
        let identity = value.aggregate.candidate_identity;
        if previous == Some(identity) {
            return Err(ArchiveError::Sequence);
        }
        previous = Some(identity);
        output.extend_from_slice(&evidence_segment(manifest, value)?);
    }
    Ok(output)
}

pub fn scan_search_archive(input: &[u8]) -> Result<SearchArchiveScan, ArchiveError> {
    let mut generation_end = 0usize;
    while generation_end < input.len() && input[generation_end..].starts_with(b"KRA9") {
        if input.len() - generation_end < KRA9_HEADER {
            return Err(ArchiveError::Length);
        }
        let length = r32(input, generation_end + 8) as usize;
        if length < KRA9_HEADER + 4 || generation_end + length > input.len() {
            return Err(ArchiveError::Length);
        }
        generation_end += length;
    }
    let base = scan_archive(&input[..generation_end])?;
    let known: std::collections::BTreeSet<u32> = base
        .generations
        .iter()
        .flat_map(|generation| generation.candidates.iter().map(|value| value.identity))
        .collect();
    let mut offset = generation_end;
    let mut evidence = Vec::new();
    let mut previous = None;
    while offset < input.len() {
        if input.len() - offset < KRE9_HEADER + 4 || &input[offset..offset + 4] != b"KRE9" {
            return Err(ArchiveError::Magic);
        }
        if r16(input, offset + 4) != 9 || r16(input, offset + 6) as usize != KRE9_HEADER {
            return Err(ArchiveError::Version);
        }
        let length = r32(input, offset + 8) as usize;
        let manifest = r32(input, offset + 12);
        let candidate = r32(input, offset + 16);
        let tier = input[offset + 20];
        let count = input[offset + 21] as usize;
        if manifest != base.manifest_identity || !known.contains(&candidate) {
            return Err(ArchiveError::Identity);
        }
        if previous.is_some_and(|value| candidate <= value) {
            return Err(ArchiveError::Sequence);
        }
        previous = Some(candidate);
        if input[offset + 22..offset + KRE9_HEADER]
            .iter()
            .any(|value| *value != 0)
            || !matches!(count, 1 | 8 | 64)
            || tier as usize != count
            || length != KRE9_HEADER + count * KAS8_LENGTH + 4
            || offset + length > input.len()
        {
            return Err(ArchiveError::Length);
        }
        if r32(input, offset + length - 4) != crc32_ieee(&input[offset..offset + length - 4]) {
            return Err(ArchiveError::Checksum);
        }
        let mut records = Vec::with_capacity(count);
        let mut record_offset = offset + KRE9_HEADER;
        for _ in 0..count {
            let record: [u8; KAS8_LENGTH] = input[record_offset..record_offset + KAS8_LENGTH]
                .try_into()
                .map_err(|_| ArchiveError::Length)?;
            parse_avionics_summary(&record).map_err(|_| ArchiveError::Decode)?;
            records.push(record);
            record_offset += KAS8_LENGTH;
        }
        evidence.push(ArchivedCandidateEvidence {
            candidate_identity: candidate,
            uncertainty_tier: tier,
            records,
        });
        offset += length;
    }
    Ok(SearchArchiveScan {
        manifest_identity: base.manifest_identity,
        generations: base.generations,
        evidence,
        valid_length: offset as u64,
    })
}

pub fn resume_search_archive_atomic(
    path: &Path,
    manifest: u32,
    generations: &[SearchGeneration],
    evidence: &[CandidateEvaluation],
) -> Result<(), ArchiveError> {
    let desired = encode_search_archive(manifest, generations, evidence)?;
    if path.exists() {
        let existing = fs::read(path).map_err(|_| ArchiveError::Io)?;
        if existing == desired {
            return Ok(());
        }
        if existing.len() > desired.len() || existing != desired[..existing.len()] {
            return Err(ArchiveError::Sequence);
        }
        let mut offset = 0usize;
        while offset < existing.len() {
            if existing.len() - offset < 12 {
                return Err(ArchiveError::Length);
            }
            let magic = &existing[offset..offset + 4];
            if magic != b"KRA9" && magic != b"KRE9" {
                return Err(ArchiveError::Magic);
            }
            let length = r32(&existing, offset + 8) as usize;
            if length < 36 || offset + length > existing.len() {
                return Err(ArchiveError::Length);
            }
            offset += length;
        }
        if offset != existing.len() {
            return Err(ArchiveError::Length);
        }
    }
    let scan = scan_search_archive(&desired)?;
    if scan.valid_length != desired.len() as u64 {
        return Err(ArchiveError::Length);
    }
    let temp = temp_path(path);
    {
        let mut file = File::create(&temp).map_err(|_| ArchiveError::Io)?;
        file.write_all(&desired).map_err(|_| ArchiveError::Io)?;
        file.sync_all().map_err(|_| ArchiveError::Io)?;
    }
    fs::rename(temp, path).map_err(|_| ArchiveError::Io)
}

pub fn write_search_archive_atomic(
    path: &Path,
    manifest: u32,
    generations: &[SearchGeneration],
    evidence: &[CandidateEvaluation],
) -> Result<(), ArchiveError> {
    let bytes = encode_search_archive(manifest, generations, evidence)?;
    let scan = scan_search_archive(&bytes)?;
    if scan.valid_length != bytes.len() as u64 {
        return Err(ArchiveError::Length);
    }
    let temp = temp_path(path);
    {
        let mut file = File::create(&temp).map_err(|_| ArchiveError::Io)?;
        file.write_all(&bytes).map_err(|_| ArchiveError::Io)?;
        file.sync_all().map_err(|_| ArchiveError::Io)?;
    }
    fs::rename(temp, path).map_err(|_| ArchiveError::Io)
}

pub fn encode_kpf9(
    manifest: u32,
    study: u32,
    finalists: &[(DesignVector, CandidateAggregate)],
) -> Result<Vec<u8>, ArchiveError> {
    if finalists.len() > KFP9_MAX_FINALISTS {
        return Err(ArchiveError::Length);
    }
    let len = KFP9_HEADER + finalists.len() * KRA9_RECORD + 4;
    let mut b = vec![0; len];
    b[0..4].copy_from_slice(b"KFP9");
    w16(&mut b, 4, 9);
    w16(&mut b, 6, KFP9_HEADER as u16);
    w32(&mut b, 8, len as u32);
    w32(&mut b, 12, manifest);
    w32(&mut b, 16, study);
    w16(&mut b, 20, finalists.len() as u16);
    let mut o = KFP9_HEADER;
    for (c, a) in finalists {
        b[o..o + KDV9_LENGTH].copy_from_slice(&c.encode().map_err(|_| ArchiveError::Decode)?);
        o += KDV9_LENGTH;
        b[o..o + KOE9_LENGTH].copy_from_slice(&a.encode().map_err(|_| ArchiveError::Decode)?);
        o += KOE9_LENGTH
    }
    let crc = crc32_ieee(&b[..len - 4]);
    w32(&mut b, len - 4, crc);
    Ok(b)
}
pub fn validate_kfp9(input: &[u8]) -> Result<usize, ArchiveError> {
    if input.len() < KFP9_HEADER + 4 || &input[..4] != b"KFP9" {
        return Err(ArchiveError::Magic);
    }
    if r16(input, 4) != 9
        || r16(input, 6) as usize != KFP9_HEADER
        || r32(input, 8) as usize != input.len()
    {
        return Err(ArchiveError::Version);
    }
    let count = r16(input, 20) as usize;
    if count > KFP9_MAX_FINALISTS || input.len() != KFP9_HEADER + count * KRA9_RECORD + 4 {
        return Err(ArchiveError::Length);
    }
    if input[22..KFP9_HEADER].iter().any(|v| *v != 0) {
        return Err(ArchiveError::Reserved);
    }
    if r32(input, input.len() - 4) != crc32_ieee(&input[..input.len() - 4]) {
        return Err(ArchiveError::Checksum);
    }
    Ok(count)
}
fn r16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn r32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn w16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes())
}
fn w32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9::{baseline_vector, built_in_manifest, StudyId};
    use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
    fn fixture() -> (u32, SearchGeneration) {
        let m = built_in_manifest(
            StudyId::PassiveRecovery,
            SearchEngineId::GridV1,
            SearchPresetId::Quick,
        );
        let d = baseline_vector(&m);
        let a = CandidateAggregate {
            identity: 0,
            manifest_identity: m.identity,
            candidate_identity: d.identity,
            uncertainty_tier: 8,
            case_count: 8,
            fatal_class: 0,
            violated_constraints: 0,
            feasible: true,
            case_crc: 1,
            normalized_violation: 0,
            objective_count: 1,
            constraint_count: 0,
            objectives: [0; 8],
            constraint_values: [0; 16],
        }
        .seal();
        let c = super::super::phase9_search::generation_fingerprint(0, &[d], &[a]);
        (
            m.identity,
            SearchGeneration {
                index: 0,
                candidates: vec![d],
                aggregates: vec![a],
                crc32: c,
            },
        )
    }
    #[test]
    fn archive_roundtrip_and_corruption() {
        let (m, g) = fixture();
        let b = encode_archive(m, std::slice::from_ref(&g)).unwrap();
        assert_eq!(scan_archive(&b).unwrap().generations, vec![g]);
        let mut bad = b;
        bad[50] ^= 1;
        assert!(scan_archive(&bad).is_err())
    }
    #[test]
    fn interrupted_resume_is_byte_identical() {
        let (m, g) = fixture();
        let dir = std::env::temp_dir();
        let p = dir.join(format!("ksa64-kra9-{}.bin", std::process::id()));
        let _ = fs::remove_file(&p);
        resume_append(&p, m, std::slice::from_ref(&g)).unwrap();
        resume_append(&p, m, std::slice::from_ref(&g)).unwrap();
        let got = fs::read(&p).unwrap();
        assert_eq!(got, encode_archive(m, &[g]).unwrap());
        fs::remove_file(p).unwrap()
    }
    #[test]
    fn interrupted_search_resume_is_byte_identical() {
        let (manifest, first) = fixture();
        let mut second = first.clone();
        second.index = 1;
        second.crc32 = super::super::phase9_search::generation_fingerprint(
            second.index,
            &second.candidates,
            &second.aggregates,
        );
        let generations = vec![first.clone(), second];
        let directory = std::env::temp_dir();
        let path = directory.join(format!("ksa64-kra9-search-{}.bin", std::process::id()));
        let _ = fs::remove_file(&path);
        fs::write(&path, encode_archive(manifest, &[first]).unwrap()).unwrap();
        resume_search_archive_atomic(&path, manifest, &generations, &[]).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            encode_search_archive(manifest, &generations, &[]).unwrap()
        );
        fs::remove_file(path).unwrap()
    }

    #[test]
    fn finalist_pack_is_strict() {
        let (m, g) = fixture();
        let b = encode_kpf9(m, 1, &[(g.candidates[0], g.aggregates[0])]).unwrap();
        assert_eq!(validate_kfp9(&b).unwrap(), 1)
    }
}
