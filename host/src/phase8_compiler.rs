//! Offline Phase 8 compiler from provenance-bearing JSON to bounded target packs.

use ksa64_core::phase8_format::{
    KMC8_LENGTH, KMP8_LENGTH, KMP8_MAX_KNOTS, KVP8_LENGTH, KVP8_MAX_AERO_KNOTS, KWP8_LENGTH,
    KWP8_MAX_WIND_KNOTS,
};
use ksa64_core::phase8_numeric::*;
use ksa64_core::phase8_pack::{
    encode_spatial_mission_pack, encode_spatial_motor_pack, encode_spatial_vehicle_pack,
    encode_wind_profile_pack, AeroKnot, Phase8PackError, SpatialMissionPack, SpatialMotorKnot,
    SpatialMotorPack, SpatialVehiclePack, WindKnot, WindProfilePack,
};
use ksa64_core::scenario::{crc32_ieee, fnv1a_32};
use serde::{Deserialize, Serialize};

use crate::phase7_compiler::{identity, parse_scaled};

#[derive(Debug)]
pub enum CompileError {
    Json(serde_json::Error),
    Schema,
    Profile,
    Provenance,
    Decimal,
    Range,
    Count,
    Ordering,
    Overlap,
    Identity,
    Pack(Phase8PackError),
}

impl From<serde_json::Error> for CompileError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
impl From<Phase8PackError> for CompileError {
    fn from(value: Phase8PackError) -> Self {
        Self::Pack(value)
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
    fn validate(&self) -> Result<(), CompileError> {
        if self.note.trim().is_empty() {
            return Err(CompileError::Provenance);
        }
        match self.kind {
            ProvenanceKind::Published | ProvenanceKind::Measured
                if self.reference.trim().is_empty() =>
            {
                Err(CompileError::Provenance)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct SourcedDecimal {
    value: String,
    provenance: Provenance,
}

impl SourcedDecimal {
    fn number(&self) -> Result<f64, CompileError> {
        self.provenance.validate()?;
        if self.value.trim().contains(['e', 'E', '+']) {
            return Err(CompileError::Decimal);
        }
        let value: f64 = self.value.parse().map_err(|_| CompileError::Decimal)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(CompileError::Decimal)
        }
    }
    fn raw(&self, bits: u8) -> Result<i32, CompileError> {
        self.provenance.validate()?;
        parse_scaled(&self.value, bits).map_err(|_| CompileError::Decimal)
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MassComponentKind {
    NoseCone,
    BodyTube,
    Transition,
    FinSet,
    InternalMass,
    PointMass,
    RecoveryDevice,
    MotorMount,
    InstalledMotor,
    FinCan,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MassPrimitive {
    Point,
    SolidCylinder,
    ThinShell,
    Rod,
    Disk,
    Cone,
}

#[derive(Clone, Debug, Deserialize)]
struct MassComponentSource {
    name: String,
    kind: MassComponentKind,
    primitive: MassPrimitive,
    mass_kg: SourcedDecimal,
    x_from_nose_m: SourcedDecimal,
    length_m: SourcedDecimal,
    radius_m: SourcedDecimal,
    #[serde(default)]
    exclusive_group: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RailGuideSource {
    name: String,
    from_tail_m: SourcedDecimal,
}

#[derive(Clone, Debug, Deserialize)]
struct AeroSeedKnotSource {
    mach: SourcedDecimal,
    axial_cd: SourcedDecimal,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NoseShape {
    Conical,
    TangentOgive,
    Elliptical,
}

#[derive(Clone, Debug, Deserialize)]
struct NoseGeometrySource {
    shape: NoseShape,
    length_m: SourcedDecimal,
    base_diameter_m: SourcedDecimal,
}

#[derive(Clone, Debug, Deserialize)]
struct FinSetGeometrySource {
    count: u8,
    root_chord_m: SourcedDecimal,
    tip_chord_m: SourcedDecimal,
    span_m: SourcedDecimal,
    leading_edge_sweep_m: SourcedDecimal,
    leading_edge_from_nose_m: SourcedDecimal,
    thickness_m: SourcedDecimal,
}

#[derive(Clone, Debug, Deserialize)]
struct TransitionGeometrySource {
    fore_diameter_m: SourcedDecimal,
    aft_diameter_m: SourcedDecimal,
    length_m: SourcedDecimal,
    fore_station_m: SourcedDecimal,
}

#[derive(Deserialize)]
struct VehicleSource {
    schema: String,
    identity: String,
    profile: String,
    length_m: SourcedDecimal,
    declared_dry_mass_kg: SourcedDecimal,
    diameter_m: SourcedDecimal,
    components: Vec<MassComponentSource>,
    nose: NoseGeometrySource,
    fin_sets: Vec<FinSetGeometrySource>,
    #[serde(default)]
    transitions: Vec<TransitionGeometrySource>,
    rail_guides: Vec<RailGuideSource>,
    motor_aft_from_tail_m: SourcedDecimal,
    drogue_cda_m2: SourcedDecimal,
    main_cda_m2: SourcedDecimal,
    qualified_cp_reference_m: SourcedDecimal,
    aero_seed: Vec<AeroSeedKnotSource>,
}

#[derive(Deserialize)]
struct CurveSource {
    provenance: Provenance,
    knots: Vec<[String; 2]>,
}

#[derive(Deserialize)]
struct MotorSource {
    schema: String,
    identity: String,
    loaded_mass_kg: SourcedDecimal,
    propellant_mass_kg: SourcedDecimal,
    length_m: SourcedDecimal,
    diameter_m: SourcedDecimal,
    loaded_cg_from_aft_m: SourcedDecimal,
    dry_cg_from_aft_m: SourcedDecimal,
    total_impulse_ns: SourcedDecimal,
    burn_time_s: SourcedDecimal,
    thrust_curve: CurveSource,
}

#[derive(Deserialize)]
struct MissionSource {
    schema: String,
    identity: String,
    vehicle_identity: String,
    motor_identity: String,
    wind_identity: String,
    environment: String,
    launch_altitude_m: SourcedDecimal,
    rail_length_m: SourcedDecimal,
    launch_azimuth_rad: SourcedDecimal,
    launch_elevation_rad: SourcedDecimal,
    main_deployment_altitude_m: SourcedDecimal,
    drogue_inflation_time_s: SourcedDecimal,
    main_inflation_time_s: SourcedDecimal,
    max_mission_time_s: SourcedDecimal,
    telemetry_period_s: SourcedDecimal,
    minimum_rail_exit_velocity_mps: SourcedDecimal,
    case_seed: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct WindKnotSource {
    altitude_m: SourcedDecimal,
    east_mps: SourcedDecimal,
    north_mps: SourcedDecimal,
}

#[derive(Deserialize)]
struct WindSource {
    schema: String,
    identity: String,
    gust_seed: u32,
    gust_cadence_s: SourcedDecimal,
    gust_amplitude_east_mps: SourcedDecimal,
    gust_amplitude_north_mps: SourcedDecimal,
    max_gust_mps: SourcedDecimal,
    knots: Vec<WindKnotSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SpatialCompileReport {
    pub dry_mass_kg: f64,
    pub dry_cg_from_nose_m: f64,
    pub dry_inertia_kgm2: [f64; 3],
    pub reference_area_m2: f64,
    pub loaded_motor_inertia_kgm2: [f64; 2],
    pub derived_cp_from_nose_m: f64,
    pub qualified_cp_reference_m: f64,
    pub normal_force_slope_per_rad: f64,
    pub pitch_yaw_damping: f64,
    pub roll_damping: f64,
    pub dry_static_margin_calibers: f64,
    pub dry_motor_inertia_kgm2: [f64; 2],
    pub source_manifest_identity: u32,
}

pub struct CompiledSpatialPacks {
    pub vehicle: [u8; KVP8_LENGTH],
    pub motor: [u8; KMP8_LENGTH],
    pub mission: [u8; KMC8_LENGTH],
    pub wind: [u8; KWP8_LENGTH],
    pub report: SpatialCompileReport,
}

fn quantize(value: f64, bits: u8) -> Result<i32, CompileError> {
    if !value.is_finite() {
        return Err(CompileError::Range);
    }
    let scaled = (value * (1u64 << bits) as f64).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        Err(CompileError::Range)
    } else {
        Ok(scaled as i32)
    }
}

fn own_inertia(primitive: MassPrimitive, mass: f64, length: f64, radius: f64) -> [f64; 3] {
    let r2 = radius * radius;
    let l2 = length * length;
    let (axial, transverse) = match primitive {
        MassPrimitive::Point => (0.0, 0.0),
        MassPrimitive::SolidCylinder => (0.5 * mass * r2, mass * (3.0 * r2 + l2) / 12.0),
        MassPrimitive::ThinShell => (mass * r2, mass * (6.0 * r2 + l2) / 12.0),
        MassPrimitive::Rod => (0.0, mass * l2 / 12.0),
        MassPrimitive::Disk => (0.5 * mass * r2, 0.25 * mass * r2),
        MassPrimitive::Cone => (0.3 * mass * r2, mass * (0.15 * r2 + 0.6 * l2)),
    };
    [axial, transverse, transverse]
}

#[derive(Clone, Copy, Debug)]
struct DerivedAerodynamics {
    cp_from_nose_m: f64,
    normal_force_slope_per_rad: f64,
    pitch_yaw_damping: f64,
    roll_damping: f64,
    dry_static_margin_calibers: f64,
}

fn derive_geometry_aerodynamics(
    source: &VehicleSource,
    length: f64,
    diameter: f64,
    dry_cg: f64,
) -> Result<DerivedAerodynamics, CompileError> {
    let nose_length = source.nose.length_m.number()?;
    let nose_cp_factor = match source.nose.shape {
        NoseShape::Conical => 2.0 / 3.0,
        NoseShape::TangentOgive => 0.466,
        NoseShape::Elliptical => 1.0 / 3.0,
    };
    let mut total_slope = 2.0;
    let mut weighted_cp = 2.0 * nose_cp_factor * nose_length;
    let reference_area = core::f64::consts::PI * diameter * diameter / 4.0;
    let mut roll_damping = 0.0;

    for transition in &source.transitions {
        let fore = transition.fore_diameter_m.number()?;
        let aft = transition.aft_diameter_m.number()?;
        let transition_length = transition.length_m.number()?;
        let station = transition.fore_station_m.number()?;
        let slope = 2.0 * (aft * aft - fore * fore) / (diameter * diameter);
        let ratio = fore / aft;
        let denominator = 1.0 - ratio * ratio;
        let cp_fraction = if denominator.abs() < 1e-12 {
            0.5
        } else {
            (1.0 + (1.0 - ratio) / denominator) / 3.0
        };
        total_slope += slope;
        weighted_cp += slope * (station + transition_length * cp_fraction);
    }

    for fins in &source.fin_sets {
        let count = f64::from(fins.count);
        let root = fins.root_chord_m.number()?;
        let tip = fins.tip_chord_m.number()?;
        let span = fins.span_m.number()?;
        let sweep = fins.leading_edge_sweep_m.number()?;
        let station = fins.leading_edge_from_nose_m.number()?;
        let mid_chord_offset = sweep + 0.5 * (tip - root);
        let mid_chord_length = (span * span + mid_chord_offset * mid_chord_offset).sqrt();
        let isolated = 4.0 * count * (span / diameter).powi(2)
            / (1.0 + (1.0 + (2.0 * mid_chord_length / (root + tip)).powi(2)).sqrt());
        let interference = 1.0 + diameter / (2.0 * span + diameter);
        let slope = isolated * interference;
        let cp = station
            + sweep * (root + 2.0 * tip) / (3.0 * (root + tip))
            + (root + tip - root * tip / (root + tip)) / 6.0;
        total_slope += slope;
        weighted_cp += slope * cp;
        let fin_area = 0.5 * (root + tip) * span;
        roll_damping += 0.5 * count * (fin_area / reference_area) * (span / length).powi(2);
    }

    if total_slope <= 0.0 || total_slope > 64.0 {
        return Err(CompileError::Range);
    }
    let cp = weighted_cp / total_slope;
    if cp <= 0.0 || cp >= length {
        return Err(CompileError::Range);
    }
    let static_margin = (cp - dry_cg) / diameter;
    let pitch_yaw_damping = total_slope * ((cp - dry_cg) / length).abs() * diameter / length;
    if pitch_yaw_damping > 64.0 || roll_damping > 64.0 {
        return Err(CompileError::Range);
    }
    Ok(DerivedAerodynamics {
        cp_from_nose_m: cp,
        normal_force_slope_per_rad: total_slope,
        pitch_yaw_damping,
        roll_damping,
        dry_static_margin_calibers: static_margin,
    })
}
fn derive_vehicle(
    source: &VehicleSource,
    source_bytes: &[u8],
) -> Result<(SpatialVehiclePack, SpatialCompileReport), CompileError> {
    if source.schema != "ksa64.spatial-vehicle-source-v1" {
        return Err(CompileError::Schema);
    }
    if source.profile != "HobbySpatialV1" {
        return Err(CompileError::Profile);
    }
    let length = source.length_m.number()?;
    let diameter = source.diameter_m.number()?;
    if length <= 0.0 || diameter <= 0.0 || diameter >= length || source.components.is_empty() {
        return Err(CompileError::Range);
    }

    let mut values = Vec::with_capacity(source.components.len());
    let mut total_mass = 0.0;
    let mut first_moment = 0.0;
    for component in &source.components {
        if component.name.trim().is_empty() {
            return Err(CompileError::Schema);
        }
        let _component_kind = component.kind;
        let mass = component.mass_kg.number()?;
        let x = component.x_from_nose_m.number()?;
        let component_length = component.length_m.number()?;
        let radius = component.radius_m.number()?;
        if mass <= 0.0
            || component_length < 0.0
            || radius < 0.0
            || x < 0.0
            || x > length
            || x - component_length / 2.0 < -1e-9
            || x + component_length / 2.0 > length + 1e-9
        {
            return Err(CompileError::Range);
        }
        values.push((component, mass, x, component_length, radius));
        total_mass += mass;
        first_moment += mass * x;
    }
    for (index, (left, _, lx, ll, _)) in values.iter().enumerate() {
        if left.exclusive_group.is_empty() {
            continue;
        }
        for (right, _, rx, rl, _) in values.iter().skip(index + 1) {
            if left.exclusive_group == right.exclusive_group
                && (lx - rx).abs() < (ll + rl) / 2.0 - 1e-9
            {
                return Err(CompileError::Overlap);
            }
        }
    }
    let declared_mass = source.declared_dry_mass_kg.number()?;
    if declared_mass <= 0.0 || ((total_mass - declared_mass) / declared_mass).abs() > 0.005 {
        return Err(CompileError::Range);
    }
    let nose_length = source.nose.length_m.number()?;
    let nose_diameter = source.nose.base_diameter_m.number()?;
    let _nose_shape = &source.nose.shape;
    if nose_length <= 0.0 || nose_diameter <= 0.0 || nose_diameter > diameter * 1.001 {
        return Err(CompileError::Range);
    }
    if source.fin_sets.is_empty() {
        return Err(CompileError::Count);
    }
    for fins in &source.fin_sets {
        if fins.count < 3
            || fins.root_chord_m.number()? <= 0.0
            || fins.tip_chord_m.number()? <= 0.0
            || fins.span_m.number()? <= 0.0
            || fins.leading_edge_sweep_m.number()? < 0.0
            || fins.leading_edge_from_nose_m.number()? < 0.0
            || fins.thickness_m.number()? <= 0.0
        {
            return Err(CompileError::Range);
        }
    }
    for transition in &source.transitions {
        if transition.fore_diameter_m.number()? <= 0.0
            || transition.aft_diameter_m.number()? <= 0.0
            || transition.length_m.number()? <= 0.0
            || transition.fore_station_m.number()? < 0.0
        {
            return Err(CompileError::Range);
        }
    }
    let cg = first_moment / total_mass;
    let mut inertia = [0.0; 3];
    for (component, mass, x, component_length, radius) in &values {
        let own = own_inertia(component.primitive, *mass, *component_length, *radius);
        inertia[0] += own[0];
        let parallel = mass * (x - cg) * (x - cg);
        inertia[1] += own[1] + parallel;
        inertia[2] += own[2] + parallel;
    }

    if source.rail_guides.len() != 2
        || source.aero_seed.len() < 2
        || source.aero_seed.len() > KVP8_MAX_AERO_KNOTS
    {
        return Err(CompileError::Count);
    }
    for guide in &source.rail_guides {
        if guide.name.trim().is_empty() {
            return Err(CompileError::Schema);
        }
    }
    let mut guides = [
        source.rail_guides[0].from_tail_m.number()?,
        source.rail_guides[1].from_tail_m.number()?,
    ];
    guides.sort_by(f64::total_cmp);

    let derived_aero = derive_geometry_aerodynamics(source, length, diameter, cg)?;
    let qualified_cp = source.qualified_cp_reference_m.number()?;
    if (derived_aero.cp_from_nose_m - qualified_cp).abs() > 0.5 * diameter {
        return Err(CompileError::Range);
    }
    let mut aero_knots = [AeroKnot::ZERO; KVP8_MAX_AERO_KNOTS];
    for (index, knot) in source.aero_seed.iter().enumerate() {
        aero_knots[index] = AeroKnot {
            mach: SpatialCoefficient::from_raw(knot.mach.raw(SPATIAL_COEFFICIENT_FRACTIONAL_BITS)?),
            axial_cd: SpatialCoefficient::from_raw(
                knot.axial_cd.raw(SPATIAL_COEFFICIENT_FRACTIONAL_BITS)?,
            ),
            cp_from_nose: SpatialMomentArm::from_raw(quantize(
                derived_aero.cp_from_nose_m,
                SPATIAL_MOMENT_ARM_FRACTIONAL_BITS,
            )?),
            normal_force_slope: SpatialCoefficient::from_raw(quantize(
                derived_aero.normal_force_slope_per_rad,
                SPATIAL_COEFFICIENT_FRACTIONAL_BITS,
            )?),
        };
    }
    let reference_area = core::f64::consts::PI * diameter * diameter / 4.0;
    let manifest_identity = fnv1a_32(source_bytes);
    if manifest_identity == 0 {
        return Err(CompileError::Identity);
    }
    let vehicle = SpatialVehiclePack {
        identity: identity(&source.identity).map_err(|_| CompileError::Identity)?,
        dry_mass: SpatialMass::from_raw(quantize(total_mass, SPATIAL_MASS_FRACTIONAL_BITS)?),
        length: SpatialPosition::from_raw(source.length_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?),
        diameter: SpatialPosition::from_raw(
            source.diameter_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
        ),
        reference_area: SpatialArea::from_raw(quantize(
            reference_area,
            SPATIAL_AREA_FRACTIONAL_BITS,
        )?),
        dry_cg_from_nose: SpatialMomentArm::from_raw(quantize(
            cg,
            SPATIAL_MOMENT_ARM_FRACTIONAL_BITS,
        )?),
        dry_inertia: [
            SpatialInertia::from_raw(quantize(inertia[0], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
            SpatialInertia::from_raw(quantize(inertia[1], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
            SpatialInertia::from_raw(quantize(inertia[2], SPATIAL_INERTIA_FRACTIONAL_BITS)?),
        ],
        motor_aft_from_tail: SpatialPosition::from_raw(
            source
                .motor_aft_from_tail_m
                .raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
        ),
        aft_rail_guide_from_tail: SpatialPosition::from_raw(quantize(
            guides[0],
            SPATIAL_POSITION_FRACTIONAL_BITS,
        )?),
        forward_rail_guide_from_tail: SpatialPosition::from_raw(quantize(
            guides[1],
            SPATIAL_POSITION_FRACTIONAL_BITS,
        )?),
        drogue_cda: SpatialArea::from_raw(source.drogue_cda_m2.raw(SPATIAL_AREA_FRACTIONAL_BITS)?),
        main_cda: SpatialArea::from_raw(source.main_cda_m2.raw(SPATIAL_AREA_FRACTIONAL_BITS)?),
        pitch_damping: SpatialCoefficient::from_raw(quantize(
            derived_aero.pitch_yaw_damping,
            SPATIAL_COEFFICIENT_FRACTIONAL_BITS,
        )?),
        yaw_damping: SpatialCoefficient::from_raw(quantize(
            derived_aero.pitch_yaw_damping,
            SPATIAL_COEFFICIENT_FRACTIONAL_BITS,
        )?),
        roll_damping: SpatialCoefficient::from_raw(quantize(
            derived_aero.roll_damping,
            SPATIAL_COEFFICIENT_FRACTIONAL_BITS,
        )?),
        source_manifest_identity: manifest_identity,
        aero_knot_count: source.aero_seed.len() as u8,
        aero_knots,
    };
    Ok((
        vehicle,
        SpatialCompileReport {
            dry_mass_kg: total_mass,
            dry_cg_from_nose_m: cg,
            dry_inertia_kgm2: inertia,
            reference_area_m2: reference_area,
            loaded_motor_inertia_kgm2: [0.0; 2],
            derived_cp_from_nose_m: derived_aero.cp_from_nose_m,
            qualified_cp_reference_m: qualified_cp,
            normal_force_slope_per_rad: derived_aero.normal_force_slope_per_rad,
            pitch_yaw_damping: derived_aero.pitch_yaw_damping,
            roll_damping: derived_aero.roll_damping,
            dry_static_margin_calibers: derived_aero.dry_static_margin_calibers,
            dry_motor_inertia_kgm2: [0.0; 2],
            source_manifest_identity: manifest_identity,
        },
    ))
}

fn derive_motor(
    source: &MotorSource,
) -> Result<(SpatialMotorPack, [f64; 2], [f64; 2]), CompileError> {
    if source.schema != "ksa64.spatial-motor-source-v1" {
        return Err(CompileError::Schema);
    }
    let loaded_mass = source.loaded_mass_kg.number()?;
    let propellant_mass = source.propellant_mass_kg.number()?;
    let length = source.length_m.number()?;
    let diameter = source.diameter_m.number()?;
    if loaded_mass <= propellant_mass || propellant_mass <= 0.0 || length <= 0.0 || diameter <= 0.0
    {
        return Err(CompileError::Range);
    }
    source.thrust_curve.provenance.validate()?;
    if source.thrust_curve.knots.len() < 2 || source.thrust_curve.knots.len() > KMP8_MAX_KNOTS {
        return Err(CompileError::Count);
    }
    let radius = diameter / 2.0;
    let dry_mass = loaded_mass - propellant_mass;
    let loaded_i = own_inertia(MassPrimitive::SolidCylinder, loaded_mass, length, radius);
    let dry_i = own_inertia(MassPrimitive::SolidCylinder, dry_mass, length, radius);
    let mut knots = [SpatialMotorKnot::ZERO; KMP8_MAX_KNOTS];
    for (index, knot) in source.thrust_curve.knots.iter().enumerate() {
        knots[index] = SpatialMotorKnot {
            time: SpatialTime::from_raw(
                parse_scaled(&knot[0], SPATIAL_TIME_FRACTIONAL_BITS)
                    .map_err(|_| CompileError::Decimal)?,
            ),
            thrust_raw_q13: parse_scaled(&knot[1], SPATIAL_FORCE_FRACTIONAL_BITS)
                .map_err(|_| CompileError::Decimal)?,
        };
    }
    Ok((
        SpatialMotorPack {
            identity: identity(&source.identity).map_err(|_| CompileError::Identity)?,
            loaded_mass: SpatialMass::from_raw(
                source.loaded_mass_kg.raw(SPATIAL_MASS_FRACTIONAL_BITS)?,
            ),
            propellant_mass: SpatialMass::from_raw(
                source
                    .propellant_mass_kg
                    .raw(SPATIAL_MASS_FRACTIONAL_BITS)?,
            ),
            length: SpatialPosition::from_raw(
                source.length_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
            ),
            diameter: SpatialPosition::from_raw(
                source.diameter_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
            ),
            loaded_cg_from_aft: SpatialMomentArm::from_raw(
                source
                    .loaded_cg_from_aft_m
                    .raw(SPATIAL_MOMENT_ARM_FRACTIONAL_BITS)?,
            ),
            dry_cg_from_aft: SpatialMomentArm::from_raw(
                source
                    .dry_cg_from_aft_m
                    .raw(SPATIAL_MOMENT_ARM_FRACTIONAL_BITS)?,
            ),
            loaded_axial_inertia: SpatialInertia::from_raw(quantize(
                loaded_i[0],
                SPATIAL_INERTIA_FRACTIONAL_BITS,
            )?),
            loaded_transverse_inertia: SpatialInertia::from_raw(quantize(
                loaded_i[1],
                SPATIAL_INERTIA_FRACTIONAL_BITS,
            )?),
            dry_axial_inertia: SpatialInertia::from_raw(quantize(
                dry_i[0],
                SPATIAL_INERTIA_FRACTIONAL_BITS,
            )?),
            dry_transverse_inertia: SpatialInertia::from_raw(quantize(
                dry_i[1],
                SPATIAL_INERTIA_FRACTIONAL_BITS,
            )?),
            total_impulse_raw_q16: source.total_impulse_ns.raw(16)?,
            burn_time: SpatialTime::from_raw(source.burn_time_s.raw(SPATIAL_TIME_FRACTIONAL_BITS)?),
            knot_count: source.thrust_curve.knots.len() as u8,
            knots,
        },
        [loaded_i[0], loaded_i[1]],
        [dry_i[0], dry_i[1]],
    ))
}

fn compile_wind(source: &WindSource) -> Result<WindProfilePack, CompileError> {
    if source.schema != "ksa64.wind-source-v1"
        || source.knots.is_empty()
        || source.knots.len() > KWP8_MAX_WIND_KNOTS
    {
        return Err(CompileError::Schema);
    }
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    for (index, knot) in source.knots.iter().enumerate() {
        knots[index] = WindKnot {
            altitude: SpatialPosition::from_raw(
                knot.altitude_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
            ),
            east: SpatialWind::from_raw(knot.east_mps.raw(SPATIAL_WIND_FRACTIONAL_BITS)?),
            north: SpatialWind::from_raw(knot.north_mps.raw(SPATIAL_WIND_FRACTIONAL_BITS)?),
        };
    }
    Ok(WindProfilePack {
        identity: identity(&source.identity).map_err(|_| CompileError::Identity)?,
        gust_seed: source.gust_seed,
        gust_cadence: SpatialTime::from_raw(
            source.gust_cadence_s.raw(SPATIAL_TIME_FRACTIONAL_BITS)?,
        ),
        gust_amplitude_east: SpatialWind::from_raw(
            source
                .gust_amplitude_east_mps
                .raw(SPATIAL_WIND_FRACTIONAL_BITS)?,
        ),
        gust_amplitude_north: SpatialWind::from_raw(
            source
                .gust_amplitude_north_mps
                .raw(SPATIAL_WIND_FRACTIONAL_BITS)?,
        ),
        max_gust: SpatialWind::from_raw(source.max_gust_mps.raw(SPATIAL_WIND_FRACTIONAL_BITS)?),
        knot_count: source.knots.len() as u8,
        knots,
    })
}

fn compile_mission(
    source: &MissionSource,
    vehicle: &VehicleSource,
    motor: &MotorSource,
    wind: &WindSource,
) -> Result<SpatialMissionPack, CompileError> {
    if source.schema != "ksa64.spatial-mission-source-v1" {
        return Err(CompileError::Schema);
    }
    if source.vehicle_identity != vehicle.identity
        || source.motor_identity != motor.identity
        || source.wind_identity != wind.identity
        || identity(&source.environment).map_err(|_| CompileError::Identity)?
            != HOBBY_SPATIAL_ENVIRONMENT_ID
    {
        return Err(CompileError::Identity);
    }
    Ok(SpatialMissionPack {
        identity: identity(&source.identity).map_err(|_| CompileError::Identity)?,
        vehicle_identity: identity(&vehicle.identity).map_err(|_| CompileError::Identity)?,
        motor_identity: identity(&motor.identity).map_err(|_| CompileError::Identity)?,
        wind_identity: identity(&wind.identity).map_err(|_| CompileError::Identity)?,
        environment_identity: HOBBY_SPATIAL_ENVIRONMENT_ID,
        launch_altitude: SpatialPosition::from_raw(
            source
                .launch_altitude_m
                .raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
        ),
        rail_length: SpatialPosition::from_raw(
            source.rail_length_m.raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
        ),
        launch_azimuth: SpatialAngle::from_raw(
            source
                .launch_azimuth_rad
                .raw(SPATIAL_ANGLE_FRACTIONAL_BITS)?,
        ),
        launch_elevation: SpatialAngle::from_raw(
            source
                .launch_elevation_rad
                .raw(SPATIAL_ANGLE_FRACTIONAL_BITS)?,
        ),
        main_deployment_altitude: SpatialPosition::from_raw(
            source
                .main_deployment_altitude_m
                .raw(SPATIAL_POSITION_FRACTIONAL_BITS)?,
        ),
        drogue_inflation_time: SpatialTime::from_raw(
            source
                .drogue_inflation_time_s
                .raw(SPATIAL_TIME_FRACTIONAL_BITS)?,
        ),
        main_inflation_time: SpatialTime::from_raw(
            source
                .main_inflation_time_s
                .raw(SPATIAL_TIME_FRACTIONAL_BITS)?,
        ),
        max_mission_time: SpatialTime::from_raw(
            source
                .max_mission_time_s
                .raw(SPATIAL_TIME_FRACTIONAL_BITS)?,
        ),
        telemetry_period: SpatialTime::from_raw(
            source
                .telemetry_period_s
                .raw(SPATIAL_TIME_FRACTIONAL_BITS)?,
        ),
        minimum_rail_exit_velocity: SpatialVelocity::from_raw(
            source
                .minimum_rail_exit_velocity_mps
                .raw(SPATIAL_VELOCITY_FRACTIONAL_BITS)?,
        ),
        case_seed: source.case_seed,
    })
}

pub fn compile_spatial_sources(
    vehicle_json: &[u8],
    motor_json: &[u8],
    mission_json: &[u8],
    wind_json: &[u8],
) -> Result<CompiledSpatialPacks, CompileError> {
    let vehicle_source: VehicleSource = serde_json::from_slice(vehicle_json)?;
    let motor_source: MotorSource = serde_json::from_slice(motor_json)?;
    let mission_source: MissionSource = serde_json::from_slice(mission_json)?;
    let wind_source: WindSource = serde_json::from_slice(wind_json)?;
    let (vehicle, mut report) = derive_vehicle(&vehicle_source, vehicle_json)?;
    let (motor, loaded_i, dry_i) = derive_motor(&motor_source)?;
    report.loaded_motor_inertia_kgm2 = loaded_i;
    report.dry_motor_inertia_kgm2 = dry_i;
    let wind = compile_wind(&wind_source)?;
    let mission = compile_mission(
        &mission_source,
        &vehicle_source,
        &motor_source,
        &wind_source,
    )?;
    let mut compiled = CompiledSpatialPacks {
        vehicle: [0; KVP8_LENGTH],
        motor: [0; KMP8_LENGTH],
        mission: [0; KMC8_LENGTH],
        wind: [0; KWP8_LENGTH],
        report,
    };
    encode_spatial_vehicle_pack(&vehicle, &mut compiled.vehicle)?;
    encode_spatial_motor_pack(&motor, &mut compiled.motor)?;
    encode_spatial_mission_pack(mission, &mut compiled.mission)?;
    encode_wind_profile_pack(&wind, &mut compiled.wind)?;
    Ok(compiled)
}

pub fn source_set_identity(vehicle: &[u8], motor: &[u8], mission: &[u8], wind: &[u8]) -> u32 {
    let mut state = crc32_ieee(vehicle);
    state = state.rotate_left(7) ^ crc32_ieee(motor);
    state = state.rotate_left(7) ^ crc32_ieee(mission);
    state.rotate_left(7) ^ crc32_ieee(wind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_inertia_fixtures_are_analytic() {
        assert_eq!(own_inertia(MassPrimitive::Point, 2.0, 3.0, 4.0), [0.0; 3]);
        assert_eq!(
            own_inertia(MassPrimitive::SolidCylinder, 12.0, 2.0, 1.0),
            [6.0, 7.0, 7.0]
        );
        assert_eq!(
            own_inertia(MassPrimitive::Rod, 3.0, 2.0, 0.0),
            [0.0, 1.0, 1.0]
        );
    }

    #[test]
    fn phase8_decimal_quantization_is_stable() {
        assert_eq!(quantize(0.01, SPATIAL_TIME_FRACTIONAL_BITS).unwrap(), 2_621);
        assert_eq!(
            quantize(0.5, SPATIAL_AREA_FRACTIONAL_BITS).unwrap(),
            268_435_456
        );
    }
    #[test]
    fn firestorm_sources_reconstruct_bounded_packs() {
        let packs = compile_spatial_sources(
            include_bytes!("../../phase8/source-data/firestorm54-spatial.json"),
            include_bytes!("../../phase8/source-data/aerotech-i211w-spatial.json"),
            include_bytes!("../../phase8/source-data/firestorm-i211-spatial-mission.json"),
            include_bytes!("../../phase8/source-data/calm-wind.json"),
        )
        .unwrap();
        assert!((packs.report.dry_mass_kg - 2.112_039_472_812_5).abs() < 1e-12);
        assert!(packs.report.dry_cg_from_nose_m > 0.8);
        assert!(packs.report.dry_cg_from_nose_m < 1.3);
        assert!(packs
            .report
            .dry_inertia_kgm2
            .iter()
            .all(|value| *value > 0.0));
        assert!((packs.report.reference_area_m2 - 0.002_611_012_969_041_496).abs() < 1e-15);
        assert_eq!(
            ksa64_core::phase8_pack::parse_spatial_vehicle_pack(&packs.vehicle)
                .unwrap()
                .source_manifest_identity,
            packs.report.source_manifest_identity
        );
        assert!(ksa64_core::phase8_pack::parse_spatial_motor_pack(&packs.motor).is_ok());
        assert!(ksa64_core::phase8_pack::parse_spatial_mission_pack(&packs.mission).is_ok());
        assert!(ksa64_core::phase8_pack::parse_wind_profile_pack(&packs.wind).is_ok());
        assert_eq!(
            packs.vehicle,
            *include_bytes!("../../phase8/examples/firestorm54.kvp8")
        );
        assert_eq!(
            packs.motor,
            *include_bytes!("../../phase8/examples/aerotech-i211w.kmp8")
        );
        assert_eq!(
            packs.mission,
            *include_bytes!("../../phase8/examples/firestorm-i211.kmc8")
        );
        assert_eq!(
            packs.wind,
            *include_bytes!("../../phase8/examples/firestorm-calm.kwp8")
        );
    }
    #[test]
    fn missing_provenance_and_inconsistent_mass_fail_closed() {
        let base: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../phase8/source-data/firestorm54-spatial.json"
        ))
        .unwrap();
        let motor = include_bytes!("../../phase8/source-data/aerotech-i211w-spatial.json");
        let mission =
            include_bytes!("../../phase8/source-data/firestorm-i211-spatial-mission.json");
        let wind = include_bytes!("../../phase8/source-data/calm-wind.json");

        let mut missing = base.clone();
        missing["components"][0]["mass_kg"]["provenance"]["note"] = "".into();
        let bytes = serde_json::to_vec(&missing).unwrap();
        assert!(matches!(
            compile_spatial_sources(&bytes, motor, mission, wind),
            Err(CompileError::Provenance)
        ));

        let mut inconsistent = base;
        inconsistent["declared_dry_mass_kg"]["value"] = "3.0".into();
        let bytes = serde_json::to_vec(&inconsistent).unwrap();
        assert!(matches!(
            compile_spatial_sources(&bytes, motor, mission, wind),
            Err(CompileError::Range)
        ));
    }
}
