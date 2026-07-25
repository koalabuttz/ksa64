//! Strict Phase 9 optimization contracts.
//!
//! The portable core owns only bounded, deterministic experiment records. Search
//! execution, allocation, JSON parsing, reports, and parallelism remain host-side.

use crate::scenario::crc32_ieee;

pub const PHASE9_CONTRACT_ID: u32 = 0x4b53_4139;
pub const PHASE9_ACCEPTED_SEED: u32 = 0x4b53_4139;
pub const KOM9_LENGTH: usize = 2_048;
pub const KDV9_LENGTH: usize = 256;
pub const KOE9_LENGTH: usize = 512;
pub const MAX_VARIABLES: usize = 32;
pub const MAX_OBJECTIVES: usize = 8;
pub const MAX_CONSTRAINTS: usize = 16;
pub const MAX_VALUES: usize = 32;
pub const MAX_METRICS: usize = 8;
pub const MAX_CONSTRAINT_RESULTS: usize = 16;
const HEADER: usize = 32;
const CRC_LEN: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SearchEngineId {
    GridV1 = 1,
    Nsga2V1 = 2,
    DifferentialEvolutionV1 = 3,
}
impl SearchEngineId {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::GridV1),
            2 => Ok(Self::Nsga2V1),
            3 => Ok(Self::DifferentialEvolutionV1),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SearchPresetId {
    Quick = 1,
    Routine = 2,
    AcceptedBalanced = 3,
    Custom = 255,
}
impl SearchPresetId {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::Quick),
            2 => Ok(Self::Routine),
            3 => Ok(Self::AcceptedBalanced),
            255 => Ok(Self::Custom),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VariableKind {
    Fixed = 1,
    Integer = 2,
    Ordinal = 3,
    Catalogue = 4,
    Boolean = 5,
}
impl VariableKind {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::Fixed),
            2 => Ok(Self::Integer),
            3 => Ok(Self::Ordinal),
            4 => Ok(Self::Catalogue),
            5 => Ok(Self::Boolean),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AggregateId {
    Nominal = 1,
    Mean = 2,
    Minimum = 3,
    Maximum = 4,
    Quantile95 = 5,
    FailureRate = 6,
}
impl AggregateId {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::Nominal),
            2 => Ok(Self::Mean),
            3 => Ok(Self::Minimum),
            4 => Ok(Self::Maximum),
            5 => Ok(Self::Quantile95),
            6 => Ok(Self::FailureRate),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Direction {
    Minimize = 1,
    Maximize = 2,
}
impl Direction {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::Minimize),
            2 => Ok(Self::Maximize),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConstraintOp {
    AtLeast = 1,
    AtMost = 2,
    Equal = 3,
}
impl ConstraintOp {
    fn parse(v: u8) -> Result<Self, Phase9Error> {
        match v {
            1 => Ok(Self::AtLeast),
            2 => Ok(Self::AtMost),
            3 => Ok(Self::Equal),
            _ => Err(Phase9Error::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase9Error {
    Length,
    Magic,
    Version,
    Contract,
    Kind,
    Reserved,
    Checksum,
    Count,
    Enum,
    Bounds,
    Quantization,
    Identity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariableSpec {
    pub id: u16,
    pub kind: VariableKind,
    pub flags: u8,
    pub minimum: i32,
    pub maximum: i32,
    pub quantum: u32,
    pub catalogue_id: u32,
}
impl VariableSpec {
    pub const EMPTY: Self = Self {
        id: 0,
        kind: VariableKind::Integer,
        flags: 0,
        minimum: 0,
        maximum: 0,
        quantum: 1,
        catalogue_id: 0,
    };
    pub fn validate(&self) -> Result<(), Phase9Error> {
        if self.id == 0 || self.minimum > self.maximum || self.quantum == 0 {
            return Err(Phase9Error::Bounds);
        };
        if matches!(self.kind, VariableKind::Boolean)
            && (self.minimum != 0 || self.maximum != 1 || self.quantum != 1)
        {
            return Err(Phase9Error::Bounds);
        }
        Ok(())
    }
    pub fn accepts(&self, v: i32) -> bool {
        if v < self.minimum || v > self.maximum {
            return false;
        }
        ((v as i64 - self.minimum as i64) % (self.quantum as i64)) == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveSpec {
    pub metric_id: u16,
    pub aggregate: AggregateId,
    pub direction: Direction,
}
impl ObjectiveSpec {
    pub const EMPTY: Self = Self {
        metric_id: 0,
        aggregate: AggregateId::Nominal,
        direction: Direction::Minimize,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintSpec {
    pub metric_id: u16,
    pub aggregate: AggregateId,
    pub op: ConstraintOp,
    pub threshold: i32,
    pub scale: u32,
}
impl ConstraintSpec {
    pub const EMPTY: Self = Self {
        metric_id: 0,
        aggregate: AggregateId::Nominal,
        op: ConstraintOp::AtMost,
        threshold: 0,
        scale: 1,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchBudgets {
    pub grid_points: u16,
    pub population: u16,
    pub generations: u16,
    pub finalists: u16,
    pub max_candidates: u32,
}
impl SearchBudgets {
    pub const fn for_preset(p: SearchPresetId) -> Self {
        match p {
            SearchPresetId::Quick => Self {
                grid_points: 5,
                population: 8,
                generations: 4,
                finalists: 4,
                max_candidates: 4_096,
            },
            SearchPresetId::Routine => Self {
                grid_points: 9,
                population: 24,
                generations: 12,
                finalists: 12,
                max_candidates: 65_536,
            },
            SearchPresetId::AcceptedBalanced => Self {
                grid_points: 17,
                population: 48,
                generations: 32,
                finalists: 32,
                max_candidates: 1_000_000,
            },
            SearchPresetId::Custom => Self {
                grid_points: 0,
                population: 0,
                generations: 0,
                finalists: 0,
                max_candidates: 0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchManifest {
    pub identity: u32,
    pub base_ids: [u32; 8],
    pub engine: SearchEngineId,
    pub preset: SearchPresetId,
    pub master_seed: u32,
    pub budgets: SearchBudgets,
    pub variable_count: u8,
    pub objective_count: u8,
    pub constraint_count: u8,
    pub variables: [VariableSpec; MAX_VARIABLES],
    pub objectives: [ObjectiveSpec; MAX_OBJECTIVES],
    pub constraints: [ConstraintSpec; MAX_CONSTRAINTS],
}
impl SearchManifest {
    pub fn validate(&self) -> Result<(), Phase9Error> {
        if self.variable_count as usize > MAX_VARIABLES
            || self.objective_count as usize > MAX_OBJECTIVES
            || self.constraint_count as usize > MAX_CONSTRAINTS
        {
            return Err(Phase9Error::Count);
        };
        if self.variable_count == 0 || self.objective_count == 0 {
            return Err(Phase9Error::Count);
        };
        for i in 0..self.variable_count as usize {
            self.variables[i].validate()?;
            for j in 0..i {
                if self.variables[j].id == self.variables[i].id {
                    return Err(Phase9Error::Identity);
                }
            }
        }
        for o in &self.objectives[..self.objective_count as usize] {
            if o.metric_id == 0 {
                return Err(Phase9Error::Bounds);
            }
        }
        for c in &self.constraints[..self.constraint_count as usize] {
            if c.metric_id == 0 || c.scale == 0 {
                return Err(Phase9Error::Bounds);
            }
        }
        Ok(())
    }
    pub fn canonical_identity(&self) -> u32 {
        let mut copy = *self;
        copy.identity = 0;
        let bytes = copy.encode_unchecked();
        crc32_ieee(&bytes[HEADER..KOM9_LENGTH - CRC_LEN])
    }
    pub fn seal(mut self) -> Result<Self, Phase9Error> {
        self.validate()?;
        self.identity = self.canonical_identity();
        Ok(self)
    }
    fn encode_unchecked(&self) -> [u8; KOM9_LENGTH] {
        let mut b = [0u8; KOM9_LENGTH];
        header(&mut b, *b"KOM9", 1, self.identity);
        let mut o = HEADER;
        for id in self.base_ids {
            w32(&mut b, o, id);
            o += 4
        }
        b[o] = self.engine as u8;
        b[o + 1] = self.preset as u8;
        b[o + 2] = self.variable_count;
        b[o + 3] = self.objective_count;
        b[o + 4] = self.constraint_count;
        o += 8;
        w32(&mut b, o, self.master_seed);
        o += 4;
        w16(&mut b, o, self.budgets.grid_points);
        w16(&mut b, o + 2, self.budgets.population);
        w16(&mut b, o + 4, self.budgets.generations);
        w16(&mut b, o + 6, self.budgets.finalists);
        w32(&mut b, o + 8, self.budgets.max_candidates);
        o = 96;
        for v in self.variables {
            w16(&mut b, o, v.id);
            b[o + 2] = v.kind as u8;
            b[o + 3] = v.flags;
            wi32(&mut b, o + 4, v.minimum);
            wi32(&mut b, o + 8, v.maximum);
            w32(&mut b, o + 12, v.quantum);
            w32(&mut b, o + 16, v.catalogue_id);
            o += 20
        }
        o = 736;
        for x in self.objectives {
            w16(&mut b, o, x.metric_id);
            b[o + 2] = x.aggregate as u8;
            b[o + 3] = x.direction as u8;
            o += 8
        }
        o = 800;
        for x in self.constraints {
            w16(&mut b, o, x.metric_id);
            b[o + 2] = x.aggregate as u8;
            b[o + 3] = x.op as u8;
            wi32(&mut b, o + 4, x.threshold);
            w32(&mut b, o + 8, x.scale);
            o += 16
        }
        seal(&mut b);
        b
    }
    pub fn encode(&self) -> Result<[u8; KOM9_LENGTH], Phase9Error> {
        self.validate()?;
        if self.identity != self.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        Ok(self.encode_unchecked())
    }
    pub fn parse(b: &[u8]) -> Result<Self, Phase9Error> {
        check(b, *b"KOM9", 1, KOM9_LENGTH)?;
        let mut base = [0; 8];
        let mut o = HEADER;
        for id in &mut base {
            *id = r32(b, o);
            o += 4
        }
        let engine = SearchEngineId::parse(b[o])?;
        let preset = SearchPresetId::parse(b[o + 1])?;
        let vc = b[o + 2];
        let oc = b[o + 3];
        let cc = b[o + 4];
        if b[o + 5..o + 8].iter().any(|x| *x != 0) {
            return Err(Phase9Error::Reserved);
        }
        o += 8;
        let seed = r32(b, o);
        o += 4;
        let budgets = SearchBudgets {
            grid_points: r16(b, o),
            population: r16(b, o + 2),
            generations: r16(b, o + 4),
            finalists: r16(b, o + 6),
            max_candidates: r32(b, o + 8),
        };
        if b[88..96].iter().any(|x| *x != 0) {
            return Err(Phase9Error::Reserved);
        }
        let mut variables = [VariableSpec::EMPTY; MAX_VARIABLES];
        o = 96;
        for v in &mut variables {
            *v = VariableSpec {
                id: r16(b, o),
                kind: VariableKind::parse(b[o + 2])?,
                flags: b[o + 3],
                minimum: ri32(b, o + 4),
                maximum: ri32(b, o + 8),
                quantum: r32(b, o + 12),
                catalogue_id: r32(b, o + 16),
            };
            o += 20
        }
        let mut objectives = [ObjectiveSpec::EMPTY; MAX_OBJECTIVES];
        o = 736;
        for x in &mut objectives {
            *x = ObjectiveSpec {
                metric_id: r16(b, o),
                aggregate: AggregateId::parse(b[o + 2])?,
                direction: Direction::parse(b[o + 3])?,
            };
            if b[o + 4..o + 8].iter().any(|v| *v != 0) {
                return Err(Phase9Error::Reserved);
            }
            o += 8
        }
        let mut constraints = [ConstraintSpec::EMPTY; MAX_CONSTRAINTS];
        o = 800;
        for x in &mut constraints {
            *x = ConstraintSpec {
                metric_id: r16(b, o),
                aggregate: AggregateId::parse(b[o + 2])?,
                op: ConstraintOp::parse(b[o + 3])?,
                threshold: ri32(b, o + 4),
                scale: r32(b, o + 8),
            };
            if b[o + 12..o + 16].iter().any(|v| *v != 0) {
                return Err(Phase9Error::Reserved);
            }
            o += 16
        }
        if b[1056..KOM9_LENGTH - CRC_LEN].iter().any(|x| *x != 0) {
            return Err(Phase9Error::Reserved);
        }
        let x = Self {
            identity: r32(b, 16),
            base_ids: base,
            engine,
            preset,
            master_seed: seed,
            budgets,
            variable_count: vc,
            objective_count: oc,
            constraint_count: cc,
            variables,
            objectives,
            constraints,
        };
        x.validate()?;
        for v in &x.variables[vc as usize..] {
            if *v != VariableSpec::EMPTY {
                return Err(Phase9Error::Reserved);
            }
        }
        for v in &x.objectives[oc as usize..] {
            if *v != ObjectiveSpec::EMPTY {
                return Err(Phase9Error::Reserved);
            }
        }
        for v in &x.constraints[cc as usize..] {
            if *v != ConstraintSpec::EMPTY {
                return Err(Phase9Error::Reserved);
            }
        }
        if x.identity != x.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        Ok(x)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesignVector {
    pub identity: u32,
    pub manifest_identity: u32,
    pub value_count: u8,
    pub values: [i32; MAX_VALUES],
    pub materialized_ids: [u32; 4],
}
impl DesignVector {
    pub fn canonical_identity(&self) -> u32 {
        let mut b = [0u8; 4 + 1 + 3 + MAX_VALUES * 4 + 16];
        w32(&mut b, 0, self.manifest_identity);
        b[4] = self.value_count;
        let mut o = 8;
        for v in self.values {
            wi32(&mut b, o, v);
            o += 4
        }
        for id in self.materialized_ids {
            w32(&mut b, o, id);
            o += 4
        }
        crc32_ieee(&b)
    }
    pub fn seal(mut self) -> Result<Self, Phase9Error> {
        if self.value_count as usize > MAX_VALUES {
            return Err(Phase9Error::Count);
        }
        self.identity = self.canonical_identity();
        Ok(self)
    }
    pub fn validate_against(&self, m: &SearchManifest) -> Result<(), Phase9Error> {
        if self.manifest_identity != m.identity || self.value_count != m.variable_count {
            return Err(Phase9Error::Identity);
        }
        for i in 0..self.value_count as usize {
            if !m.variables[i].accepts(self.values[i]) {
                return Err(Phase9Error::Quantization);
            }
        }
        if self.identity != self.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<[u8; KDV9_LENGTH], Phase9Error> {
        if self.value_count as usize > MAX_VALUES || self.identity != self.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        let mut b = [0; KDV9_LENGTH];
        header(&mut b, *b"KDV9", 2, self.identity);
        w32(&mut b, 32, self.manifest_identity);
        b[36] = self.value_count;
        let mut o = 40;
        for v in self.values {
            wi32(&mut b, o, v);
            o += 4
        }
        for id in self.materialized_ids {
            w32(&mut b, o, id);
            o += 4
        }
        seal(&mut b);
        Ok(b)
    }
    pub fn parse(b: &[u8]) -> Result<Self, Phase9Error> {
        check(b, *b"KDV9", 2, KDV9_LENGTH)?;
        if b[37..40].iter().any(|x| *x != 0) || b[184..KDV9_LENGTH - 4].iter().any(|x| *x != 0) {
            return Err(Phase9Error::Reserved);
        }
        let count = b[36];
        if count as usize > MAX_VALUES {
            return Err(Phase9Error::Count);
        }
        let mut values = [0; MAX_VALUES];
        let mut o = 40;
        for v in &mut values {
            *v = ri32(b, o);
            o += 4
        }
        if values[count as usize..].iter().any(|x| *x != 0) {
            return Err(Phase9Error::Reserved);
        }
        let mut ids = [0; 4];
        for id in &mut ids {
            *id = r32(b, o);
            o += 4
        }
        let x = Self {
            identity: r32(b, 16),
            manifest_identity: r32(b, 32),
            value_count: count,
            values,
            materialized_ids: ids,
        };
        if x.identity != x.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        Ok(x)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateAggregate {
    pub identity: u32,
    pub manifest_identity: u32,
    pub candidate_identity: u32,
    pub uncertainty_tier: u8,
    pub case_count: u8,
    pub fatal_class: u8,
    pub violated_constraints: u8,
    pub feasible: bool,
    pub case_crc: u32,
    pub normalized_violation: i64,
    pub objective_count: u8,
    pub constraint_count: u8,
    pub objectives: [i32; MAX_METRICS],
    pub constraint_values: [i32; MAX_CONSTRAINT_RESULTS],
}
impl CandidateAggregate {
    pub fn canonical_identity(&self) -> u32 {
        let mut c = *self;
        c.identity = 0;
        let b = c.encode_unchecked();
        crc32_ieee(&b[HEADER..KOE9_LENGTH - CRC_LEN])
    }
    pub fn seal(mut self) -> Self {
        self.identity = self.canonical_identity();
        self
    }
    fn encode_unchecked(&self) -> [u8; KOE9_LENGTH] {
        let mut b = [0; KOE9_LENGTH];
        header(&mut b, *b"KOE9", 3, self.identity);
        w32(&mut b, 32, self.manifest_identity);
        w32(&mut b, 36, self.candidate_identity);
        b[40] = self.uncertainty_tier;
        b[41] = self.case_count;
        b[42] = self.fatal_class;
        b[43] = self.violated_constraints;
        b[44] = u8::from(self.feasible);
        b[45] = self.objective_count;
        b[46] = self.constraint_count;
        w32(&mut b, 48, self.case_crc);
        wi64(&mut b, 52, self.normalized_violation);
        let mut o = 64;
        for v in self.objectives {
            wi32(&mut b, o, v);
            o += 4
        }
        o = 96;
        for v in self.constraint_values {
            wi32(&mut b, o, v);
            o += 4
        }
        seal(&mut b);
        b
    }
    pub fn encode(&self) -> Result<[u8; KOE9_LENGTH], Phase9Error> {
        if self.objective_count as usize > MAX_METRICS
            || self.constraint_count as usize > MAX_CONSTRAINT_RESULTS
            || self.identity != self.canonical_identity()
        {
            return Err(Phase9Error::Identity);
        }
        Ok(self.encode_unchecked())
    }
    pub fn parse(b: &[u8]) -> Result<Self, Phase9Error> {
        check(b, *b"KOE9", 3, KOE9_LENGTH)?;
        if !matches!(b[44], 0 | 1)
            || b[47] != 0
            || b[60..64].iter().any(|x| *x != 0)
            || b[160..KOE9_LENGTH - 4].iter().any(|x| *x != 0)
        {
            return Err(Phase9Error::Reserved);
        }
        let oc = b[45];
        let cc = b[46];
        if oc as usize > MAX_METRICS || cc as usize > MAX_CONSTRAINT_RESULTS {
            return Err(Phase9Error::Count);
        }
        let mut objectives = [0; MAX_METRICS];
        let mut o = 64;
        for v in &mut objectives {
            *v = ri32(b, o);
            o += 4
        }
        let mut constraints = [0; MAX_CONSTRAINT_RESULTS];
        o = 96;
        for v in &mut constraints {
            *v = ri32(b, o);
            o += 4
        }
        if objectives[oc as usize..].iter().any(|x| *x != 0)
            || constraints[cc as usize..].iter().any(|x| *x != 0)
        {
            return Err(Phase9Error::Reserved);
        }
        let x = Self {
            identity: r32(b, 16),
            manifest_identity: r32(b, 32),
            candidate_identity: r32(b, 36),
            uncertainty_tier: b[40],
            case_count: b[41],
            fatal_class: b[42],
            violated_constraints: b[43],
            feasible: b[44] != 0,
            objective_count: oc,
            constraint_count: cc,
            case_crc: r32(b, 48),
            normalized_violation: ri64(b, 52),
            objectives,
            constraint_values: constraints,
        };
        if x.identity != x.canonical_identity() {
            return Err(Phase9Error::Identity);
        }
        Ok(x)
    }
}

fn header(out: &mut [u8], magic: [u8; 4], kind: u16, identity: u32) {
    out.fill(0);
    out[0..4].copy_from_slice(&magic);
    w16(out, 4, 9);
    w16(out, 6, HEADER as u16);
    w16(out, 8, out.len() as u16);
    w16(out, 10, kind);
    w32(out, 12, PHASE9_CONTRACT_ID);
    w32(out, 16, identity)
}
fn check(b: &[u8], magic: [u8; 4], kind: u16, len: usize) -> Result<(), Phase9Error> {
    if b.len() != len {
        return Err(Phase9Error::Length);
    }
    if b[0..4] != magic {
        return Err(Phase9Error::Magic);
    }
    if r16(b, 4) != 9 || r16(b, 6) != HEADER as u16 || r16(b, 8) as usize != len {
        return Err(Phase9Error::Version);
    }
    if r16(b, 10) != kind {
        return Err(Phase9Error::Kind);
    }
    if r32(b, 12) != PHASE9_CONTRACT_ID {
        return Err(Phase9Error::Contract);
    }
    if b[20..HEADER].iter().any(|x| *x != 0) {
        return Err(Phase9Error::Reserved);
    }
    if r32(b, len - 4) != crc32_ieee(&b[..len - 4]) {
        return Err(Phase9Error::Checksum);
    }
    Ok(())
}
fn seal(b: &mut [u8]) {
    let o = b.len() - 4;
    let c = crc32_ieee(&b[..o]);
    w32(b, o, c)
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
fn ri64(b: &[u8], o: usize) -> i64 {
    i64::from_le_bytes(b[o..o + 8].try_into().unwrap())
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
fn wi64(b: &mut [u8], o: usize, v: i64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> SearchManifest {
        let mut m = SearchManifest {
            identity: 0,
            base_ids: [1, 2, 3, 4, 5, 6, 7, 8],
            engine: SearchEngineId::Nsga2V1,
            preset: SearchPresetId::Quick,
            master_seed: PHASE9_ACCEPTED_SEED,
            budgets: SearchBudgets::for_preset(SearchPresetId::Quick),
            variable_count: 1,
            objective_count: 1,
            constraint_count: 1,
            variables: [VariableSpec::EMPTY; MAX_VARIABLES],
            objectives: [ObjectiveSpec::EMPTY; MAX_OBJECTIVES],
            constraints: [ConstraintSpec::EMPTY; MAX_CONSTRAINTS],
        };
        m.variables[0] = VariableSpec {
            id: 1,
            kind: VariableKind::Fixed,
            flags: 0,
            minimum: -10,
            maximum: 10,
            quantum: 2,
            catalogue_id: 0,
        };
        m.objectives[0] = ObjectiveSpec {
            metric_id: 1,
            aggregate: AggregateId::Mean,
            direction: Direction::Maximize,
        };
        m.constraints[0] = ConstraintSpec {
            metric_id: 2,
            aggregate: AggregateId::Maximum,
            op: ConstraintOp::AtMost,
            threshold: 8,
            scale: 1,
        };
        m.seal().unwrap()
    }
    #[test]
    fn strict_records_round_trip() {
        let m = manifest();
        assert_eq!(SearchManifest::parse(&m.encode().unwrap()).unwrap(), m);
        let mut d = DesignVector {
            identity: 0,
            manifest_identity: m.identity,
            value_count: 1,
            values: [0; MAX_VALUES],
            materialized_ids: [10, 11, 12, 13],
        };
        d.values[0] = 2;
        let d = d.seal().unwrap();
        d.validate_against(&m).unwrap();
        assert_eq!(DesignVector::parse(&d.encode().unwrap()).unwrap(), d);
        let a = CandidateAggregate {
            identity: 0,
            manifest_identity: m.identity,
            candidate_identity: d.identity,
            uncertainty_tier: 8,
            case_count: 8,
            fatal_class: 0,
            violated_constraints: 0,
            feasible: true,
            case_crc: 7,
            normalized_violation: 0,
            objective_count: 1,
            constraint_count: 1,
            objectives: [0; MAX_METRICS],
            constraint_values: [0; MAX_CONSTRAINT_RESULTS],
        }
        .seal();
        assert_eq!(CandidateAggregate::parse(&a.encode().unwrap()).unwrap(), a)
    }
    #[test]
    fn corruption_and_quantization_fail_closed() {
        let m = manifest();
        let mut b = m.encode().unwrap();
        b[100] ^= 1;
        assert_eq!(SearchManifest::parse(&b), Err(Phase9Error::Checksum));
        let mut d = DesignVector {
            identity: 0,
            manifest_identity: m.identity,
            value_count: 1,
            values: [0; MAX_VALUES],
            materialized_ids: [0; 4],
        };
        d.values[0] = 3;
        let d = d.seal().unwrap();
        assert_eq!(d.validate_against(&m), Err(Phase9Error::Quantization))
    }
}
