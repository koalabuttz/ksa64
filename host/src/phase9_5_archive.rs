//! Strict segmented KAE9 archives and bounded KFE9 finalist packages.
use crate::phase9_5_workbench::{AdvancedSearchResult, AdvancedStudyId};
use ksa64_core::phase9_5_contract::{parse_advanced_effector_summary, KAS9_LENGTH};
use ksa64_core::phase9_contract::{CandidateAggregate, DesignVector, KDV9_LENGTH, KOE9_LENGTH};
use ksa64_interface::crc32_ieee;
use std::collections::{BTreeMap, BTreeSet};

pub const KAE9_HEADER_LENGTH: usize = 128;
pub const KAE9_SEGMENT_HEADER_LENGTH: usize = 32;
pub const KAE9_CANDIDATE_RECORD_LENGTH: usize = KDV9_LENGTH + KOE9_LENGTH;
pub const KAE9_EVIDENCE_RECORD_LENGTH: usize = 8 + KAS9_LENGTH;
pub const KFE9_HEADER_LENGTH: usize = 128;
pub const KFE9_RECORD_LENGTH: usize = KDV9_LENGTH + KOE9_LENGTH + KAS9_LENGTH;
pub const KFE9_MAX_FINALISTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedArchiveError {
    Length,
    Magic,
    Version,
    Reserved,
    Checksum,
    Identity,
    Record,
    Count,
}
fn w16(o: &mut [u8], p: usize, v: u16) {
    o[p..p + 2].copy_from_slice(&v.to_le_bytes())
}
fn w32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn r16(i: &[u8], p: usize) -> u16 {
    u16::from_le_bytes(i[p..p + 2].try_into().unwrap())
}
fn r32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
fn segment(kind: u16, index: u16, count: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; KAE9_SEGMENT_HEADER_LENGTH + payload.len()];
    out[..4].copy_from_slice(b"SE95");
    w16(&mut out, 4, 1);
    w16(&mut out, 6, kind);
    w16(&mut out, 8, index);
    w32(&mut out, 12, count);
    w32(&mut out, 16, payload.len() as u32);
    w32(&mut out, 20, crc32_ieee(payload));
    let identity = crc32_ieee(&out[4..24]);
    w32(&mut out, 24, identity);
    let hcrc = crc32_ieee(&out[..28]);
    w32(&mut out, 28, hcrc);
    out[32..].copy_from_slice(payload);
    out
}
fn header(
    magic: &[u8; 4],
    manifest: u32,
    study: u32,
    count: u32,
    payload: &[u8],
    record_len: u32,
) -> [u8; 128] {
    let mut h = [0u8; 128];
    h[..4].copy_from_slice(magic);
    w16(&mut h, 4, 1);
    w16(&mut h, 6, 128);
    w32(&mut h, 8, manifest);
    w32(&mut h, 12, study);
    w32(&mut h, 16, count);
    w32(&mut h, 20, payload.len() as u32);
    w32(&mut h, 24, crc32_ieee(payload));
    w32(&mut h, 28, record_len);
    let id = crc32_ieee(&h[4..32]);
    w32(&mut h, 32, id.max(1));
    let crc = crc32_ieee(&h[..124]);
    w32(&mut h, 124, crc);
    h
}

pub fn encode_kae9(
    result: &AdvancedSearchResult,
    study: AdvancedStudyId,
) -> Result<Vec<u8>, AdvancedArchiveError> {
    let mut segments = Vec::new();
    for g in &result.search.generations {
        let mut payload = Vec::with_capacity(g.candidates.len() * KAE9_CANDIDATE_RECORD_LENGTH);
        for (c, a) in g.candidates.iter().zip(&g.aggregates) {
            payload.extend_from_slice(&c.encode().map_err(|_| AdvancedArchiveError::Record)?);
            payload.extend_from_slice(&a.encode().map_err(|_| AdvancedArchiveError::Record)?)
        }
        segments.push(segment(1, g.index, g.candidates.len() as u32, &payload))
    }
    for ((candidate, tier), evaluation) in &result.evidence {
        let mut payload = Vec::with_capacity(evaluation.cases.len() * KAE9_EVIDENCE_RECORD_LENGTH);
        for (index, case) in evaluation.cases.iter().enumerate() {
            payload.extend_from_slice(&candidate.to_le_bytes());
            payload.push(*tier);
            payload.push(index as u8);
            payload.extend_from_slice(&[0, 0]);
            payload.extend_from_slice(&case.kas9)
        }
        segments.push(segment(
            2,
            *tier as u16,
            evaluation.cases.len() as u32,
            &payload,
        ))
    }
    let total: usize = segments.iter().map(Vec::len).sum();
    let mut payload = Vec::with_capacity(total);
    for s in segments {
        payload.extend_from_slice(&s)
    }
    let h = header(
        b"KAE9",
        result.search.manifest_identity,
        study.raw(),
        result.search.generations.len() as u32 + result.evidence.len() as u32,
        &payload,
        0,
    );
    let mut out = Vec::with_capacity(128 + payload.len());
    out.extend_from_slice(&h);
    out.extend_from_slice(&payload);
    Ok(out)
}
fn validate_header(
    input: &[u8],
    magic: &[u8; 4],
) -> Result<(u32, u32, u32, u32), AdvancedArchiveError> {
    if input.len() < 128 {
        return Err(AdvancedArchiveError::Length);
    }
    if &input[..4] != magic {
        return Err(AdvancedArchiveError::Magic);
    }
    if r16(input, 4) != 1 || r16(input, 6) != 128 {
        return Err(AdvancedArchiveError::Version);
    }
    if input[36..124].iter().any(|b| *b != 0) {
        return Err(AdvancedArchiveError::Reserved);
    }
    if r32(input, 124) != crc32_ieee(&input[..124]) || r32(input, 24) != crc32_ieee(&input[128..]) {
        return Err(AdvancedArchiveError::Checksum);
    }
    if r32(input, 20) as usize != input.len() - 128 {
        return Err(AdvancedArchiveError::Length);
    }
    Ok((
        r32(input, 8),
        r32(input, 12),
        r32(input, 16),
        r32(input, 28),
    ))
}
pub fn validate_kae9(input: &[u8]) -> Result<(), AdvancedArchiveError> {
    let (manifest, _study, count, _) = validate_header(input, b"KAE9")?;
    let mut at = 128;
    let mut segments = 0u32;
    while at < input.len() {
        if at + 32 > input.len() {
            return Err(AdvancedArchiveError::Length);
        }
        let h = &input[at..at + 32];
        if &h[..4] != b"SE95" || r16(h, 4) != 1 {
            return Err(AdvancedArchiveError::Magic);
        }
        if h[10..12].iter().any(|b| *b != 0)
            || r32(h, 28) != crc32_ieee(&h[..28])
            || r32(h, 24) == 0
        {
            return Err(AdvancedArchiveError::Checksum);
        }
        let kind = r16(h, 6);
        let n = r32(h, 12) as usize;
        let len = r32(h, 16) as usize;
        if at + 32 + len > input.len() {
            return Err(AdvancedArchiveError::Length);
        }
        let p = &input[at + 32..at + 32 + len];
        if crc32_ieee(p) != r32(h, 20) {
            return Err(AdvancedArchiveError::Checksum);
        }
        match kind {
            1 => {
                if len != n * KAE9_CANDIDATE_RECORD_LENGTH {
                    return Err(AdvancedArchiveError::Length);
                }
                for i in 0..n {
                    let o = i * KAE9_CANDIDATE_RECORD_LENGTH;
                    let d = DesignVector::parse(&p[o..o + KDV9_LENGTH])
                        .map_err(|_| AdvancedArchiveError::Record)?;
                    let a = CandidateAggregate::parse(
                        &p[o + KDV9_LENGTH..o + KAE9_CANDIDATE_RECORD_LENGTH],
                    )
                    .map_err(|_| AdvancedArchiveError::Record)?;
                    if d.manifest_identity != manifest
                        || a.manifest_identity != manifest
                        || d.identity != a.candidate_identity
                    {
                        return Err(AdvancedArchiveError::Identity);
                    }
                }
            }
            2 => {
                if len != n * KAE9_EVIDENCE_RECORD_LENGTH {
                    return Err(AdvancedArchiveError::Length);
                }
                for i in 0..n {
                    let o = i * KAE9_EVIDENCE_RECORD_LENGTH;
                    if p[o + 6] != 0
                        || p[o + 7] != 0
                        || p[o + 4] == 0
                        || parse_advanced_effector_summary(
                            &p[o + 8..o + KAE9_EVIDENCE_RECORD_LENGTH],
                        )
                        .is_err()
                    {
                        return Err(AdvancedArchiveError::Record);
                    }
                }
            }
            _ => return Err(AdvancedArchiveError::Version),
        }
        at += 32 + len;
        segments += 1
    }
    if segments != count {
        return Err(AdvancedArchiveError::Count);
    }
    Ok(())
}

pub fn encode_kfe9(
    result: &AdvancedSearchResult,
    study: AdvancedStudyId,
) -> Result<Vec<u8>, AdvancedArchiveError> {
    let last = result
        .search
        .generations
        .last()
        .ok_or(AdvancedArchiveError::Count)?;
    let mut by_id: BTreeMap<u32, (DesignVector, CandidateAggregate)> = BTreeMap::new();
    for g in &result.search.generations {
        for (c, a) in g.candidates.iter().zip(&g.aggregates) {
            by_id.insert(c.identity, (*c, *a));
        }
    }
    let finalist_ids: BTreeSet<u32> = result
        .search
        .finalists
        .iter()
        .filter(|x| x.aggregate.feasible)
        .map(|x| x.aggregate.candidate_identity)
        .collect();
    let mut records = Vec::new();
    for id in finalist_ids.into_iter().take(KFE9_MAX_FINALISTS) {
        let (c, _) = by_id.get(&id).ok_or(AdvancedArchiveError::Identity)?;
        let evaluation = result
            .evidence
            .get(&(id, 64))
            .or_else(|| result.evidence.get(&(id, 8)))
            .ok_or(AdvancedArchiveError::Record)?;
        let a = evaluation.aggregate;
        let case = evaluation
            .cases
            .first()
            .ok_or(AdvancedArchiveError::Record)?;
        records.extend_from_slice(&c.encode().map_err(|_| AdvancedArchiveError::Record)?);
        records.extend_from_slice(&a.encode().map_err(|_| AdvancedArchiveError::Record)?);
        records.extend_from_slice(&case.kas9)
    }
    let count = records.len() / KFE9_RECORD_LENGTH;
    if count > last.candidates.len() {
        return Err(AdvancedArchiveError::Count);
    }
    let h = header(
        b"KFE9",
        result.search.manifest_identity,
        study.raw(),
        count as u32,
        &records,
        KFE9_RECORD_LENGTH as u32,
    );
    let mut out = Vec::with_capacity(128 + records.len());
    out.extend_from_slice(&h);
    out.extend_from_slice(&records);
    Ok(out)
}
pub fn validate_kfe9(input: &[u8]) -> Result<(), AdvancedArchiveError> {
    let (manifest, _study, count, record) = validate_header(input, b"KFE9")?;
    if record as usize != KFE9_RECORD_LENGTH
        || count as usize > KFE9_MAX_FINALISTS
        || input.len() != 128 + count as usize * KFE9_RECORD_LENGTH
    {
        return Err(AdvancedArchiveError::Length);
    }
    for i in 0..count as usize {
        let o = 128 + i * KFE9_RECORD_LENGTH;
        let d = DesignVector::parse(&input[o..o + KDV9_LENGTH])
            .map_err(|_| AdvancedArchiveError::Record)?;
        let a = CandidateAggregate::parse(&input[o + KDV9_LENGTH..o + KDV9_LENGTH + KOE9_LENGTH])
            .map_err(|_| AdvancedArchiveError::Record)?;
        if d.manifest_identity != manifest
            || a.manifest_identity != manifest
            || d.identity != a.candidate_identity
            || !a.feasible
        {
            return Err(AdvancedArchiveError::Identity);
        }
        parse_advanced_effector_summary(
            &input[o + KDV9_LENGTH + KOE9_LENGTH..o + KFE9_RECORD_LENGTH],
        )
        .map_err(|_| AdvancedArchiveError::Record)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFinalistRecord {
    pub design: DesignVector,
    pub aggregate: CandidateAggregate,
    pub summary: ksa64_core::phase9_5_contract::AdvancedEffectorEvaluationSummary,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFinalistPackage<'a> {
    input: &'a [u8],
    pub manifest_identity: u32,
    pub study_identity: u32,
    pub count: u8,
}
impl<'a> AdvancedFinalistPackage<'a> {
    pub fn parse(input: &'a [u8]) -> Result<Self, AdvancedArchiveError> {
        validate_kfe9(input)?;
        let (manifest, study, count, _) = validate_header(input, b"KFE9")?;
        Ok(Self {
            input,
            manifest_identity: manifest,
            study_identity: study,
            count: count as u8,
        })
    }
    pub fn record(&self, index: usize) -> Result<AdvancedFinalistRecord, AdvancedArchiveError> {
        if index >= self.count as usize {
            return Err(AdvancedArchiveError::Count);
        }
        let o = KFE9_HEADER_LENGTH + index * KFE9_RECORD_LENGTH;
        let design = DesignVector::parse(&self.input[o..o + KDV9_LENGTH])
            .map_err(|_| AdvancedArchiveError::Record)?;
        let aggregate =
            CandidateAggregate::parse(&self.input[o + KDV9_LENGTH..o + KDV9_LENGTH + KOE9_LENGTH])
                .map_err(|_| AdvancedArchiveError::Record)?;
        let summary = parse_advanced_effector_summary(
            &self.input[o + KDV9_LENGTH + KOE9_LENGTH..o + KFE9_RECORD_LENGTH],
        )
        .map_err(|_| AdvancedArchiveError::Record)?
        .summary;
        Ok(AdvancedFinalistRecord {
            design,
            aggregate,
            summary,
        })
    }
}

pub fn subset_kfe9(input: &[u8], indices: &[usize]) -> Result<Vec<u8>, AdvancedArchiveError> {
    let package = AdvancedFinalistPackage::parse(input)?;
    if indices.len() > KFE9_MAX_FINALISTS {
        return Err(AdvancedArchiveError::Count);
    }
    let mut seen = BTreeSet::new();
    let mut payload = Vec::with_capacity(indices.len() * KFE9_RECORD_LENGTH);
    for index in indices {
        if !seen.insert(*index) || *index >= package.count as usize {
            return Err(AdvancedArchiveError::Count);
        }
        let at = KFE9_HEADER_LENGTH + *index * KFE9_RECORD_LENGTH;
        payload.extend_from_slice(&input[at..at + KFE9_RECORD_LENGTH]);
    }
    let h = header(
        b"KFE9",
        package.manifest_identity,
        package.study_identity,
        indices.len() as u32,
        &payload,
        KFE9_RECORD_LENGTH as u32,
    );
    let mut out = Vec::with_capacity(KFE9_HEADER_LENGTH + payload.len());
    out.extend_from_slice(&h);
    out.extend_from_slice(&payload);
    validate_kfe9(&out)?;
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedRetentionPlan {
    pub capacity_bytes: usize,
    pub retained_summaries: usize,
    pub full_histories: usize,
    pub compact_histories: usize,
    pub unused_bytes: usize,
}
pub fn plan_advanced_retention(
    reu_kib: usize,
    finalist_count: usize,
    full_history_bytes: usize,
    compact_history_bytes: usize,
) -> AdvancedRetentionPlan {
    if reu_kib == 0 {
        return AdvancedRetentionPlan {
            capacity_bytes: 0,
            retained_summaries: finalist_count.min(5),
            full_histories: 0,
            compact_histories: usize::from(finalist_count != 0),
            unused_bytes: 0,
        };
    }
    let capacity = reu_kib.saturating_mul(1024);
    let summary_budget = capacity / 4;
    let summaries = finalist_count.min(summary_budget / KFE9_RECORD_LENGTH);
    let summary_bytes = summaries.saturating_mul(KFE9_RECORD_LENGTH);
    let mut remaining = capacity.saturating_sub(summary_bytes);
    let full = if full_history_bytes == 0 {
        0
    } else {
        finalist_count.min(remaining / full_history_bytes)
    };
    remaining = remaining.saturating_sub(full.saturating_mul(full_history_bytes));
    let compact_candidates = finalist_count.saturating_sub(full);
    let compact = if compact_history_bytes == 0 {
        0
    } else {
        compact_candidates.min(remaining / compact_history_bytes)
    };
    remaining = remaining.saturating_sub(compact.saturating_mul(compact_history_bytes));
    AdvancedRetentionPlan {
        capacity_bytes: capacity,
        retained_summaries: summaries,
        full_histories: full,
        compact_histories: compact,
        unused_bytes: remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9_5_workbench::{built_in_advanced_manifest, run_advanced_search};
    use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
    #[test]
    fn strict_archive_rejects_corruption() {
        let mut m = built_in_advanced_manifest(AdvancedStudyId::Canard, SearchEngineId::GridV1);
        m.preset = SearchPresetId::Custom;
        m.budgets.grid_points = 2;
        m.budgets.finalists = 1;
        m.budgets.max_candidates = 4;
        m = m.seal().unwrap();
        let r = run_advanced_search(&m, AdvancedStudyId::Canard, 1).unwrap();
        let bytes = encode_kae9(&r, AdvancedStudyId::Canard).unwrap();
        validate_kae9(&bytes).unwrap();
        let mut bad = bytes.clone();
        let n = bad.len();
        bad[n - 1] ^= 1;
        assert_eq!(validate_kae9(&bad), Err(AdvancedArchiveError::Checksum));
    }
    #[test]
    fn finalist_reader_subset_and_retention_are_deterministic() {
        let bytes = include_bytes!("../../phase9_5/evidence/workbench/mixed-nsga2.kfe9");
        let package = AdvancedFinalistPackage::parse(bytes).unwrap();
        assert_eq!(package.count, 8);
        assert!(package.record(0).unwrap().aggregate.feasible);
        let subset = subset_kfe9(bytes, &[0, 3, 7]).unwrap();
        let selected = AdvancedFinalistPackage::parse(&subset).unwrap();
        assert_eq!(selected.count, 3);
        assert_eq!(
            selected.record(1).unwrap().design,
            package.record(3).unwrap().design
        );
        assert_eq!(
            subset_kfe9(bytes, &[1, 1]),
            Err(AdvancedArchiveError::Count)
        );
        let stock = plan_advanced_retention(0, 8, 640_128, 4_160);
        assert_eq!(stock.retained_summaries, 5);
        assert_eq!(stock.compact_histories, 1);
        for kib in [128, 256, 512, 1024, 16_384] {
            let plan = plan_advanced_retention(kib, 8, 640_128, 4_160);
            assert!(plan.retained_summaries <= 8);
            assert!(plan.full_histories + plan.compact_histories <= 8);
            assert!(plan.unused_bytes <= plan.capacity_bytes);
        }
    }
}
