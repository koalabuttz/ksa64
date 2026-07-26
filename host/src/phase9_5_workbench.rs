//! Deterministic Phase 9.5 campaigns and additive optimization workbench.
use crate::phase9::CandidateEvaluation as LegacyCandidateEvaluation;
use crate::phase9_search::{
    run_search_with_workers, CandidateEvaluator, SearchError, SearchResult,
};
use ksa64_core::evaluation::{EvaluationOutcome, MetricSlot};
use ksa64_core::phase8_format::KVP8_LENGTH;
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_numeric::{SpatialPosition, SpatialWind};
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack, SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack,
    WindProfilePack,
};
use ksa64_core::phase9_5_contract::{
    parse_allocator_pack, parse_effector_pack, write_advanced_campaign_config,
    write_advanced_effector_summary, AdvancedCampaignConfig, AdvancedEffectorEvaluationSummary,
    AdvancedEffectorPack, PriorityResidualAllocatorPack, KAS9_LENGTH, KSC9_LENGTH,
    PHASE95_ACCEPTED_SEED,
};
use ksa64_core::phase9_contract::{
    AggregateId, CandidateAggregate, ConstraintOp, ConstraintSpec, DesignVector, Direction,
    ObjectiveSpec, SearchBudgets, SearchEngineId, SearchManifest, SearchPresetId, VariableKind,
    VariableSpec, MAX_CONSTRAINTS, MAX_CONSTRAINT_RESULTS, MAX_METRICS, MAX_OBJECTIVES,
    MAX_VARIABLES,
};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::keyed_word_raw;
use ksa64_sim::phase8_5::reference_avionics_profile;
use ksa64_sim::phase8_campaign::{
    derive_spatial_uncertainty, materialize_spatial_case, SpatialCampaignConfig,
};
use ksa64_sim::phase9_5_mission::{
    evaluate_with_advanced_effectors, reference_capability, AdvancedEffectorEvaluationRequest,
    AdvancedMissionFaults,
};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

pub const ADVANCED_CAMPAIGN_RUNS: u32 = 64;
pub const ADVANCED_EVALUATOR_ID: u32 = 0x0959_5001;
pub const CANARD_STUDY_ID: u32 = 0x0959_c001;
pub const RCS_STUDY_ID: u32 = 0x0959_c002;
pub const MIXED_STUDY_ID: u32 = 0x0959_c003;
pub const RESEARCH_STUDY_ID: u32 = 0x0959_c004;

pub mod variable {
    pub const CANARD_CONTROL_SCALE: u16 = 1001;
    pub const CANARD_DRAG_SCALE: u16 = 1002;
    pub const CANARD_TRAVEL_TURN16: u16 = 1003;
    pub const CANARD_SLEW_TURN16: u16 = 1004;
    pub const CANARD_MASS_SCALE: u16 = 1005;
    pub const RCS_THRUST_SCALE: u16 = 1010;
    pub const RCS_PROPELLANT_SCALE: u16 = 1011;
    pub const RCS_RESERVE_Q15: u16 = 1012;
    pub const ROLL_KP_Q15: u16 = 1020;
    pub const ROLL_KD_Q15: u16 = 1021;
}
pub mod metric {
    pub const APOGEE: u16 = 2001;
    pub const LANDING_DISTANCE: u16 = 2002;
    pub const RAIL_EXIT_VELOCITY: u16 = 2003;
    pub const MIN_STATIC_MARGIN: u16 = 2004;
    pub const IMPACT_VELOCITY: u16 = 2005;
    pub const ATTITUDE_ERROR: u16 = 2006;
    pub const SYSTEM_MASS: u16 = 2007;
    pub const HINGE_LOAD: u16 = 2008;
    pub const SATURATION_COUNT: u16 = 2009;
    pub const RCS_USE: u16 = 2010;
    pub const PULSE_COUNT: u16 = 2011;
    pub const RESIDUAL_TORQUE: u16 = 2012;
    pub const DEPLOYMENT_ACK: u16 = 2013;
    pub const ALARMS: u16 = 2014;
    pub const RESERVE_MARGIN: u16 = 2015;
    pub const AUTHORITY_HANDOFFS: u16 = 2016;
    pub const RAIL_SETTLE_ERROR: u16 = 2017;
    pub const DISTURBANCE_SETTLE_ERROR: u16 = 2018;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedStudyId {
    Canard,
    Rcs,
    Mixed,
    Research,
}
impl AdvancedStudyId {
    pub const fn raw(self) -> u32 {
        match self {
            Self::Canard => CANARD_STUDY_ID,
            Self::Rcs => RCS_STUDY_ID,
            Self::Mixed => MIXED_STUDY_ID,
            Self::Research => RESEARCH_STUDY_ID,
        }
    }
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Canard => "firestorm-c9",
            Self::Rcs => "firestorm-r9",
            Self::Mixed => "firestorm-m9",
            Self::Research => "ksa-x1",
        }
    }
    pub const fn has_rcs(self) -> bool {
        !matches!(self, Self::Canard)
    }
    pub const fn experimental(self) -> bool {
        matches!(self, Self::Research)
    }
}

#[derive(Clone)]
pub struct AdvancedReference {
    pub vehicle: SpatialVehiclePack,
    pub motor: SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: WindProfilePack,
    pub effectors: AdvancedEffectorPack,
    pub allocator: PriorityResidualAllocatorPack,
}
fn load_reference(study: AdvancedStudyId) -> AdvancedReference {
    let (v, e, a): (&[u8; KVP8_LENGTH], &[u8; 2048], &[u8; 512]) = match study {
        AdvancedStudyId::Canard => (
            include_bytes!("../../phase9_5/examples/firestorm-c9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-c9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-c9.kpa9"),
        ),
        AdvancedStudyId::Rcs => (
            include_bytes!("../../phase9_5/examples/firestorm-r9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-r9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-r9.kpa9"),
        ),
        AdvancedStudyId::Mixed => (
            include_bytes!("../../phase9_5/examples/firestorm-m9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpa9"),
        ),
        AdvancedStudyId::Research => (
            include_bytes!("../../phase9_5/examples/ksa-x1.kvp8"),
            include_bytes!("../../phase9_5/examples/ksa-x1.kpe9"),
            include_bytes!("../../phase9_5/examples/ksa-x1.kpa9"),
        ),
    };
    let vehicle = parse_spatial_vehicle_pack(v).expect("checked-in KVP8");
    let motor =
        parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
            .expect("checked-in KMP8");
    let mut mission =
        parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
            .expect("checked-in KMC8");
    mission.identity ^= mission.vehicle_identity ^ vehicle.identity;
    mission.vehicle_identity = vehicle.identity;
    AdvancedReference {
        vehicle,
        motor,
        mission,
        wind: parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
            .expect("checked-in KWP8"),
        effectors: parse_effector_pack(e).expect("checked-in KPE9"),
        allocator: parse_allocator_pack(a).expect("checked-in KPA9"),
    }
}

fn scale(raw: i32, ppm: i32) -> i32 {
    ((i64::from(raw) * i64::from(ppm)) / 1_000_000).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
        as i32
}
fn uniform(word: u32, minimum: i32, maximum: i32) -> i32 {
    minimum
        + (((u64::from(word) * (i64::from(maximum) - i64::from(minimum) + 1) as u64) >> 32) as i32)
}
fn draw(run: u32, id: u8, minimum: i32, maximum: i32) -> i32 {
    uniform(
        keyed_word_raw(PHASE95_ACCEPTED_SEED, run, id, 0, 0),
        minimum,
        maximum,
    )
}
fn value_for(m: &SearchManifest, d: &DesignVector, id: u16, default: i32) -> i32 {
    (0..d.value_count as usize)
        .find_map(|i| (m.variables[i].id == id).then_some(d.values[i]))
        .unwrap_or(default)
}
fn var(id: u16, min: i32, max: i32, quantum: u32) -> VariableSpec {
    VariableSpec {
        id,
        kind: VariableKind::Fixed,
        flags: 0,
        minimum: min,
        maximum: max,
        quantum,
        catalogue_id: 0,
    }
}
fn obj(id: u16, a: AggregateId, d: Direction) -> ObjectiveSpec {
    ObjectiveSpec {
        metric_id: id,
        aggregate: a,
        direction: d,
    }
}
fn con(id: u16, a: AggregateId, op: ConstraintOp, t: i32, s: u32) -> ConstraintSpec {
    ConstraintSpec {
        metric_id: id,
        aggregate: a,
        op,
        threshold: t,
        scale: s,
    }
}

pub fn built_in_advanced_manifest(
    study: AdvancedStudyId,
    engine: SearchEngineId,
) -> SearchManifest {
    let reference = load_reference(study);
    let preset = if matches!(study, AdvancedStudyId::Canard | AdvancedStudyId::Rcs) {
        SearchPresetId::AcceptedBalanced
    } else {
        SearchPresetId::Routine
    };
    let mut variables = [VariableSpec::EMPTY; MAX_VARIABLES];
    let mut objectives = [ObjectiveSpec::EMPTY; MAX_OBJECTIVES];
    let mut constraints = [ConstraintSpec::EMPTY; MAX_CONSTRAINTS];
    let (vc, oc) = match study {
        AdvancedStudyId::Canard => {
            variables[0] = var(variable::CANARD_CONTROL_SCALE, 750_000, 1_250_000, 25_000);
            variables[1] = var(variable::CANARD_TRAVEL_TURN16, 910, 2730, 91);
            variables[2] = var(variable::CANARD_DRAG_SCALE, 750_000, 1_250_000, 25_000);
            variables[3] = var(variable::CANARD_SLEW_TURN16, 341, 1366, 41);
            variables[4] = var(variable::CANARD_MASS_SCALE, 750_000, 1_250_000, 25_000);
            variables[5] = var(variable::ROLL_KP_Q15, 6_000, 24_000, 500);
            variables[6] = var(variable::ROLL_KD_Q15, 1_024, 12_000, 256);
            objectives[0] = obj(
                metric::ATTITUDE_ERROR,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[1] = obj(metric::APOGEE, AggregateId::Mean, Direction::Maximize);
            objectives[2] = obj(
                metric::SYSTEM_MASS,
                AggregateId::Maximum,
                Direction::Minimize,
            );
            objectives[3] = obj(
                metric::HINGE_LOAD,
                AggregateId::Maximum,
                Direction::Minimize,
            );
            objectives[4] = obj(
                metric::SATURATION_COUNT,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            (7, 5)
        }
        AdvancedStudyId::Rcs => {
            variables[0] = var(variable::RCS_THRUST_SCALE, 500_000, 1_500_000, 50_000);
            variables[1] = var(variable::RCS_PROPELLANT_SCALE, 750_000, 1_500_000, 25_000);
            variables[2] = var(variable::RCS_RESERVE_Q15, 3_277, 9_830, 328);
            variables[3] = var(variable::ROLL_KP_Q15, 6_000, 24_000, 500);
            variables[4] = var(variable::ROLL_KD_Q15, 1_024, 12_000, 256);
            objectives[0] = obj(
                metric::ATTITUDE_ERROR,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[1] = obj(
                metric::RCS_USE,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[2] = obj(
                metric::SYSTEM_MASS,
                AggregateId::Maximum,
                Direction::Minimize,
            );
            objectives[3] = obj(
                metric::PULSE_COUNT,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            (5, 4)
        }
        AdvancedStudyId::Mixed | AdvancedStudyId::Research => {
            variables[0] = var(variable::CANARD_CONTROL_SCALE, 750_000, 1_250_000, 25_000);
            variables[1] = var(variable::RCS_THRUST_SCALE, 500_000, 1_500_000, 50_000);
            variables[2] = var(variable::CANARD_TRAVEL_TURN16, 910, 2730, 91);
            variables[3] = var(variable::RCS_PROPELLANT_SCALE, 750_000, 1_500_000, 25_000);
            variables[4] = var(variable::RCS_RESERVE_Q15, 3_277, 9_830, 328);
            variables[5] = var(variable::ROLL_KP_Q15, 6_000, 24_000, 500);
            variables[6] = var(variable::ROLL_KD_Q15, 1_024, 12_000, 256);
            objectives[0] = obj(
                metric::ATTITUDE_ERROR,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[1] = obj(metric::APOGEE, AggregateId::Mean, Direction::Maximize);
            objectives[2] = obj(
                metric::SYSTEM_MASS,
                AggregateId::Maximum,
                Direction::Minimize,
            );
            objectives[3] = obj(
                metric::RCS_USE,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[4] = obj(
                metric::RESIDUAL_TORQUE,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            (7, 5)
        }
    };
    let mut cc = 0usize;
    for c in [
        con(
            metric::RAIL_EXIT_VELOCITY,
            AggregateId::Minimum,
            ConstraintOp::AtLeast,
            15 << 19,
            1 << 19,
        ),
        con(
            metric::MIN_STATIC_MARGIN,
            AggregateId::Minimum,
            ConstraintOp::AtLeast,
            1 << 24,
            1 << 24,
        ),
        con(
            metric::IMPACT_VELOCITY,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            8 << 19,
            1 << 19,
        ),
        con(
            metric::DEPLOYMENT_ACK,
            AggregateId::Minimum,
            ConstraintOp::AtLeast,
            1,
            1,
        ),
        con(
            metric::ALARMS,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            0,
            1,
        ),
    ] {
        constraints[cc] = c;
        cc += 1;
    }
    if !matches!(study, AdvancedStudyId::Rcs) {
        constraints[cc] = con(
            metric::HINGE_LOAD,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            1 << 23,
            1 << 23,
        );
        cc += 1;
    }
    if study.has_rcs() {
        constraints[cc] = con(
            metric::RESERVE_MARGIN,
            AggregateId::Minimum,
            ConstraintOp::AtLeast,
            0,
            1 << 16,
        );
        cc += 1;
    }
    if matches!(study, AdvancedStudyId::Canard) {
        constraints[cc] = con(
            metric::RAIL_SETTLE_ERROR,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            546,
            182,
        );
        cc += 1;
    }
    if matches!(study, AdvancedStudyId::Rcs) {
        constraints[cc] = con(
            metric::DISTURBANCE_SETTLE_ERROR,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            364,
            182,
        );
        cc += 1;
    }
    if matches!(study, AdvancedStudyId::Mixed | AdvancedStudyId::Research) {
        constraints[cc] = con(
            metric::RESIDUAL_TORQUE,
            AggregateId::Maximum,
            ConstraintOp::AtMost,
            16_384,
            4_096,
        );
        cc += 1;
    }
    let mut budgets = SearchBudgets::for_preset(preset);
    if study.experimental() {
        budgets.finalists = 0;
    }
    SearchManifest {
        identity: 0,
        base_ids: [
            reference.vehicle.identity,
            reference.motor.identity,
            reference.mission.identity,
            reference.wind.identity,
            reference.effectors.identity,
            reference.allocator.identity,
            study.raw(),
            0,
        ],
        engine,
        preset,
        master_seed: PHASE95_ACCEPTED_SEED,
        budgets,
        variable_count: vc,
        objective_count: oc,
        constraint_count: cc as u8,
        variables,
        objectives,
        constraints,
    }
    .seal()
    .expect("advanced manifest")
}

#[allow(clippy::needless_range_loop)]
pub fn baseline_advanced_vector(m: &SearchManifest) -> DesignVector {
    let mut values = [0i32; 32];
    for i in 0..m.variable_count as usize {
        values[i] = match m.variables[i].id {
            variable::CANARD_CONTROL_SCALE
            | variable::CANARD_DRAG_SCALE
            | variable::CANARD_MASS_SCALE
            | variable::RCS_THRUST_SCALE
            | variable::RCS_PROPELLANT_SCALE => 1_000_000,
            variable::CANARD_TRAVEL_TURN16 => 1820,
            variable::CANARD_SLEW_TURN16 => 683,
            variable::RCS_RESERVE_Q15 => 6554,
            variable::ROLL_KP_Q15 => 14_000,
            variable::ROLL_KD_Q15 => 4_096,
            _ => m.variables[i].minimum,
        }
    }
    crate::phase9::design_from_values(m, values)
}
fn apply_point_mass(vehicle: &mut SpatialVehiclePack, delta: i32, position_q28: i32) {
    if delta == 0 {
        return;
    }
    let old = vehicle.dry_mass.raw();
    let new = old.saturating_add(delta);
    if new <= 0 {
        return;
    }
    let cg = vehicle.dry_cg_from_nose.raw();
    let ncg = ((i128::from(old) * i128::from(cg) + i128::from(delta) * i128::from(position_q28))
        / i128::from(new))
    .clamp(1, i128::from(i32::MAX)) as i32;
    vehicle.dry_mass = ksa64_core::phase8_numeric::SpatialMass::from_raw(new);
    vehicle.dry_cg_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(ncg);
}
fn materialize_candidate(
    m: &SearchManifest,
    d: &DesignVector,
    study: AdvancedStudyId,
) -> Result<AdvancedReference, AdvancedWorkbenchError> {
    d.validate_against(m)
        .map_err(|_| AdvancedWorkbenchError::Candidate)?;
    let mut r = load_reference(study);
    let control = value_for(m, d, variable::CANARD_CONTROL_SCALE, 1_000_000);
    let drag = value_for(m, d, variable::CANARD_DRAG_SCALE, 1_000_000);
    let travel = value_for(m, d, variable::CANARD_TRAVEL_TURN16, 1820) as i16;
    let slew = value_for(m, d, variable::CANARD_SLEW_TURN16, 683) as i16;
    let canard_mass = value_for(m, d, variable::CANARD_MASS_SCALE, 1_000_000);
    if r.effectors.set.has_canards() {
        for k in &mut r.effectors.coefficient_knots[..r.effectors.coefficient_count as usize] {
            k.control_q24 = scale(k.control_q24, control);
            k.drag_q24 = scale(k.drag_q24, drag)
        }
        let mut delta = 0;
        for c in &mut r.effectors.canards[..r.effectors.canard_count as usize] {
            let old = c.mass_q21;
            c.mass_q21 = scale(old, canard_mass).max(1);
            delta += c.mass_q21 - old;
            c.limit_turn16 = travel.max(1);
            c.slew_turn16_per_release = slew.max(1)
        }
        apply_point_mass(
            &mut r.vehicle,
            delta,
            r.effectors.canards[0].position_q28[0],
        );
    }
    if r.effectors.set.has_rcs() {
        let thrust = value_for(m, d, variable::RCS_THRUST_SCALE, 1_000_000);
        for j in &mut r.effectors.jets[..r.effectors.jet_count as usize] {
            j.nominal_thrust_q23 = scale(j.nominal_thrust_q23, thrust).max(1)
        }
        let prop = value_for(m, d, variable::RCS_PROPELLANT_SCALE, 1_000_000);
        r.effectors.propellant_wet_mass_q21 =
            scale(r.effectors.propellant_wet_mass_q21, prop).max(1);
        for k in &mut r.effectors.supply_knots[..r.effectors.supply_count as usize] {
            k.remaining_propellant_q21 = scale(k.remaining_propellant_q21, prop).max(1)
        }
        r.effectors.reserve_q15 = value_for(
            m,
            d,
            variable::RCS_RESERVE_Q15,
            i32::from(r.effectors.reserve_q15),
        )
        .clamp(0, 32768) as u16;
        r.allocator.reserve_q15 = r.effectors.reserve_q15;
    }
    r.allocator.roll_kp_q15 = value_for(m, d, variable::ROLL_KP_Q15, r.allocator.roll_kp_q15);
    r.allocator.roll_kd_q15 = value_for(m, d, variable::ROLL_KD_Q15, r.allocator.roll_kd_q15);
    r.vehicle.identity = d.materialized_ids[0];
    r.mission.identity = d.materialized_ids[1];
    r.mission.vehicle_identity = r.vehicle.identity;
    r.effectors.identity = d.materialized_ids[2];
    r.effectors.vehicle_identity = r.vehicle.identity;
    r.effectors.neutral_vehicle_identity = r.vehicle.identity;
    r.allocator.identity = d.materialized_ids[3];
    r.allocator.effector_identity = r.effectors.identity;
    if !r.vehicle.is_valid()
        || !r.mission.is_valid()
        || !r.effectors.is_valid()
        || !r.allocator.is_valid()
    {
        return Err(AdvancedWorkbenchError::Candidate);
    }
    Ok(r)
}

pub const ADVANCED_CASE_METRIC_COUNT: usize = 19;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancedCaseEvidence {
    pub outcome: EvaluationOutcome,
    pub metrics: [i32; ADVANCED_CASE_METRIC_COUNT],
    pub checksum: u32,
    pub kas9: [u8; KAS9_LENGTH],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancedCandidateEvaluation {
    pub aggregate: CandidateAggregate,
    pub cases: Vec<AdvancedCaseEvidence>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedWorkbenchError {
    Candidate,
    World,
    Encoding,
    Metric,
    Configuration,
}
fn metric_index(id: u16) -> Option<usize> {
    Some(match id {
        metric::APOGEE => 0,
        metric::LANDING_DISTANCE => 1,
        metric::RAIL_EXIT_VELOCITY => 2,
        metric::MIN_STATIC_MARGIN => 3,
        metric::IMPACT_VELOCITY => 4,
        metric::ATTITUDE_ERROR => 5,
        metric::SYSTEM_MASS => 6,
        metric::HINGE_LOAD => 7,
        metric::SATURATION_COUNT => 8,
        metric::RCS_USE => 9,
        metric::PULSE_COUNT => 10,
        metric::RESIDUAL_TORQUE => 11,
        metric::DEPLOYMENT_ACK => 12,
        metric::ALARMS => 13,
        metric::RESERVE_MARGIN => 14,
        metric::AUTHORITY_HANDOFFS => 15,
        metric::RAIL_SETTLE_ERROR => 16,
        metric::DISTURBANCE_SETTLE_ERROR => 17,
        _ => return None,
    })
}
fn extract_metrics(
    s: &AdvancedEffectorEvaluationSummary,
    dry_mass: i32,
    reserve_q15: u16,
) -> [i32; ADVANCED_CASE_METRIC_COUNT] {
    let p = &s.physical;
    let mut v = [0; ADVANCED_CASE_METRIC_COUNT];
    v[0] = p.metrics[MetricSlot::ApogeeAltitude as usize];
    v[1] = p.metrics[MetricSlot::LandingDistance as usize];
    v[2] = p.metrics[MetricSlot::RailExitVelocity as usize];
    v[3] = p.metrics[MetricSlot::RailExitStaticMargin as usize]
        .min(p.metrics[MetricSlot::BurnoutStaticMargin as usize]);
    v[4] = p.metrics[MetricSlot::ImpactVelocity as usize];
    v[5] = i32::from(s.max_attitude_error_turn16.unsigned_abs());
    v[6] = dry_mass;
    v[7] = s.max_hinge_q24.into_iter().max().unwrap_or(0);
    v[8] = s.saturation_count.min(i32::MAX as u32) as i32;
    v[9] = s.rcs_initial_propellant_q21 - s.rcs_final_propellant_q21;
    v[10] = s.pulse_count.min(i32::MAX as u32) as i32;
    v[11] = s.max_residual_torque_q12.into_iter().max().unwrap_or(0);
    v[12] = i32::from(s.deployment_feedback & 24 == 24);
    v[13] = i32::from(s.alarms != 0);
    v[14] = s.rcs_final_propellant_q21
        - ((i64::from(s.rcs_initial_propellant_q21) * i64::from(reserve_q15) / 32768) as i32);
    v[15] = i32::from(s.authority_handoffs);
    v[16] = i32::from(s.rail_settle_error_turn16.unsigned_abs());
    v[17] = i32::from(s.disturbance_settle_error_turn16.unsigned_abs());
    v[18] = 0;
    v
}
fn advanced_variation(mut e: AdvancedEffectorPack, run: u32) -> (AdvancedEffectorPack, u32) {
    if run == 0 {
        return (e, 0);
    }
    let canard = 1_000_000 + draw(run, 13, -25_000, 25_000);
    let rcs = 1_000_000 + draw(run, 14, -50_000, 50_000);
    let supply = 1_000_000 + draw(run, 15, -50_000, 50_000);
    for k in &mut e.coefficient_knots[..e.coefficient_count as usize] {
        k.control_q24 = scale(k.control_q24, canard)
    }
    for j in &mut e.jets[..e.jet_count as usize] {
        j.nominal_thrust_q23 = scale(j.nominal_thrust_q23, rcs)
    }
    for k in &mut e.supply_knots[..e.supply_count as usize] {
        k.thrust_scale_q30 = scale(k.thrust_scale_q30, supply)
    }
    let mut b = [0u8; 12];
    b[..4].copy_from_slice(&canard.to_le_bytes());
    b[4..8].copy_from_slice(&rcs.to_le_bytes());
    b[8..].copy_from_slice(&supply.to_le_bytes());
    (e, crc32_ieee(&b))
}
fn evaluate_case(
    base: &AdvancedReference,
    run: u32,
    study: AdvancedStudyId,
) -> Result<AdvancedCaseEvidence, AdvancedWorkbenchError> {
    let spatial = derive_spatial_uncertainty(
        SpatialCampaignConfig {
            master_seed: PHASE95_ACCEPTED_SEED,
            run_count: 64,
        },
        run,
    );
    let (mission, mut wind, _physical_variation) =
        materialize_spatial_case(base.mission, &base.wind, spatial, run);
    let variation = SpatialMissionVariation {
        mass_scale_ppm: 1_000_000 + (_physical_variation.mass_scale_ppm - 1_000_000) / 4,
        thrust_scale_ppm: 1_000_000 + (_physical_variation.thrust_scale_ppm - 1_000_000) / 4,
        axial_drag_scale_ppm: 1_000_000
            + (_physical_variation.axial_drag_scale_ppm - 1_000_000) / 4,
        normal_force_scale_ppm: 1_000_000
            + (_physical_variation.normal_force_scale_ppm - 1_000_000) / 4,
        cp_offset_q28: _physical_variation.cp_offset_q28 / 4,
        density_scale_ppm: 1_000_000 + (_physical_variation.density_scale_ppm - 1_000_000) / 4,
        wind_scale_ppm: 1_000_000,
        recovery_cda_scale_ppm: 1_000_000
            + (_physical_variation.recovery_cda_scale_ppm - 1_000_000) / 4,
        inflation_scale_ppm: 1_000_000 + (_physical_variation.inflation_scale_ppm - 1_000_000) / 4,
    };
    if run != 0 {
        let mut desired = wind.knots[0];
        desired.east = SpatialWind::from_raw(desired.east.raw() / 20);
        desired.north = SpatialWind::from_raw(desired.north.raw() / 20);
        wind.gust_amplitude_east = SpatialWind::from_raw(wind.gust_amplitude_east.raw() / 20);
        wind.gust_amplitude_north = SpatialWind::from_raw(wind.gust_amplitude_north.raw() / 20);
        wind.max_gust = SpatialWind::from_raw(wind.max_gust.raw() / 20);
        wind.knot_count = 3;
        wind.knots[0].east = SpatialWind::ZERO;
        wind.knots[0].north = SpatialWind::ZERO;
        wind.knots[1] = desired;
        wind.knots[1].altitude = SpatialPosition::from_raw(50 << 13);
        wind.knots[2] = desired;
        wind.knots[2].altitude = SpatialPosition::from_raw(100_000 << 13);
    }
    let (effectors, advanced_crc) = advanced_variation(base.effectors, run);
    let mut faults = AdvancedMissionFaults::NOMINAL;
    if study.has_rcs() {
        faults.disturbance_epoch = 256;
        let d = if run == 0 {
            1 << 22
        } else {
            draw(run, 16, 1 << 21, 1 << 22)
        };
        faults.disturbance_angular_rate_q24 = [d, -d, d]
    }
    if run != 0 && keyed_word_raw(PHASE95_ACCEPTED_SEED, run, 17, 0, 0) & 15 == 0 {
        faults.pitot_dropout_start = 32;
        faults.pitot_dropout_epochs = 32
    }
    let capability = reference_capability(base.vehicle.identity, &base.allocator);
    let summary = evaluate_with_advanced_effectors(AdvancedEffectorEvaluationRequest {
        vehicle: &base.vehicle,
        motor: &base.motor,
        mission,
        wind: &wind,
        variation,
        variation_checksum: spatial.checksum.rotate_left(7) ^ advanced_crc,
        avionics: reference_avionics_profile(false),
        capability,
        effectors: &effectors,
        allocator: &base.allocator,
        uncertainty_identity: run,
        evaluator_identity: ADVANCED_EVALUATOR_ID,
        faults,
    })
    .map_err(|_| AdvancedWorkbenchError::World)?;
    let metrics = extract_metrics(
        &summary,
        base.vehicle.dry_mass.raw(),
        base.allocator.reserve_q15,
    );
    let mut kas9 = [0u8; KAS9_LENGTH];
    write_advanced_effector_summary(summary, &mut kas9)
        .map_err(|_| AdvancedWorkbenchError::Encoding)?;
    Ok(AdvancedCaseEvidence {
        outcome: summary.physical.outcome,
        metrics,
        checksum: crc32_ieee(&kas9),
        kas9,
    })
}
fn aggregate_value(c: &[AdvancedCaseEvidence], index: usize, a: AggregateId) -> i32 {
    let mut v: Vec<i32> = c.iter().map(|x| x.metrics[index]).collect();
    match a {
        AggregateId::Nominal => v[0],
        AggregateId::Minimum => *v.iter().min().unwrap(),
        AggregateId::Maximum => *v.iter().max().unwrap(),
        AggregateId::Mean => {
            let s: i128 = v.iter().map(|x| i128::from(*x)).sum();
            let n = v.len() as i128;
            ((s + if s < 0 { -(n / 2) } else { n / 2 }) / n)
                .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
        }
        AggregateId::Quantile95 => {
            v.sort_unstable();
            v[(95 * v.len()).div_ceil(100).max(1) - 1]
        }
        AggregateId::FailureRate => {
            ((c.iter()
                .filter(|x| {
                    !matches!(
                        x.outcome,
                        EvaluationOutcome::Complete | EvaluationOutcome::GroundContact
                    )
                })
                .count()
                * 1_000_000
                + c.len() / 2)
                / c.len()) as i32
        }
    }
}
fn fatal(o: EvaluationOutcome) -> u8 {
    match o {
        EvaluationOutcome::Complete | EvaluationOutcome::GroundContact => 0,
        EvaluationOutcome::RecoveryIncomplete => 1,
        EvaluationOutcome::ModelEnvelopeExceeded => 2,
        EvaluationOutcome::NoLiftoff | EvaluationOutcome::StepLimit => 3,
        EvaluationOutcome::Aborted => 4,
        EvaluationOutcome::ConfigurationFault => 5,
        EvaluationOutcome::NumericFault => 6,
        _ => 1,
    }
}
#[allow(clippy::needless_range_loop)]
fn aggregate_candidate(
    m: &SearchManifest,
    d: &DesignVector,
    c: &[AdvancedCaseEvidence],
) -> Result<CandidateAggregate, AdvancedWorkbenchError> {
    let mut objectives = [0; MAX_METRICS];
    for i in 0..m.objective_count as usize {
        objectives[i] = aggregate_value(
            c,
            metric_index(m.objectives[i].metric_id).ok_or(AdvancedWorkbenchError::Metric)?,
            m.objectives[i].aggregate,
        )
    }
    let mut values = [0; MAX_CONSTRAINT_RESULTS];
    let mut violated = 0u8;
    let mut norm = 0i128;
    for i in 0..m.constraint_count as usize {
        let s = m.constraints[i];
        let v = aggregate_value(
            c,
            metric_index(s.metric_id).ok_or(AdvancedWorkbenchError::Metric)?,
            s.aggregate,
        );
        values[i] = v;
        let delta = match s.op {
            ConstraintOp::AtLeast => (i128::from(s.threshold) - i128::from(v)).max(0),
            ConstraintOp::AtMost => (i128::from(v) - i128::from(s.threshold)).max(0),
            ConstraintOp::Equal => (i128::from(v) - i128::from(s.threshold)).abs(),
        };
        if delta > 0 {
            violated = violated.saturating_add(1);
            norm = norm.saturating_add(delta * 1_000_000 / i128::from(s.scale))
        }
    }
    let fatal = c.iter().map(|x| fatal(x.outcome)).max().unwrap_or(0);
    let mut b = Vec::new();
    for x in c {
        b.extend_from_slice(&x.checksum.to_le_bytes())
    }
    Ok(CandidateAggregate {
        identity: 0,
        manifest_identity: m.identity,
        candidate_identity: d.identity,
        uncertainty_tier: c.len() as u8,
        case_count: c.len() as u8,
        fatal_class: fatal,
        violated_constraints: violated,
        feasible: fatal == 0 && violated == 0,
        case_crc: crc32_ieee(&b),
        normalized_violation: norm.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        objective_count: m.objective_count,
        constraint_count: m.constraint_count,
        objectives,
        constraint_values: values,
    }
    .seal())
}

pub fn evaluate_advanced_candidate(
    m: &SearchManifest,
    d: &DesignVector,
    study: AdvancedStudyId,
    tier: u8,
) -> Result<AdvancedCandidateEvaluation, AdvancedWorkbenchError> {
    if !matches!(tier, 1 | 8 | 64) {
        return Err(AdvancedWorkbenchError::Configuration);
    }
    let base = materialize_candidate(m, d, study)?;
    let limit = if study.experimental() {
        tier.min(8)
    } else {
        tier
    };
    let mut cases = Vec::with_capacity(limit as usize);
    for run in 0..u32::from(limit) {
        cases.push(evaluate_case(&base, run, study)?)
    }
    let aggregate = aggregate_candidate(m, d, &cases)?;
    Ok(AdvancedCandidateEvaluation { aggregate, cases })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancedCampaignResult {
    pub config: [u8; KSC9_LENGTH],
    pub records: Vec<[u8; KAS9_LENGTH]>,
    pub crc32: u32,
}
pub fn run_advanced_campaign(
    study: AdvancedStudyId,
    workers: usize,
) -> Result<AdvancedCampaignResult, AdvancedWorkbenchError> {
    let m = built_in_advanced_manifest(study, SearchEngineId::Nsga2V1);
    let d = baseline_advanced_vector(&m);
    let base = materialize_candidate(&m, &d, study)?;
    let n = ADVANCED_CAMPAIGN_RUNS;
    let next = AtomicU32::new(0);
    let slots = Mutex::new(vec![None; n as usize]);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1).min(n as usize) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                slots.lock().unwrap()[i as usize] = Some(evaluate_case(&base, i, study))
            });
        }
    });
    let mut records = Vec::with_capacity(n as usize);
    for s in slots.into_inner().unwrap() {
        records.push(s.expect("ordered campaign")?.kas9)
    }
    let mut all = Vec::with_capacity(records.len() * KAS9_LENGTH);
    for r in &records {
        all.extend_from_slice(r)
    }
    let config = AdvancedCampaignConfig {
        identity: crc32_ieee(&all).max(1),
        master_seed: PHASE95_ACCEPTED_SEED,
        run_count: n as u16,
        vehicle_identity: base.vehicle.identity,
        motor_identity: base.motor.identity,
        mission_identity: base.mission.identity,
        wind_identity: base.wind.identity,
        avionics_identity: reference_avionics_profile(false).identity,
        effector_identity: base.effectors.identity,
        allocator_identity: base.allocator.identity,
    };
    let mut cfg = [0u8; KSC9_LENGTH];
    write_advanced_campaign_config(&config, &mut cfg)
        .map_err(|_| AdvancedWorkbenchError::Encoding)?;
    Ok(AdvancedCampaignResult {
        config: cfg,
        records,
        crc32: crc32_ieee(&all),
    })
}

struct Adapter<'a> {
    manifest: &'a SearchManifest,
    study: AdvancedStudyId,
    evidence: Mutex<BTreeMap<(u32, u8), AdvancedCandidateEvaluation>>,
}
impl CandidateEvaluator for Adapter<'_> {
    fn evaluate(
        &self,
        d: &DesignVector,
        tier: u8,
    ) -> Result<LegacyCandidateEvaluation, SearchError> {
        let value = evaluate_advanced_candidate(self.manifest, d, self.study, tier)
            .map_err(|_| SearchError::Evaluation)?;
        self.evidence
            .lock()
            .unwrap()
            .insert((d.identity, tier), value.clone());
        Ok(LegacyCandidateEvaluation {
            aggregate: value.aggregate,
            cases: Vec::new(),
        })
    }
}
#[derive(Debug)]
pub struct AdvancedSearchResult {
    pub search: SearchResult,
    pub evidence: BTreeMap<(u32, u8), AdvancedCandidateEvaluation>,
}
pub fn run_advanced_search(
    m: &SearchManifest,
    study: AdvancedStudyId,
    workers: usize,
) -> Result<AdvancedSearchResult, SearchError> {
    let adapter = Adapter {
        manifest: m,
        study,
        evidence: Mutex::new(BTreeMap::new()),
    };
    let mut search = run_search_with_workers(m, &adapter, &[0, 1], workers)?;
    if !study.experimental() {
        search
            .finalists
            .retain(|candidate| candidate.aggregate.feasible);
    }
    let evidence = adapter.evidence.into_inner().unwrap();
    Ok(AdvancedSearchResult { search, evidence })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase9_5_contract::parse_advanced_campaign_config;
    #[test]
    fn manifests_and_baselines_are_valid() {
        for s in [
            AdvancedStudyId::Canard,
            AdvancedStudyId::Rcs,
            AdvancedStudyId::Mixed,
            AdvancedStudyId::Research,
        ] {
            for e in [SearchEngineId::GridV1, SearchEngineId::Nsga2V1] {
                let m = built_in_advanced_manifest(s, e);
                m.validate().unwrap();
                let d = baseline_advanced_vector(&m);
                materialize_candidate(&m, &d, s).unwrap();
            }
        }
    }
    #[test]
    fn nominal_advanced_candidate_is_repeatable() {
        let m = built_in_advanced_manifest(AdvancedStudyId::Mixed, SearchEngineId::Nsga2V1);
        let d = baseline_advanced_vector(&m);
        let a = evaluate_advanced_candidate(&m, &d, AdvancedStudyId::Mixed, 1).unwrap();
        let b = evaluate_advanced_candidate(&m, &d, AdvancedStudyId::Mixed, 1).unwrap();
        assert_eq!(a, b)
    }
    #[test]
    fn ksc9_campaign_contract_round_trips() {
        let r = run_advanced_campaign(AdvancedStudyId::Mixed, 1).unwrap();
        assert_eq!(
            parse_advanced_campaign_config(&r.config).unwrap().run_count,
            64
        );
        assert_eq!(r.records.len(), 64)
    }
}
