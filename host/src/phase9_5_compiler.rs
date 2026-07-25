//! Offline Phase 9.5 compiler for provenance-bearing advanced-effector sources.

use ksa64_core::phase8_format::KVP8_LENGTH;
use ksa64_core::phase8_numeric::{
    SpatialInertia, SpatialMass, SpatialMomentArm, SPATIAL_INERTIA_FRACTIONAL_BITS,
    SPATIAL_MASS_FRACTIONAL_BITS, SPATIAL_MOMENT_ARM_FRACTIONAL_BITS,
    SPATIAL_POSITION_FRACTIONAL_BITS,
};
use ksa64_core::phase8_pack::{
    encode_spatial_vehicle_pack, parse_spatial_vehicle_pack, Phase8PackError, SpatialVehiclePack,
};
use ksa64_core::phase9_5_contract::{
    write_allocator_pack, write_effector_pack, AdvancedEffectorPack, AdvancedEffectorSetId,
    CanardCoefficientKnot, CanardInstallation, ControlAllocatorId, Phase95ContractError,
    PriorityResidualAllocatorPack, RcsJetInstallation, RcsSupplySourceId, SupplyKnot, KPA9_LENGTH,
    KPE9_LENGTH, MAX_CANARDS, MAX_CANARD_COEFFICIENT_KNOTS, MAX_RCS_JETS, MAX_SUPPLY_KNOTS,
};
use ksa64_core::scenario::fnv1a_32;
use serde::{Deserialize, Serialize};

use crate::phase7_compiler::{identity, parse_scaled};

#[derive(Debug)]
pub enum AdvancedCompileError {
    Json(serde_json::Error),
    Schema,
    Provenance,
    Decimal,
    Range,
    Identity,
    Variant,
    Pack8(Phase8PackError),
    Pack95(Phase95ContractError),
}
impl From<serde_json::Error> for AdvancedCompileError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
impl From<Phase8PackError> for AdvancedCompileError {
    fn from(value: Phase8PackError) -> Self {
        Self::Pack8(value)
    }
}
impl From<Phase95ContractError> for AdvancedCompileError {
    fn from(value: Phase95ContractError) -> Self {
        Self::Pack95(value)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProvenanceKind {
    Published,
    Measured,
    DeclaredAssumption,
    Derived,
}
#[derive(Clone, Debug, Deserialize)]
struct Provenance {
    kind: ProvenanceKind,
    reference: String,
    note: String,
}
impl Provenance {
    fn validate(&self) -> Result<(), AdvancedCompileError> {
        if self.note.trim().is_empty() {
            return Err(AdvancedCompileError::Provenance);
        }
        if matches!(
            self.kind,
            ProvenanceKind::Published | ProvenanceKind::Measured
        ) && self.reference.trim().is_empty()
        {
            return Err(AdvancedCompileError::Provenance);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize)]
struct SourcedDecimal {
    value: String,
    provenance: Provenance,
}
impl SourcedDecimal {
    fn raw(&self, bits: u8) -> Result<i32, AdvancedCompileError> {
        self.provenance.validate()?;
        parse_scaled(&self.value, bits).map_err(|_| AdvancedCompileError::Decimal)
    }
    fn number(&self) -> Result<f64, AdvancedCompileError> {
        self.provenance.validate()?;
        if self.value.contains(['e', 'E', '+']) {
            return Err(AdvancedCompileError::Decimal);
        }
        let value: f64 = self
            .value
            .parse()
            .map_err(|_| AdvancedCompileError::Decimal)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(AdvancedCompileError::Decimal)
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
struct CanardSource {
    provenance: Provenance,
    root_m: SourcedDecimal,
    tip_m: SourcedDecimal,
    span_m: SourcedDecimal,
    sweep_m: SourcedDecimal,
    station_from_nose_m: SourcedDecimal,
    radial_station_m: SourcedDecimal,
    mass_each_kg: SourcedDecimal,
    travel_deg: SourcedDecimal,
    slew_deg_per_s: SourcedDecimal,
    lag_releases: u8,
    hinge_limit_nm: SourcedDecimal,
    coefficient_knots: Vec<[String; 4]>,
}
#[derive(Clone, Debug, Deserialize)]
struct RcsSource {
    provenance: Provenance,
    fore_station_m: SourcedDecimal,
    aft_station_m: SourcedDecimal,
    radial_station_m: SourcedDecimal,
    tank_station_m: SourcedDecimal,
    tank_dry_mass_kg: SourcedDecimal,
    propellant_mass_kg: SourcedDecimal,
    jet_hardware_mass_kg: SourcedDecimal,
    nominal_thrust_n: SourcedDecimal,
    specific_impulse_s: SourcedDecimal,
    full_pressure_pa: SourcedDecimal,
    empty_pressure_pa: SourcedDecimal,
    reserve_fraction: SourcedDecimal,
}
#[derive(Clone, Debug, Deserialize)]
struct VariantSource {
    name: String,
    vehicle_identity: String,
    effector_identity: String,
    allocator_identity: String,
    set: String,
    supply: String,
    include_gimbal: bool,
    experimental: bool,
    provenance: Provenance,
}
#[derive(Deserialize)]
struct AdvancedSource {
    schema: String,
    canard: CanardSource,
    firestorm_rcs: RcsSource,
    research_rcs: RcsSource,
    variants: Vec<VariantSource>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdvancedCompileReport {
    pub name: String,
    pub experimental: bool,
    pub base_vehicle_identity: u32,
    pub vehicle_identity: u32,
    pub effector_identity: u32,
    pub allocator_identity: u32,
    pub dry_mass_kg: f64,
    pub dry_cg_from_nose_m: f64,
    pub dry_inertia_kgm2: [f64; 3],
    pub canard_count: u8,
    pub jet_count: u8,
    pub supply_source: String,
    pub provenance_identity: u32,
}
pub struct CompiledAdvancedVariant {
    pub name: String,
    pub vehicle: [u8; KVP8_LENGTH],
    pub effector: [u8; KPE9_LENGTH],
    pub allocator: [u8; KPA9_LENGTH],
    pub report: AdvancedCompileReport,
}
pub struct CompiledAdvancedSet {
    pub variants: Vec<CompiledAdvancedVariant>,
}

fn quantize(value: f64, bits: u8) -> Result<i32, AdvancedCompileError> {
    if !value.is_finite() {
        return Err(AdvancedCompileError::Range);
    }
    let scaled = (value * (1u64 << bits) as f64).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        Err(AdvancedCompileError::Range)
    } else {
        Ok(scaled as i32)
    }
}
fn id(text: &str) -> Result<u32, AdvancedCompileError> {
    identity(text).map_err(|_| AdvancedCompileError::Identity)
}
fn add_point_masses(
    base: SpatialVehiclePack,
    derivative_identity: u32,
    manifest_identity: u32,
    masses: &[(f64, f64, f64)],
) -> Result<(SpatialVehiclePack, f64, f64, [f64; 3]), AdvancedCompileError> {
    let scale_mass = (1u64 << SPATIAL_MASS_FRACTIONAL_BITS) as f64;
    let scale_arm = (1u64 << SPATIAL_MOMENT_ARM_FRACTIONAL_BITS) as f64;
    let scale_inertia = (1u64 << SPATIAL_INERTIA_FRACTIONAL_BITS) as f64;
    let base_mass = f64::from(base.dry_mass.raw()) / scale_mass;
    let base_cg = f64::from(base.dry_cg_from_nose.raw()) / scale_arm;
    let mut total = base_mass;
    let mut moment = base_mass * base_cg;
    for &(mass, x, _) in masses {
        if mass <= 0.0
            || x < 0.0
            || x > f64::from(base.length.raw()) / (1u64 << SPATIAL_POSITION_FRACTIONAL_BITS) as f64
        {
            return Err(AdvancedCompileError::Range);
        }
        total += mass;
        moment += mass * x;
    }
    let cg = moment / total;
    let mut inertia = [
        f64::from(base.dry_inertia[0].raw()) / scale_inertia,
        f64::from(base.dry_inertia[1].raw()) / scale_inertia,
        f64::from(base.dry_inertia[2].raw()) / scale_inertia,
    ];
    let base_parallel = base_mass * (base_cg - cg).powi(2);
    inertia[1] += base_parallel;
    inertia[2] += base_parallel;
    for &(mass, x, radial) in masses {
        inertia[0] += mass * radial * radial;
        let parallel = mass * ((x - cg).powi(2) + 0.5 * radial * radial);
        inertia[1] += parallel;
        inertia[2] += parallel;
    }
    let mut derivative = base;
    derivative.identity = derivative_identity;
    derivative.source_manifest_identity = manifest_identity;
    derivative.dry_mass = SpatialMass::from_raw(quantize(total, SPATIAL_MASS_FRACTIONAL_BITS)?);
    derivative.dry_cg_from_nose =
        SpatialMomentArm::from_raw(quantize(cg, SPATIAL_MOMENT_ARM_FRACTIONAL_BITS)?);
    derivative.dry_inertia = [
        SpatialInertia::from_raw(quantize(inertia[0], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
        SpatialInertia::from_raw(quantize(inertia[1], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
        SpatialInertia::from_raw(quantize(inertia[2], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
    ];
    if !derivative.is_valid() {
        return Err(AdvancedCompileError::Range);
    }
    Ok((derivative, total, cg, inertia))
}
fn turn16(degrees: f64) -> Result<i16, AdvancedCompileError> {
    let value = (degrees * 65536.0 / 360.0).round();
    if value <= 0.0 || value > i16::MAX as f64 {
        Err(AdvancedCompileError::Range)
    } else {
        Ok(value as i16)
    }
}
fn compile_canards(
    source: &CanardSource,
) -> Result<
    (
        [CanardInstallation; MAX_CANARDS],
        [CanardCoefficientKnot; MAX_CANARD_COEFFICIENT_KNOTS],
        u8,
    ),
    AdvancedCompileError,
> {
    source.provenance.validate()?;
    let hinge_limit = source.hinge_limit_nm.number()?;
    if hinge_limit <= 0.0
        || source.lag_releases > 8
        || source.coefficient_knots.len() < 2
        || source.coefficient_knots.len() > MAX_CANARD_COEFFICIENT_KNOTS
    {
        return Err(AdvancedCompileError::Range);
    }
    let x = source.station_from_nose_m.raw(28)?;
    let r = source.radial_station_m.raw(28)?;
    let root = source.root_m.raw(28)?;
    let tip = source.tip_m.raw(28)?;
    let span = source.span_m.raw(28)?;
    let sweep = source.sweep_m.raw(28)?;
    let mass = source.mass_each_kg.raw(21)?;
    let limit = turn16(source.travel_deg.number()?)?;
    let slew = turn16(source.slew_deg_per_s.number()? / 32.0)?;
    let inertia_value = quantize(
        source.mass_each_kg.number()? * source.span_m.number()?.powi(2) / 12.0,
        19,
    )?
    .max(1);
    let mut result = [CanardInstallation::EMPTY; MAX_CANARDS];
    let positions = [[x, r, 0], [x, -r, 0], [x, 0, r], [x, 0, -r]];
    let normals = [[0, 0, 32767], [0, 0, -32767], [0, 32767, 0], [0, -32767, 0]];
    let hinges = [[0, 32767, 0], [0, 32767, 0], [0, 0, 32767], [0, 0, 32767]];
    for index in 0..MAX_CANARDS {
        result[index] = CanardInstallation {
            position_q28: positions[index],
            normal_q15: normals[index],
            hinge_axis_q15: hinges[index],
            root_q28: root,
            tip_q28: tip,
            span_q28: span,
            sweep_q28: sweep,
            mass_q21: mass,
            inertia_q19: [inertia_value; 3],
            limit_turn16: limit,
            slew_turn16_per_release: slew,
            lag_releases: source.lag_releases,
            flags: 0,
            failure_identity: 0x9500 + index as u16,
        };
    }
    let mut knots = [CanardCoefficientKnot::ZERO; MAX_CANARD_COEFFICIENT_KNOTS];
    let mut previous = -1;
    for (index, values) in source.coefficient_knots.iter().enumerate() {
        let mach = parse_scaled(&values[0], 24).map_err(|_| AdvancedCompileError::Decimal)?;
        let control = parse_scaled(&values[1], 24).map_err(|_| AdvancedCompileError::Decimal)?;
        let drag = parse_scaled(&values[2], 24).map_err(|_| AdvancedCompileError::Decimal)?;
        let hinge = parse_scaled(&values[3], 24).map_err(|_| AdvancedCompileError::Decimal)?;
        if mach <= previous || control <= 0 || drag < 0 || hinge <= 0 {
            return Err(AdvancedCompileError::Range);
        }
        knots[index] = CanardCoefficientKnot {
            mach_q24: mach,
            control_q24: control,
            drag_q24: drag,
            hinge_q24: hinge,
        };
        previous = mach;
    }
    Ok((result, knots, source.coefficient_knots.len() as u8))
}
fn jet(
    position: [f64; 3],
    direction: [i32; 3],
    source: &RcsSource,
    index: usize,
    provenance_identity: u32,
) -> Result<RcsJetInstallation, AdvancedCompileError> {
    Ok(RcsJetInstallation {
        position_q28: [
            quantize(position[0], 28)?,
            quantize(position[1], 28)?,
            quantize(position[2], 28)?,
        ],
        direction_q30: direction,
        nominal_thrust_q23: source.nominal_thrust_n.raw(23)?,
        specific_impulse_q16: source.specific_impulse_s.raw(16)?,
        min_pulse_quanta: 1,
        max_pulse_quanta: 8,
        valve_delay_quanta: 0,
        flags: 0,
        failure_identity: 0x9600 + index as u16,
        provenance_identity: provenance_identity.wrapping_add(index as u32 + 1),
    })
}
fn compile_rcs(
    source: &RcsSource,
    provenance_identity: u32,
    supply_source: RcsSupplySourceId,
) -> Result<
    (
        [RcsJetInstallation; MAX_RCS_JETS],
        [SupplyKnot; MAX_SUPPLY_KNOTS],
        u8,
    ),
    AdvancedCompileError,
> {
    source.provenance.validate()?;
    let fore = source.fore_station_m.number()?;
    let aft = source.aft_station_m.number()?;
    let mid = (fore + aft) * 0.5;
    let radial = source.radial_station_m.number()?;
    let one = 1i32 << 30;
    let definitions = [
        ([mid, radial, 0.0], [0, 0, one]),
        ([mid, -radial, 0.0], [0, 0, -one]),
        ([mid, radial, 0.0], [0, 0, -one]),
        ([mid, -radial, 0.0], [0, 0, one]),
        ([fore, 0.0, 0.0], [0, 0, one]),
        ([aft, 0.0, 0.0], [0, 0, -one]),
        ([fore, 0.0, 0.0], [0, 0, -one]),
        ([aft, 0.0, 0.0], [0, 0, one]),
        ([fore, 0.0, 0.0], [0, -one, 0]),
        ([aft, 0.0, 0.0], [0, one, 0]),
        ([fore, 0.0, 0.0], [0, one, 0]),
        ([aft, 0.0, 0.0], [0, -one, 0]),
    ];
    let mut jets = [RcsJetInstallation::EMPTY; MAX_RCS_JETS];
    for index in 0..MAX_RCS_JETS {
        jets[index] = jet(
            definitions[index].0,
            definitions[index].1,
            source,
            index,
            provenance_identity,
        )?;
    }
    let wet = source.propellant_mass_kg.number()?;
    let empty_pressure = source.empty_pressure_pa.number()?;
    let full_pressure = source.full_pressure_pa.number()?;
    if wet <= 0.0 || empty_pressure <= 0.0 || full_pressure < empty_pressure {
        return Err(AdvancedCompileError::Range);
    }
    let mut knots = [SupplyKnot::ZERO; MAX_SUPPLY_KNOTS];
    let fractions = [0.0, 0.25, 0.5, 0.75, 1.0];
    for (index, fraction) in fractions.iter().copied().enumerate() {
        let pressure = match supply_source {
            RcsSupplySourceId::RegulatedV1 => full_pressure,
            RcsSupplySourceId::IdealIsothermalBlowdownV1 => {
                empty_pressure + (full_pressure - empty_pressure) * fraction
            }
            RcsSupplySourceId::None => return Err(AdvancedCompileError::Variant),
        };
        let scale = if supply_source == RcsSupplySourceId::RegulatedV1 {
            1.0
        } else {
            (pressure / full_pressure).max(0.2)
        };
        knots[index] = SupplyKnot {
            remaining_propellant_q21: quantize(wet * fraction, 21)?,
            pressure_q8: quantize(pressure, 8)?,
            thrust_scale_q30: quantize(scale, 30)?,
            mass_flow_scale_q30: quantize(scale, 30)?,
        };
    }
    Ok((jets, knots, fractions.len() as u8))
}
fn compile_allocator(
    set: AdvancedEffectorSetId,
    identity_value: u32,
    effector_identity: u32,
    include_gimbal: bool,
) -> PriorityResidualAllocatorPack {
    let mut canard_mix = [[0i16; 4]; 3];
    canard_mix[0] = [16384, 16384, -16384, -16384];
    canard_mix[1] = [32767, -32767, 0, 0];
    canard_mix[2] = [0, 0, -32767, 32767];
    let mut rcs_mix = [[0i16; 12]; 3];
    for (axis, mix) in rcs_mix.iter_mut().enumerate() {
        let at = axis * 4;
        mix[at] = 32767;
        mix[at + 1] = 32767;
        mix[at + 2] = -32767;
        mix[at + 3] = -32767;
    }
    PriorityResidualAllocatorPack {
        identity: identity_value,
        allocator: ControlAllocatorId::PriorityResidualV1,
        set,
        flags: 0,
        effector_identity,
        legacy_gimbal_identity: if include_gimbal { 0x8500_0002 } else { 0 },
        priorities: [1, 2, 3],
        canard_enable_q10: 300 << 10,
        canard_full_q10: 2000 << 10,
        canard_disable_q10: 200 << 10,
        reserve_q15: 6554,
        roll_kp_q15: 14_000,
        roll_kd_q15: 4_096,
        group_authority_q12: [[0, 1638, 2048], [2048, 2458, 2048], [2048, 2458, 2048]],
        gimbal_mix_q15: [[0, 0], [32767, 0], [0, 32767]],
        canard_mix_q15: canard_mix,
        rcs_mix_q15: rcs_mix,
        safe_canards: [0; 4],
        safe_gimbal: [0; 2],
    }
}

pub fn compile_advanced_sources(
    base_vehicle_bytes: &[u8],
    source_bytes: &[u8],
) -> Result<CompiledAdvancedSet, AdvancedCompileError> {
    let source: AdvancedSource = serde_json::from_slice(source_bytes)?;
    if source.schema != "ksa64.advanced-effector-source-v1" || source.variants.len() != 4 {
        return Err(AdvancedCompileError::Schema);
    }
    let base = parse_spatial_vehicle_pack(base_vehicle_bytes)?;
    let provenance_identity = fnv1a_32(source_bytes);
    if provenance_identity == 0 {
        return Err(AdvancedCompileError::Identity);
    }
    let (canards, coefficient_knots, coefficient_count) = compile_canards(&source.canard)?;
    let mut compiled = Vec::with_capacity(source.variants.len());
    for variant in &source.variants {
        variant.provenance.validate()?;
        let set = match variant.set.as_str() {
            "canard_only" => AdvancedEffectorSetId::CanardOnly,
            "rcs_only" => AdvancedEffectorSetId::RcsOnly,
            "gimbal_canard_rcs" => AdvancedEffectorSetId::GimbalCanardRcs,
            _ => return Err(AdvancedCompileError::Variant),
        };
        let supply_source = match variant.supply.as_str() {
            "none" => RcsSupplySourceId::None,
            "regulated_v1" => RcsSupplySourceId::RegulatedV1,
            "ideal_isothermal_blowdown_v1" => RcsSupplySourceId::IdealIsothermalBlowdownV1,
            _ => return Err(AdvancedCompileError::Variant),
        };
        if set.has_rcs() != (supply_source != RcsSupplySourceId::None)
            || set == AdvancedEffectorSetId::GimbalCanardRcs && !variant.include_gimbal
        {
            return Err(AdvancedCompileError::Variant);
        }
        let vehicle_identity = id(&variant.vehicle_identity)?;
        let effector_identity = id(&variant.effector_identity)?;
        let allocator_identity = id(&variant.allocator_identity)?;
        let rcs_source = if variant.experimental {
            &source.research_rcs
        } else {
            &source.firestorm_rcs
        };
        let mut masses = Vec::new();
        if set.has_canards() {
            for _ in 0..4 {
                masses.push((
                    source.canard.mass_each_kg.number()?,
                    source.canard.station_from_nose_m.number()?,
                    source.canard.radial_station_m.number()?,
                ));
            }
        }
        if set.has_rcs() {
            masses.push((
                rcs_source.tank_dry_mass_kg.number()?,
                rcs_source.tank_station_m.number()?,
                0.0,
            ));
            for index in 0..12 {
                masses.push((
                    rcs_source.jet_hardware_mass_kg.number()?,
                    if index < 4 {
                        (rcs_source.fore_station_m.number()? + rcs_source.aft_station_m.number()?)
                            * 0.5
                    } else if index % 2 == 0 {
                        rcs_source.fore_station_m.number()?
                    } else {
                        rcs_source.aft_station_m.number()?
                    },
                    rcs_source.radial_station_m.number()?,
                ));
            }
        }
        if variant.include_gimbal {
            masses.push((0.020, 1.88, 0.0));
        }
        let (vehicle, dry_mass, cg, inertia) =
            add_point_masses(base, vehicle_identity, provenance_identity, &masses)?;
        let mut effector = AdvancedEffectorPack {
            identity: effector_identity,
            set,
            supply_source,
            flags: u16::from(variant.experimental),
            vehicle_identity,
            neutral_vehicle_identity: vehicle_identity,
            supply_identity: 0,
            provenance_identity,
            tank_position_q28: [0; 3],
            tank_dry_mass_q21: 0,
            propellant_wet_mass_q21: 0,
            reserve_q15: 0,
            canard_hinge_limits_q24: [0; 4],
            canard_count: 0,
            jet_count: 0,
            coefficient_count: 0,
            supply_count: 0,
            canards: [CanardInstallation::EMPTY; MAX_CANARDS],
            coefficient_knots: [CanardCoefficientKnot::ZERO; MAX_CANARD_COEFFICIENT_KNOTS],
            jets: [RcsJetInstallation::EMPTY; MAX_RCS_JETS],
            supply_knots: [SupplyKnot::ZERO; MAX_SUPPLY_KNOTS],
        };
        if set.has_canards() {
            effector.canard_count = 4;
            effector.coefficient_count = coefficient_count;
            effector.canards = canards;
            effector.coefficient_knots = coefficient_knots;
            effector.canard_hinge_limits_q24 = [source.canard.hinge_limit_nm.raw(24)?; 4];
        }
        if set.has_rcs() {
            let (jets, supply_knots, supply_count) =
                compile_rcs(rcs_source, provenance_identity, supply_source)?;
            effector.jet_count = 12;
            effector.supply_count = supply_count;
            effector.jets = jets;
            effector.supply_knots = supply_knots;
            effector.supply_identity = id(if variant.experimental {
                "ksa-x1-regulated-supply-v1"
            } else {
                "firestorm-blowdown-supply-v1"
            })?;
            effector.tank_position_q28 = [rcs_source.tank_station_m.raw(28)?, 0, 0];
            effector.tank_dry_mass_q21 = rcs_source.tank_dry_mass_kg.raw(21)?;
            effector.propellant_wet_mass_q21 = rcs_source.propellant_mass_kg.raw(21)?;
            effector.reserve_q15 = u16::try_from(rcs_source.reserve_fraction.raw(15)?)
                .map_err(|_| AdvancedCompileError::Range)?;
        }
        if !effector.is_valid() {
            return Err(AdvancedCompileError::Variant);
        }
        let allocator = compile_allocator(
            set,
            allocator_identity,
            effector_identity,
            variant.include_gimbal,
        );
        if !allocator.is_valid() {
            return Err(AdvancedCompileError::Variant);
        }
        let mut vehicle_bytes = [0u8; KVP8_LENGTH];
        let mut effector_bytes = [0u8; KPE9_LENGTH];
        let mut allocator_bytes = [0u8; KPA9_LENGTH];
        encode_spatial_vehicle_pack(&vehicle, &mut vehicle_bytes)?;
        write_effector_pack(&effector, &mut effector_bytes)?;
        write_allocator_pack(&allocator, &mut allocator_bytes)?;
        compiled.push(CompiledAdvancedVariant {
            name: variant.name.clone(),
            vehicle: vehicle_bytes,
            effector: effector_bytes,
            allocator: allocator_bytes,
            report: AdvancedCompileReport {
                name: variant.name.clone(),
                experimental: variant.experimental,
                base_vehicle_identity: base.identity,
                vehicle_identity,
                effector_identity,
                allocator_identity,
                dry_mass_kg: dry_mass,
                dry_cg_from_nose_m: cg,
                dry_inertia_kgm2: inertia,
                canard_count: effector.canard_count,
                jet_count: effector.jet_count,
                supply_source: variant.supply.clone(),
                provenance_identity,
            },
        });
    }
    Ok(CompiledAdvancedSet { variants: compiled })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase9_5_contract::{parse_allocator_pack, parse_effector_pack};
    #[test]
    fn reference_sources_reconstruct_four_strict_derivatives() {
        let result = compile_advanced_sources(
            include_bytes!("../../phase8/examples/firestorm54.kvp8"),
            include_bytes!("../../phase9_5/source-data/advanced-effectors-v1.json"),
        )
        .unwrap();
        assert_eq!(result.variants.len(), 4);
        for variant in &result.variants {
            let vehicle = parse_spatial_vehicle_pack(&variant.vehicle).unwrap();
            let effector = parse_effector_pack(&variant.effector).unwrap();
            let allocator = parse_allocator_pack(&variant.allocator).unwrap();
            assert_eq!(vehicle.identity, effector.vehicle_identity);
            assert_eq!(effector.identity, allocator.effector_identity);
            assert!(variant.report.dry_mass_kg > 2.1);
            assert!(
                variant.report.dry_cg_from_nose_m > 0.7 && variant.report.dry_cg_from_nose_m < 1.4
            );
        }
        assert!(!result.variants[0].report.experimental);
        assert!(result.variants[3].report.experimental);
    }
    #[test]
    fn provenance_and_identity_fail_closed() {
        let invalid = br#"{"schema":"ksa64.advanced-effector-source-v1"}"#;
        assert!(compile_advanced_sources(
            include_bytes!("../../phase8/examples/firestorm54.kvp8"),
            invalid
        )
        .is_err());
    }
}
