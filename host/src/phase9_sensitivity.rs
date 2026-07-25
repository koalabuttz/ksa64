//! Exact one-design-quantum sensitivity records (KSN9).
use crate::phase9_search::{CandidateEvaluator, SearchError};
use ksa64_core::phase9_contract::{DesignVector, SearchManifest};
use ksa64_interface::crc32_ieee;
pub const KSN9_LENGTH: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensitivityRecord {
    pub manifest_identity: u32,
    pub candidate_identity: u32,
    pub variable_id: u16,
    pub objective_index: u8,
    pub flags: u8,
    pub baseline: i32,
    pub lower: i32,
    pub upper: i32,
    pub delta_lower: i32,
    pub delta_upper: i32,
    pub slope_q16: i32,
}
pub fn one_at_a_time<E: CandidateEvaluator>(
    m: &SearchManifest,
    base: &DesignVector,
    e: &E,
    tier: u8,
) -> Result<Vec<SensitivityRecord>, SearchError> {
    let nominal = e.evaluate(base, tier)?;
    let mut out = Vec::new();
    for v in 0..m.variable_count as usize {
        let spec = m.variables[v];
        let center = base.values[v];
        let lower =
            (i64::from(center) - i64::from(spec.quantum)).max(i64::from(spec.minimum)) as i32;
        let upper =
            (i64::from(center) + i64::from(spec.quantum)).min(i64::from(spec.maximum)) as i32;
        let mut lo = *base;
        lo.values[v] = lower;
        lo.identity = 0;
        lo = lo.seal().map_err(|_| SearchError::Configuration)?;
        let mut hi = *base;
        hi.values[v] = upper;
        hi.identity = 0;
        hi = hi.seal().map_err(|_| SearchError::Configuration)?;
        let le = e.evaluate(&lo, tier)?;
        let he = e.evaluate(&hi, tier)?;
        for o in 0..m.objective_count as usize {
            let baseline = nominal.aggregate.objectives[o];
            let lv = le.aggregate.objectives[o];
            let hv = he.aggregate.objectives[o];
            let span = i64::from(upper) - i64::from(lower);
            let slope = if span == 0 {
                0
            } else {
                (((i64::from(hv) - i64::from(lv)) << 16) / span)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
            };
            out.push(SensitivityRecord {
                manifest_identity: m.identity,
                candidate_identity: base.identity,
                variable_id: spec.id,
                objective_index: o as u8,
                flags: u8::from(lower == center) | u8::from(upper == center) << 1,
                baseline,
                lower: lv,
                upper: hv,
                delta_lower: lv.saturating_sub(baseline),
                delta_upper: hv.saturating_sub(baseline),
                slope_q16: slope,
            })
        }
    }
    Ok(out)
}
pub fn encode_ksn9(r: SensitivityRecord) -> [u8; KSN9_LENGTH] {
    let mut b = [0; KSN9_LENGTH];
    b[..4].copy_from_slice(b"KSN9");
    w16(&mut b, 4, 9);
    w16(&mut b, 6, KSN9_LENGTH as u16);
    w32(&mut b, 8, r.manifest_identity);
    w32(&mut b, 12, r.candidate_identity);
    w16(&mut b, 16, r.variable_id);
    b[18] = r.objective_index;
    b[19] = r.flags;
    for (o, v) in [
        (20, r.baseline),
        (24, r.lower),
        (28, r.upper),
        (32, r.delta_lower),
        (36, r.delta_upper),
        (40, r.slope_q16),
    ] {
        wi32(&mut b, o, v)
    }
    let crc = crc32_ieee(&b[..60]);
    w32(&mut b, 60, crc);
    b
}
pub fn parse_ksn9(b: &[u8]) -> Option<SensitivityRecord> {
    if b.len() != KSN9_LENGTH
        || &b[..4] != b"KSN9"
        || r16(b, 4) != 9
        || r16(b, 6) as usize != KSN9_LENGTH
        || b[44..60].iter().any(|v| *v != 0)
        || r32(b, 60) != crc32_ieee(&b[..60])
    {
        return None;
    }
    Some(SensitivityRecord {
        manifest_identity: r32(b, 8),
        candidate_identity: r32(b, 12),
        variable_id: r16(b, 16),
        objective_index: b[18],
        flags: b[19],
        baseline: ri32(b, 20),
        lower: ri32(b, 24),
        upper: ri32(b, 28),
        delta_lower: ri32(b, 32),
        delta_upper: ri32(b, 36),
        slope_q16: ri32(b, 40),
    })
}
fn r16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn r32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn ri32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn w16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes())
}
fn w32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes())
}
fn wi32(b: &mut [u8], o: usize, v: i32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_is_strict() {
        let r = SensitivityRecord {
            manifest_identity: 1,
            candidate_identity: 2,
            variable_id: 3,
            objective_index: 4,
            flags: 0,
            baseline: 5,
            lower: 4,
            upper: 6,
            delta_lower: -1,
            delta_upper: 1,
            slope_q16: 65_536,
        };
        assert_eq!(parse_ksn9(&encode_ksn9(r)), Some(r));
        let mut b = encode_ksn9(r);
        b[50] = 1;
        let c = crc32_ieee(&b[..60]);
        w32(&mut b, 60, c);
        assert_eq!(parse_ksn9(&b), None)
    }
}
