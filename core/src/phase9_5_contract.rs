//! Strict additive Phase 9.5 effector, allocator, and evaluation contracts.

// Explicit indices mirror frozen wire offsets and keep MOS code generation auditable.
#![allow(clippy::needless_range_loop)]

use crate::evaluation::{EvaluationSummary, ModelProfileId};
use crate::phase8_result::{encode_ksr8, parse_ksr8, spatial_evaluation_identity};
use crate::phase9_5_numeric::RCS_PULSE_QUANTUM_Q18;
use crate::scenario::crc32_ieee;

pub const PHASE95_CONTRACT_ID: u32 = 0x0950_0001;
pub const PHASE95_ACCEPTED_SEED: u32 = 0x4b53_4195;
pub const KPE9_LENGTH: usize = 2_048;
pub const KPA9_LENGTH: usize = 512;
pub const KLE9_LENGTH: usize = 256;
pub const KAS9_LENGTH: usize = 512;
pub const KSC9_LENGTH: usize = 512;
pub const KAT9_HEADER_LENGTH: usize = 128;
pub const KAT9_FRAME_LENGTH: usize = 320;
pub const MAX_CANARDS: usize = 4;
pub const MAX_RCS_JETS: usize = 12;
pub const MAX_CANARD_COEFFICIENT_KNOTS: usize = 8;
pub const MAX_SUPPLY_KNOTS: usize = 16;
const HEADER: usize = 32;
const VERSION: u16 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AdvancedEffectorSetId {
    CanardOnly = 1,
    RcsOnly = 2,
    GimbalCanardRcs = 3,
}
impl AdvancedEffectorSetId {
    fn parse(v: u8) -> Result<Self, Phase95ContractError> {
        match v {
            1 => Ok(Self::CanardOnly),
            2 => Ok(Self::RcsOnly),
            3 => Ok(Self::GimbalCanardRcs),
            _ => Err(Phase95ContractError::Enum),
        }
    }
    pub const fn has_canards(self) -> bool {
        matches!(self, Self::CanardOnly | Self::GimbalCanardRcs)
    }
    pub const fn has_rcs(self) -> bool {
        matches!(self, Self::RcsOnly | Self::GimbalCanardRcs)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RcsSupplySourceId {
    None = 0,
    RegulatedV1 = 1,
    IdealIsothermalBlowdownV1 = 2,
}
impl RcsSupplySourceId {
    fn parse(v: u8) -> Result<Self, Phase95ContractError> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::RegulatedV1),
            2 => Ok(Self::IdealIsothermalBlowdownV1),
            _ => Err(Phase95ContractError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlAllocatorId {
    PriorityResidualV1 = 1,
}
impl ControlAllocatorId {
    fn parse(v: u8) -> Result<Self, Phase95ContractError> {
        match v {
            1 => Ok(Self::PriorityResidualV1),
            _ => Err(Phase95ContractError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase95ContractError {
    Length,
    Magic,
    Version,
    Kind,
    Contract,
    Identity,
    Reserved,
    Checksum,
    Enum,
    Range,
    Unsupported,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
#[allow(dead_code)]
enum Kind {
    Effector = 1,
    Allocator = 2,
    Evaluation = 3,
    Summary = 4,
    Campaign = 5,
}
impl Kind {
    const fn magic(self) -> [u8; 4] {
        match self {
            Self::Effector => *b"KPE9",
            Self::Allocator => *b"KPA9",
            Self::Evaluation => *b"KLE9",
            Self::Summary => *b"KAS9",
            Self::Campaign => *b"KSC9",
        }
    }
    const fn length(self) -> usize {
        match self {
            Self::Effector => KPE9_LENGTH,
            Self::Allocator => KPA9_LENGTH,
            Self::Evaluation => KLE9_LENGTH,
            Self::Summary => KAS9_LENGTH,
            Self::Campaign => KSC9_LENGTH,
        }
    }
}
fn p16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
}
fn pi16(o: &mut [u8], i: usize, v: i16) {
    p16(o, i, v as u16)
}
fn p32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes())
}
fn pi32(o: &mut [u8], i: usize, v: i32) {
    p32(o, i, v as u32)
}
fn g16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn gi16(b: &[u8], i: usize) -> i16 {
    g16(b, i) as i16
}
fn g32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn gi32(b: &[u8], i: usize) -> i32 {
    g32(b, i) as i32
}
fn header(o: &mut [u8], kind: Kind, id: u32) -> Result<(), Phase95ContractError> {
    if o.len() != kind.length() {
        return Err(Phase95ContractError::Length);
    }
    if id == 0 {
        return Err(Phase95ContractError::Identity);
    }
    o.fill(0);
    o[..4].copy_from_slice(&kind.magic());
    p16(o, 4, VERSION);
    p16(o, 6, HEADER as u16);
    p16(o, 8, o.len() as u16);
    p16(o, 10, kind as u16);
    p32(o, 12, PHASE95_CONTRACT_ID);
    p32(o, 16, id);
    Ok(())
}
fn seal(o: &mut [u8]) {
    let at = o.len() - 4;
    let crc = crc32_ieee(&o[..at]);
    p32(o, at, crc)
}
fn validate(b: &[u8], kind: Kind) -> Result<u32, Phase95ContractError> {
    if b.len() != kind.length() {
        return Err(Phase95ContractError::Length);
    }
    if b[..4] != kind.magic() {
        return Err(Phase95ContractError::Magic);
    }
    if g16(b, 4) != VERSION || g16(b, 6) as usize != HEADER {
        return Err(Phase95ContractError::Version);
    }
    if g16(b, 8) as usize != b.len() || g16(b, 10) != kind as u16 {
        return Err(Phase95ContractError::Kind);
    }
    if g32(b, 12) != PHASE95_CONTRACT_ID {
        return Err(Phase95ContractError::Contract);
    }
    if b[20..HEADER].iter().any(|x| *x != 0) {
        return Err(Phase95ContractError::Reserved);
    }
    let id = g32(b, 16);
    if id == 0 {
        return Err(Phase95ContractError::Identity);
    }
    let at = b.len() - 4;
    if g32(b, at) != crc32_ieee(&b[..at]) {
        return Err(Phase95ContractError::Checksum);
    }
    Ok(id)
}
fn zero(b: &[u8], start: usize, end: usize) -> Result<(), Phase95ContractError> {
    if b[start..end].iter().any(|x| *x != 0) {
        Err(Phase95ContractError::Reserved)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardInstallation {
    pub position_q28: [i32; 3],
    pub normal_q15: [i16; 3],
    pub hinge_axis_q15: [i16; 3],
    pub root_q28: i32,
    pub tip_q28: i32,
    pub span_q28: i32,
    pub sweep_q28: i32,
    pub mass_q21: i32,
    pub inertia_q19: [i32; 3],
    pub limit_turn16: i16,
    pub slew_turn16_per_release: i16,
    pub lag_releases: u8,
    pub flags: u8,
    pub failure_identity: u16,
}
impl CanardInstallation {
    pub const EMPTY: Self = Self {
        position_q28: [0; 3],
        normal_q15: [0; 3],
        hinge_axis_q15: [0; 3],
        root_q28: 0,
        tip_q28: 0,
        span_q28: 0,
        sweep_q28: 0,
        mass_q21: 0,
        inertia_q19: [0; 3],
        limit_turn16: 0,
        slew_turn16_per_release: 0,
        lag_releases: 0,
        flags: 0,
        failure_identity: 0,
    };
    pub fn is_valid(&self) -> bool {
        self.root_q28 > 0
            && self.tip_q28 > 0
            && self.span_q28 > 0
            && self.sweep_q28 >= 0
            && self.mass_q21 > 0
            && self.inertia_q19.iter().all(|x| *x > 0)
            && self.limit_turn16 > 0
            && self.slew_turn16_per_release > 0
            && self.lag_releases <= 8
            && self.failure_identity != 0
            && self.normal_q15.iter().any(|x| *x != 0)
            && self.hinge_axis_q15.iter().any(|x| *x != 0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardCoefficientKnot {
    pub mach_q24: i32,
    pub control_q24: i32,
    pub drag_q24: i32,
    pub hinge_q24: i32,
}
impl CanardCoefficientKnot {
    pub const ZERO: Self = Self {
        mach_q24: 0,
        control_q24: 0,
        drag_q24: 0,
        hinge_q24: 0,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcsJetInstallation {
    pub position_q28: [i32; 3],
    pub direction_q30: [i32; 3],
    pub nominal_thrust_q23: i32,
    pub specific_impulse_q16: i32,
    pub min_pulse_quanta: u8,
    pub max_pulse_quanta: u8,
    pub valve_delay_quanta: u8,
    pub flags: u8,
    pub failure_identity: u16,
    pub provenance_identity: u32,
}
impl RcsJetInstallation {
    pub const EMPTY: Self = Self {
        position_q28: [0; 3],
        direction_q30: [0; 3],
        nominal_thrust_q23: 0,
        specific_impulse_q16: 0,
        min_pulse_quanta: 0,
        max_pulse_quanta: 0,
        valve_delay_quanta: 0,
        flags: 0,
        failure_identity: 0,
        provenance_identity: 0,
    };
    pub fn is_valid(&self) -> bool {
        self.direction_q30.iter().any(|x| *x != 0)
            && self.nominal_thrust_q23 > 0
            && self.specific_impulse_q16 > 0
            && self.min_pulse_quanta <= self.max_pulse_quanta
            && self.max_pulse_quanta <= 8
            && self.valve_delay_quanta <= 8
            && self.failure_identity != 0
            && self.provenance_identity != 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupplyKnot {
    pub remaining_propellant_q21: i32,
    pub pressure_q8: i32,
    pub thrust_scale_q30: i32,
    pub mass_flow_scale_q30: i32,
}
impl SupplyKnot {
    pub const ZERO: Self = Self {
        remaining_propellant_q21: 0,
        pressure_q8: 0,
        thrust_scale_q30: 0,
        mass_flow_scale_q30: 0,
    };
    pub fn is_valid(&self) -> bool {
        self.remaining_propellant_q21 >= 0
            && self.pressure_q8 > 0
            && self.thrust_scale_q30 > 0
            && self.mass_flow_scale_q30 > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedEffectorPack {
    pub identity: u32,
    pub set: AdvancedEffectorSetId,
    pub supply_source: RcsSupplySourceId,
    pub flags: u16,
    pub vehicle_identity: u32,
    pub neutral_vehicle_identity: u32,
    pub supply_identity: u32,
    pub provenance_identity: u32,
    pub tank_position_q28: [i32; 3],
    pub tank_dry_mass_q21: i32,
    pub propellant_wet_mass_q21: i32,
    pub reserve_q15: u16,
    pub canard_hinge_limits_q24: [i32; 4],
    pub canard_count: u8,
    pub jet_count: u8,
    pub coefficient_count: u8,
    pub supply_count: u8,
    pub canards: [CanardInstallation; MAX_CANARDS],
    pub coefficient_knots: [CanardCoefficientKnot; MAX_CANARD_COEFFICIENT_KNOTS],
    pub jets: [RcsJetInstallation; MAX_RCS_JETS],
    pub supply_knots: [SupplyKnot; MAX_SUPPLY_KNOTS],
}
impl AdvancedEffectorPack {
    pub fn is_valid(&self) -> bool {
        if self.identity == 0
            || self.vehicle_identity == 0
            || self.neutral_vehicle_identity == 0
            || self.provenance_identity == 0
            || self.canard_count as usize > MAX_CANARDS
            || self.jet_count as usize > MAX_RCS_JETS
            || self.coefficient_count as usize > MAX_CANARD_COEFFICIENT_KNOTS
            || self.supply_count as usize > MAX_SUPPLY_KNOTS
        {
            return false;
        }
        if self.set.has_canards() != (self.canard_count == 4)
            || self.set.has_rcs() != (self.jet_count == 12)
        {
            return false;
        }
        if self.set.has_canards() {
            if self.coefficient_count < 2
                || self.canard_hinge_limits_q24.iter().any(|value| *value <= 0)
                || !self.canards[..self.canard_count as usize]
                    .iter()
                    .all(CanardInstallation::is_valid)
            {
                return false;
            }
            let mut last = -1;
            for k in &self.coefficient_knots[..self.coefficient_count as usize] {
                if k.mach_q24 <= last || k.control_q24 <= 0 || k.drag_q24 < 0 || k.hinge_q24 <= 0 {
                    return false;
                }
                last = k.mach_q24
            }
        }
        if self.set.has_rcs() {
            if self.supply_source == RcsSupplySourceId::None
                || self.supply_identity == 0
                || self.supply_count < 2
                || self.tank_dry_mass_q21 <= 0
                || self.propellant_wet_mass_q21 <= 0
                || self.reserve_q15 > 32768
                || !self.jets[..self.jet_count as usize]
                    .iter()
                    .all(RcsJetInstallation::is_valid)
            {
                return false;
            }
            let mut last = -1;
            for k in &self.supply_knots[..self.supply_count as usize] {
                if !k.is_valid() || k.remaining_propellant_q21 <= last {
                    return false;
                }
                last = k.remaining_propellant_q21
            }
        } else if self.supply_source != RcsSupplySourceId::None
            || self.supply_identity != 0
            || self.supply_count != 0
            || self.propellant_wet_mass_q21 != 0
        {
            return false;
        }
        RCS_PULSE_QUANTUM_Q18 == 1024
    }
}

fn put_canard(o: &mut [u8], at: usize, v: &CanardInstallation) {
    for i in 0..3 {
        pi32(o, at + i * 4, v.position_q28[i]);
        pi16(o, at + 12 + i * 2, v.normal_q15[i]);
        pi16(o, at + 18 + i * 2, v.hinge_axis_q15[i]);
        pi32(o, at + 44 + i * 4, v.inertia_q19[i]);
    }
    pi32(o, at + 24, v.root_q28);
    pi32(o, at + 28, v.tip_q28);
    pi32(o, at + 32, v.span_q28);
    pi32(o, at + 36, v.sweep_q28);
    pi32(o, at + 40, v.mass_q21);
    pi16(o, at + 56, v.limit_turn16);
    pi16(o, at + 58, v.slew_turn16_per_release);
    o[at + 60] = v.lag_releases;
    o[at + 61] = v.flags;
    p16(o, at + 62, v.failure_identity)
}
fn get_canard(b: &[u8], at: usize) -> CanardInstallation {
    let mut p = [0; 3];
    let mut n = [0; 3];
    let mut h = [0; 3];
    let mut inertia = [0; 3];
    for i in 0..3 {
        p[i] = gi32(b, at + i * 4);
        n[i] = gi16(b, at + 12 + i * 2);
        h[i] = gi16(b, at + 18 + i * 2);
        inertia[i] = gi32(b, at + 44 + i * 4);
    }
    CanardInstallation {
        position_q28: p,
        normal_q15: n,
        hinge_axis_q15: h,
        root_q28: gi32(b, at + 24),
        tip_q28: gi32(b, at + 28),
        span_q28: gi32(b, at + 32),
        sweep_q28: gi32(b, at + 36),
        mass_q21: gi32(b, at + 40),
        inertia_q19: inertia,
        limit_turn16: gi16(b, at + 56),
        slew_turn16_per_release: gi16(b, at + 58),
        lag_releases: b[at + 60],
        flags: b[at + 61],
        failure_identity: g16(b, at + 62),
    }
}
fn put_jet(o: &mut [u8], at: usize, v: &RcsJetInstallation) {
    for i in 0..3 {
        pi32(o, at + i * 4, v.position_q28[i]);
        pi32(o, at + 12 + i * 4, v.direction_q30[i]);
    }
    pi32(o, at + 24, v.nominal_thrust_q23);
    pi32(o, at + 28, v.specific_impulse_q16);
    o[at + 32] = v.min_pulse_quanta;
    o[at + 33] = v.max_pulse_quanta;
    o[at + 34] = v.valve_delay_quanta;
    o[at + 35] = v.flags;
    p16(o, at + 36, v.failure_identity);
    p32(o, at + 40, v.provenance_identity)
}
fn get_jet(b: &[u8], at: usize) -> Result<RcsJetInstallation, Phase95ContractError> {
    zero(b, at + 38, at + 40)?;
    zero(b, at + 44, at + 48)?;
    let mut p = [0; 3];
    let mut d = [0; 3];
    for i in 0..3 {
        p[i] = gi32(b, at + i * 4);
        d[i] = gi32(b, at + 12 + i * 4);
    }
    Ok(RcsJetInstallation {
        position_q28: p,
        direction_q30: d,
        nominal_thrust_q23: gi32(b, at + 24),
        specific_impulse_q16: gi32(b, at + 28),
        min_pulse_quanta: b[at + 32],
        max_pulse_quanta: b[at + 33],
        valve_delay_quanta: b[at + 34],
        flags: b[at + 35],
        failure_identity: g16(b, at + 36),
        provenance_identity: g32(b, at + 40),
    })
}

pub fn write_effector_pack(
    v: &AdvancedEffectorPack,
    o: &mut [u8],
) -> Result<(), Phase95ContractError> {
    if !v.is_valid() {
        return Err(Phase95ContractError::Range);
    }
    header(o, Kind::Effector, v.identity)?;
    o[32] = v.set as u8;
    o[33] = v.supply_source as u8;
    o[34] = v.canard_count;
    o[35] = v.jet_count;
    o[36] = v.coefficient_count;
    o[37] = v.supply_count;
    p16(o, 38, v.flags);
    p32(o, 40, v.vehicle_identity);
    p32(o, 44, v.neutral_vehicle_identity);
    p32(o, 48, v.supply_identity);
    p32(o, 52, v.provenance_identity);
    for i in 0..3 {
        pi32(o, 56 + i * 4, v.tank_position_q28[i])
    }
    pi32(o, 68, v.tank_dry_mass_q21);
    pi32(o, 72, v.propellant_wet_mass_q21);
    p16(o, 76, v.reserve_q15);
    for i in 0..MAX_CANARDS {
        pi32(o, 80 + i * 4, v.canard_hinge_limits_q24[i])
    }
    for i in 0..MAX_CANARDS {
        put_canard(o, 96 + i * 64, &v.canards[i])
    }
    for i in 0..MAX_CANARD_COEFFICIENT_KNOTS {
        let at = 352 + i * 16;
        let k = v.coefficient_knots[i];
        pi32(o, at, k.mach_q24);
        pi32(o, at + 4, k.control_q24);
        pi32(o, at + 8, k.drag_q24);
        pi32(o, at + 12, k.hinge_q24)
    }
    for i in 0..MAX_RCS_JETS {
        put_jet(o, 480 + i * 48, &v.jets[i])
    }
    for i in 0..MAX_SUPPLY_KNOTS {
        let at = 1056 + i * 16;
        let k = v.supply_knots[i];
        pi32(o, at, k.remaining_propellant_q21);
        pi32(o, at + 4, k.pressure_q8);
        pi32(o, at + 8, k.thrust_scale_q30);
        pi32(o, at + 12, k.mass_flow_scale_q30)
    }
    seal(o);
    Ok(())
}
pub fn parse_effector_pack(b: &[u8]) -> Result<AdvancedEffectorPack, Phase95ContractError> {
    let identity = validate(b, Kind::Effector)?;
    zero(b, 78, 80)?;
    zero(b, 1312, KPE9_LENGTH - 4)?;
    let mut canards = [CanardInstallation::EMPTY; MAX_CANARDS];
    for i in 0..MAX_CANARDS {
        canards[i] = get_canard(b, 96 + i * 64)
    }
    let mut coefficient_knots = [CanardCoefficientKnot::ZERO; MAX_CANARD_COEFFICIENT_KNOTS];
    for i in 0..MAX_CANARD_COEFFICIENT_KNOTS {
        let at = 352 + i * 16;
        coefficient_knots[i] = CanardCoefficientKnot {
            mach_q24: gi32(b, at),
            control_q24: gi32(b, at + 4),
            drag_q24: gi32(b, at + 8),
            hinge_q24: gi32(b, at + 12),
        }
    }
    let mut jets = [RcsJetInstallation::EMPTY; MAX_RCS_JETS];
    for i in 0..MAX_RCS_JETS {
        jets[i] = get_jet(b, 480 + i * 48)?
    }
    let mut supply_knots = [SupplyKnot::ZERO; MAX_SUPPLY_KNOTS];
    for i in 0..MAX_SUPPLY_KNOTS {
        let at = 1056 + i * 16;
        supply_knots[i] = SupplyKnot {
            remaining_propellant_q21: gi32(b, at),
            pressure_q8: gi32(b, at + 4),
            thrust_scale_q30: gi32(b, at + 8),
            mass_flow_scale_q30: gi32(b, at + 12),
        }
    }
    let v = AdvancedEffectorPack {
        identity,
        set: AdvancedEffectorSetId::parse(b[32])?,
        supply_source: RcsSupplySourceId::parse(b[33])?,
        canard_count: b[34],
        jet_count: b[35],
        coefficient_count: b[36],
        supply_count: b[37],
        flags: g16(b, 38),
        vehicle_identity: g32(b, 40),
        neutral_vehicle_identity: g32(b, 44),
        supply_identity: g32(b, 48),
        provenance_identity: g32(b, 52),
        tank_position_q28: [gi32(b, 56), gi32(b, 60), gi32(b, 64)],
        tank_dry_mass_q21: gi32(b, 68),
        propellant_wet_mass_q21: gi32(b, 72),
        reserve_q15: g16(b, 76),
        canard_hinge_limits_q24: [gi32(b, 80), gi32(b, 84), gi32(b, 88), gi32(b, 92)],
        canards,
        coefficient_knots,
        jets,
        supply_knots,
    };
    if !v.is_valid() {
        return Err(Phase95ContractError::Range);
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorityResidualAllocatorPack {
    pub identity: u32,
    pub allocator: ControlAllocatorId,
    pub set: AdvancedEffectorSetId,
    pub flags: u16,
    pub effector_identity: u32,
    pub legacy_gimbal_identity: u32,
    pub priorities: [u8; 3],
    pub canard_enable_q10: i32,
    pub canard_full_q10: i32,
    pub canard_disable_q10: i32,
    pub reserve_q15: u16,
    pub roll_kp_q15: i32,
    pub roll_kd_q15: i32,
    pub group_authority_q12: [[i32; 3]; 3],
    pub gimbal_mix_q15: [[i16; 2]; 3],
    pub canard_mix_q15: [[i16; 4]; 3],
    pub rcs_mix_q15: [[i16; 12]; 3],
    pub safe_canards: [i16; 4],
    pub safe_gimbal: [i16; 2],
}
impl PriorityResidualAllocatorPack {
    pub fn is_valid(&self) -> bool {
        self.identity != 0
            && self.effector_identity != 0
            && self.priorities.iter().all(|p| *p >= 1 && *p <= 3)
            && self.priorities[0] != self.priorities[1]
            && self.priorities[0] != self.priorities[2]
            && self.priorities[1] != self.priorities[2]
            && self.canard_disable_q10 >= 0
            && self.canard_disable_q10 < self.canard_enable_q10
            && self.canard_enable_q10 <= self.canard_full_q10
            && self.reserve_q15 <= 32768
            && self.roll_kp_q15 >= 0
            && self.roll_kd_q15 >= 0
    }
}
pub fn write_allocator_pack(
    v: &PriorityResidualAllocatorPack,
    o: &mut [u8],
) -> Result<(), Phase95ContractError> {
    if !v.is_valid() {
        return Err(Phase95ContractError::Range);
    }
    header(o, Kind::Allocator, v.identity)?;
    o[32] = v.allocator as u8;
    o[33] = v.set as u8;
    p16(o, 34, v.flags);
    p32(o, 36, v.effector_identity);
    p32(o, 40, v.legacy_gimbal_identity);
    o[44..47].copy_from_slice(&v.priorities);
    pi32(o, 48, v.canard_enable_q10);
    pi32(o, 52, v.canard_full_q10);
    pi32(o, 56, v.canard_disable_q10);
    p16(o, 60, v.reserve_q15);
    pi32(o, 64, v.roll_kp_q15);
    pi32(o, 68, v.roll_kd_q15);
    for axis in 0..3 {
        for group in 0..3 {
            pi32(
                o,
                84 + (axis * 3 + group) * 4,
                v.group_authority_q12[axis][group],
            )
        }
        for j in 0..2 {
            pi16(o, 128 + (axis * 2 + j) * 2, v.gimbal_mix_q15[axis][j])
        }
        for c in 0..4 {
            pi16(o, 144 + (axis * 4 + c) * 2, v.canard_mix_q15[axis][c])
        }
        for j in 0..12 {
            pi16(o, 176 + (axis * 12 + j) * 2, v.rcs_mix_q15[axis][j])
        }
    }
    for i in 0..4 {
        pi16(o, 72 + i * 2, v.safe_canards[i])
    }
    for i in 0..2 {
        pi16(o, 80 + i * 2, v.safe_gimbal[i])
    }
    seal(o);
    Ok(())
}
pub fn parse_allocator_pack(
    b: &[u8],
) -> Result<PriorityResidualAllocatorPack, Phase95ContractError> {
    let identity = validate(b, Kind::Allocator)?;
    if b[47] != 0 || b[62] != 0 || b[63] != 0 {
        return Err(Phase95ContractError::Reserved);
    }
    zero(b, 120, 128)?;
    zero(b, 168, 176)?;
    zero(b, 248, KPA9_LENGTH - 4)?;
    let mut authority = [[0; 3]; 3];
    let mut gm = [[0; 2]; 3];
    let mut cm = [[0; 4]; 3];
    let mut rm = [[0; 12]; 3];
    for axis in 0..3 {
        for group in 0..3 {
            authority[axis][group] = gi32(b, 84 + (axis * 3 + group) * 4)
        }
        for j in 0..2 {
            gm[axis][j] = gi16(b, 128 + (axis * 2 + j) * 2)
        }
        for c in 0..4 {
            cm[axis][c] = gi16(b, 144 + (axis * 4 + c) * 2)
        }
        for j in 0..12 {
            rm[axis][j] = gi16(b, 176 + (axis * 12 + j) * 2)
        }
    }
    let v = PriorityResidualAllocatorPack {
        identity,
        allocator: ControlAllocatorId::parse(b[32])?,
        set: AdvancedEffectorSetId::parse(b[33])?,
        flags: g16(b, 34),
        effector_identity: g32(b, 36),
        legacy_gimbal_identity: g32(b, 40),
        priorities: [b[44], b[45], b[46]],
        canard_enable_q10: gi32(b, 48),
        canard_full_q10: gi32(b, 52),
        canard_disable_q10: gi32(b, 56),
        reserve_q15: g16(b, 60),
        roll_kp_q15: gi32(b, 64),
        roll_kd_q15: gi32(b, 68),
        group_authority_q12: authority,
        gimbal_mix_q15: gm,
        canard_mix_q15: cm,
        rcs_mix_q15: rm,
        safe_canards: [gi16(b, 72), gi16(b, 74), gi16(b, 76), gi16(b, 78)],
        safe_gimbal: [gi16(b, 80), gi16(b, 82)],
    };
    if !v.is_valid() {
        return Err(Phase95ContractError::Range);
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedEvaluationRequest {
    pub identity: u32,
    pub model_profile: u8,
    pub reference_frame: u8,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub wind_identity: u32,
    pub avionics_identity: u32,
    pub legacy_gimbal_identity: u32,
    pub effector_identity: u32,
    pub allocator_identity: u32,
    pub uncertainty_identity: u32,
    pub evaluator_identity: u32,
}
impl AdvancedEvaluationRequest {
    pub fn is_valid(&self) -> bool {
        self.identity != 0
            && self.model_profile == 4
            && self.reference_frame == 1
            && self.vehicle_identity != 0
            && self.motor_identity != 0
            && self.mission_identity != 0
            && self.wind_identity != 0
            && self.avionics_identity != 0
            && self.effector_identity != 0
            && self.allocator_identity != 0
            && self.evaluator_identity != 0
    }
}
pub fn write_advanced_evaluation_request(
    v: &AdvancedEvaluationRequest,
    o: &mut [u8],
) -> Result<(), Phase95ContractError> {
    if !v.is_valid() {
        return Err(Phase95ContractError::Range);
    }
    header(o, Kind::Evaluation, v.identity)?;
    o[32] = v.model_profile;
    o[33] = v.reference_frame;
    p32(o, 36, v.vehicle_identity);
    p32(o, 40, v.motor_identity);
    p32(o, 44, v.mission_identity);
    p32(o, 48, v.wind_identity);
    p32(o, 52, v.avionics_identity);
    p32(o, 56, v.legacy_gimbal_identity);
    p32(o, 60, v.effector_identity);
    p32(o, 64, v.allocator_identity);
    p32(o, 68, v.uncertainty_identity);
    p32(o, 72, v.evaluator_identity);
    seal(o);
    Ok(())
}
pub fn parse_advanced_evaluation_request(
    b: &[u8],
) -> Result<AdvancedEvaluationRequest, Phase95ContractError> {
    let identity = validate(b, Kind::Evaluation)?;
    zero(b, 34, 36)?;
    zero(b, 76, KLE9_LENGTH - 4)?;
    let v = AdvancedEvaluationRequest {
        identity,
        model_profile: b[32],
        reference_frame: b[33],
        vehicle_identity: g32(b, 36),
        motor_identity: g32(b, 40),
        mission_identity: g32(b, 44),
        wind_identity: g32(b, 48),
        avionics_identity: g32(b, 52),
        legacy_gimbal_identity: g32(b, 56),
        effector_identity: g32(b, 60),
        allocator_identity: g32(b, 64),
        uncertainty_identity: g32(b, 68),
        evaluator_identity: g32(b, 72),
    };
    if !v.is_valid() {
        return Err(if v.reference_frame != 1 {
            Phase95ContractError::Unsupported
        } else {
            Phase95ContractError::Range
        });
    }
    Ok(v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedEffectorEvaluationSummary {
    pub physical: EvaluationSummary,
    pub physical_summary_identity: u32,
    pub avionics_identity: u32,
    pub legacy_gimbal_identity: u32,
    pub effector_identity: u32,
    pub allocator_identity: u32,
    pub uncertainty_identity: u32,
    pub evaluator_identity: u32,
    pub releases: u32,
    pub max_navigation_error_q13: i32,
    pub max_attitude_error_turn16: i16,
    pub alarms: u16,
    pub saturation_count: u32,
    pub pulse_count: u32,
    pub valve_edge_count: u32,
    pub depletion_count: u16,
    pub authority_handoffs: u16,
    pub air_fallback_epochs: u16,
    pub deployment_feedback: u16,
    pub max_hinge_q24: [i32; 4],
    pub rcs_initial_propellant_q21: i32,
    pub rcs_final_propellant_q21: i32,
    pub checksum_chains: [u32; 8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kas9Record {
    pub identity: u32,
    pub summary: AdvancedEffectorEvaluationSummary,
}

pub fn write_advanced_effector_summary(
    value: AdvancedEffectorEvaluationSummary,
    output: &mut [u8],
) -> Result<(), Phase95ContractError> {
    if value.physical.profile != ModelProfileId::LocalEnu6DofV1
        || value.physical_summary_identity != spatial_evaluation_identity(value.physical)
        || value.avionics_identity == 0
        || value.effector_identity == 0
        || value.allocator_identity == 0
        || value.evaluator_identity == 0
        || value.rcs_final_propellant_q21 < 0
        || value.rcs_final_propellant_q21 > value.rcs_initial_propellant_q21
    {
        return Err(Phase95ContractError::Identity);
    }
    let mut identity = value.physical_summary_identity
        ^ value.effector_identity.rotate_left(7)
        ^ value.allocator_identity.rotate_left(13);
    for word in value.checksum_chains {
        identity = identity.rotate_left(5) ^ word;
    }
    header(output, Kind::Summary, identity.max(1))?;
    output[32] = value.physical.profile as u8;
    output[33] = value.physical.outcome as u8;
    output[34] = value.physical.numeric_faults;
    p32(output, 36, value.physical_summary_identity);
    p32(output, 40, value.avionics_identity);
    p32(output, 44, value.legacy_gimbal_identity);
    p32(output, 48, value.effector_identity);
    p32(output, 52, value.allocator_identity);
    p32(output, 56, value.uncertainty_identity);
    p32(output, 60, value.evaluator_identity);
    p32(output, 64, value.physical.steps);
    p32(output, 68, value.physical.events);
    p32(output, 72, value.releases);
    pi32(output, 76, value.max_navigation_error_q13);
    pi16(output, 80, value.max_attitude_error_turn16);
    p16(output, 82, value.alarms);
    p32(output, 84, value.saturation_count);
    p32(output, 88, value.pulse_count);
    p32(output, 92, value.valve_edge_count);
    p16(output, 96, value.depletion_count);
    p16(output, 98, value.authority_handoffs);
    p16(output, 100, value.air_fallback_epochs);
    p16(output, 102, value.deployment_feedback);
    for i in 0..4 {
        pi32(output, 104 + i * 4, value.max_hinge_q24[i]);
    }
    pi32(output, 120, value.rcs_initial_propellant_q21);
    pi32(output, 124, value.rcs_final_propellant_q21);
    pi32(
        output,
        128,
        value
            .rcs_initial_propellant_q21
            .saturating_sub(value.rcs_final_propellant_q21),
    );
    for i in 0..8 {
        p32(output, 160 + i * 4, value.checksum_chains[i]);
    }
    let mut physical = [0u8; crate::phase8_format::KSR8_LENGTH];
    encode_ksr8(value.physical, &mut physical).map_err(|_| Phase95ContractError::Range)?;
    output[224..480].copy_from_slice(&physical);
    seal(output);
    Ok(())
}

pub fn parse_advanced_effector_summary(input: &[u8]) -> Result<Kas9Record, Phase95ContractError> {
    let identity = validate(input, Kind::Summary)?;
    zero(input, 35, 36)?;
    zero(input, 132, 160)?;
    zero(input, 192, 224)?;
    zero(input, 480, KAS9_LENGTH - 4)?;
    if input[32] != ModelProfileId::LocalEnu6DofV1 as u8 {
        return Err(Phase95ContractError::Unsupported);
    }
    let physical = parse_ksr8(&input[224..480])
        .map_err(|_| Phase95ContractError::Range)?
        .summary;
    if physical.outcome as u8 != input[33]
        || physical.numeric_faults != input[34]
        || physical.steps != g32(input, 64)
        || physical.events != g32(input, 68)
        || spatial_evaluation_identity(physical) != g32(input, 36)
    {
        return Err(Phase95ContractError::Identity);
    }
    let mut hinges = [0; 4];
    for i in 0..4 {
        hinges[i] = gi32(input, 104 + i * 4);
    }
    let mut checks = [0; 8];
    for i in 0..8 {
        checks[i] = g32(input, 160 + i * 4);
    }
    let initial = gi32(input, 120);
    let final_prop = gi32(input, 124);
    if initial < 0
        || final_prop < 0
        || final_prop > initial
        || gi32(input, 128) != initial - final_prop
    {
        return Err(Phase95ContractError::Range);
    }
    let summary = AdvancedEffectorEvaluationSummary {
        physical,
        physical_summary_identity: g32(input, 36),
        avionics_identity: g32(input, 40),
        legacy_gimbal_identity: g32(input, 44),
        effector_identity: g32(input, 48),
        allocator_identity: g32(input, 52),
        uncertainty_identity: g32(input, 56),
        evaluator_identity: g32(input, 60),
        releases: g32(input, 72),
        max_navigation_error_q13: gi32(input, 76),
        max_attitude_error_turn16: gi16(input, 80),
        alarms: g16(input, 82),
        saturation_count: g32(input, 84),
        pulse_count: g32(input, 88),
        valve_edge_count: g32(input, 92),
        depletion_count: g16(input, 96),
        authority_handoffs: g16(input, 98),
        air_fallback_epochs: g16(input, 100),
        deployment_feedback: g16(input, 102),
        max_hinge_q24: hinges,
        rcs_initial_propellant_q21: initial,
        rcs_final_propellant_q21: final_prop,
        checksum_chains: checks,
    };
    Ok(Kas9Record { identity, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::EvaluationOutcome;
    #[allow(dead_code)]
    mod independent {
        include!("../../phase9_5/generated/contract_vectors_v1.rs");
    }
    fn canard() -> CanardInstallation {
        CanardInstallation {
            position_q28: [120_000_000, 0, 0],
            normal_q15: [0, 32767, 0],
            hinge_axis_q15: [32767, 0, 0],
            root_q28: 16_106_127,
            tip_q28: 8_053_064,
            span_q28: 6_710_886,
            sweep_q28: 5_368_709,
            mass_q21: 52_429,
            inertia_q19: [100, 200, 300],
            limit_turn16: 1820,
            slew_turn16_per_release: 683,
            lag_releases: 1,
            flags: 0,
            failure_identity: 11,
        }
    }
    fn jet(index: i32) -> RcsJetInstallation {
        RcsJetInstallation {
            position_q28: [index * 10_000_000, 0, 0],
            direction_q30: [0, 1 << 30, 0],
            nominal_thrust_q23: 1 << 23,
            specific_impulse_q16: 55 << 16,
            min_pulse_quanta: 1,
            max_pulse_quanta: 8,
            valve_delay_quanta: 0,
            flags: 0,
            failure_identity: 20 + index as u16,
            provenance_identity: 30 + index as u32,
        }
    }
    fn pack() -> AdvancedEffectorPack {
        let mut canards = [CanardInstallation::EMPTY; 4];
        canards.fill(canard());
        let mut coeff = [CanardCoefficientKnot::ZERO; 8];
        coeff[0] = CanardCoefficientKnot {
            mach_q24: 1,
            control_q24: 1 << 24,
            drag_q24: 1 << 20,
            hinge_q24: 1 << 23,
        };
        coeff[1] = CanardCoefficientKnot {
            mach_q24: 1 << 23,
            control_q24: 1 << 24,
            drag_q24: 1 << 20,
            hinge_q24: 1 << 23,
        };
        let mut jets = [RcsJetInstallation::EMPTY; 12];
        for (i, j) in jets.iter_mut().enumerate() {
            *j = jet(i as i32)
        }
        let mut supply = [SupplyKnot::ZERO; 16];
        supply[0] = SupplyKnot {
            remaining_propellant_q21: 1,
            pressure_q8: 500_000_000,
            thrust_scale_q30: 1 << 29,
            mass_flow_scale_q30: 1 << 29,
        };
        supply[1] = SupplyKnot {
            remaining_propellant_q21: 1 << 20,
            pressure_q8: 1_000_000_000,
            thrust_scale_q30: 1 << 30,
            mass_flow_scale_q30: 1 << 30,
        };
        AdvancedEffectorPack {
            identity: 1,
            set: AdvancedEffectorSetId::GimbalCanardRcs,
            supply_source: RcsSupplySourceId::IdealIsothermalBlowdownV1,
            flags: 0,
            vehicle_identity: 2,
            neutral_vehicle_identity: 3,
            supply_identity: 4,
            provenance_identity: 5,
            tank_position_q28: [1, 2, 3],
            tank_dry_mass_q21: 1 << 18,
            propellant_wet_mass_q21: 1 << 20,
            reserve_q15: 6554,
            canard_hinge_limits_q24: [1 << 23; 4],
            canard_count: 4,
            jet_count: 12,
            coefficient_count: 2,
            supply_count: 2,
            canards,
            coefficient_knots: coeff,
            jets,
            supply_knots: supply,
        }
    }
    #[test]
    fn strict_packs_round_trip_and_reject_reserved() {
        let v = pack();
        let mut b = [0; KPE9_LENGTH];
        write_effector_pack(&v, &mut b).unwrap();
        assert_eq!(parse_effector_pack(&b), Ok(v));
        b[1500] = 1;
        seal(&mut b);
        assert_eq!(parse_effector_pack(&b), Err(Phase95ContractError::Reserved));
    }
    #[test]
    fn allocator_and_request_are_strict() {
        let a = PriorityResidualAllocatorPack {
            identity: 6,
            allocator: ControlAllocatorId::PriorityResidualV1,
            set: AdvancedEffectorSetId::GimbalCanardRcs,
            flags: 0,
            effector_identity: 1,
            legacy_gimbal_identity: 7,
            priorities: [1, 2, 3],
            canard_enable_q10: 300 << 10,
            canard_full_q10: 2000 << 10,
            canard_disable_q10: 200 << 10,
            reserve_q15: 6554,
            roll_kp_q15: 1000,
            roll_kd_q15: 2000,
            group_authority_q12: [[1; 3]; 3],
            gimbal_mix_q15: [[1; 2]; 3],
            canard_mix_q15: [[2; 4]; 3],
            rcs_mix_q15: [[3; 12]; 3],
            safe_canards: [0; 4],
            safe_gimbal: [0; 2],
        };
        let mut ab = [0; KPA9_LENGTH];
        write_allocator_pack(&a, &mut ab).unwrap();
        assert_eq!(parse_allocator_pack(&ab), Ok(a));
        let r = AdvancedEvaluationRequest {
            identity: 8,
            model_profile: 4,
            reference_frame: 1,
            vehicle_identity: 2,
            motor_identity: 3,
            mission_identity: 4,
            wind_identity: 5,
            avionics_identity: 6,
            legacy_gimbal_identity: 7,
            effector_identity: 1,
            allocator_identity: 6,
            uncertainty_identity: 0,
            evaluator_identity: 9,
        };
        let mut rb = [0; KLE9_LENGTH];
        write_advanced_evaluation_request(&r, &mut rb).unwrap();
        assert_eq!(rb, independent::KLE9_VECTOR);
        assert_eq!(parse_advanced_evaluation_request(&rb), Ok(r));
        rb[100] = 1;
        seal(&mut rb);
        assert_eq!(
            parse_advanced_evaluation_request(&rb),
            Err(Phase95ContractError::Reserved)
        );
    }
    #[test]
    fn kas9_round_trip_is_strict() {
        let mut physical = EvaluationSummary::empty(ModelProfileId::LocalEnu6DofV1);
        physical.outcome = EvaluationOutcome::GroundContact;
        physical.steps = 99;
        physical.events = 63;
        physical.identities = [1, 2, 3, 4, 5, 6];
        physical.source_checksums = [7, 8, 9, 10, 11];
        let value = AdvancedEffectorEvaluationSummary {
            physical,
            physical_summary_identity: spatial_evaluation_identity(physical),
            avionics_identity: 12,
            legacy_gimbal_identity: 0,
            effector_identity: 13,
            allocator_identity: 14,
            uncertainty_identity: 0,
            evaluator_identity: 15,
            releases: 88,
            max_navigation_error_q13: 16,
            max_attitude_error_turn16: 17,
            alarms: 0,
            saturation_count: 18,
            pulse_count: 19,
            valve_edge_count: 20,
            depletion_count: 0,
            authority_handoffs: 21,
            air_fallback_epochs: 22,
            deployment_feedback: 3,
            max_hinge_q24: [23, 24, 25, 26],
            rcs_initial_propellant_q21: 1000,
            rcs_final_propellant_q21: 750,
            checksum_chains: [27, 28, 29, 30, 31, 32, 33, 34],
        };
        let mut bytes = [0u8; KAS9_LENGTH];
        write_advanced_effector_summary(value, &mut bytes).unwrap();
        assert_eq!(
            parse_advanced_effector_summary(&bytes).unwrap().summary,
            value
        );
        bytes[200] = 1;
        seal(&mut bytes);
        assert_eq!(
            parse_advanced_effector_summary(&bytes),
            Err(Phase95ContractError::Reserved)
        );
    }
}
