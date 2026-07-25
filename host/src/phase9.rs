//! Phase 9 candidate materialization and exact robustness evaluation.
use crate::phase8_5::checked_in_reference;
use crate::phase8_5_campaign::derive_avionics_case;
use ksa64_core::evaluation::{EvaluationOutcome, MetricSlot};
use ksa64_core::phase8_5_contract::{write_avionics_summary, KAS8_LENGTH};
use ksa64_core::phase9_contract::{
    AggregateId, CandidateAggregate, ConstraintOp, ConstraintSpec, DesignVector, Direction,
    ObjectiveSpec, SearchBudgets, SearchEngineId, SearchManifest, SearchPresetId, VariableKind,
    VariableSpec, MAX_CONSTRAINTS, MAX_CONSTRAINT_RESULTS, MAX_METRICS, MAX_OBJECTIVES,
    MAX_VARIABLES,
};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase8_5::{
    evaluate_with_avionics, five_mps_crosswind_settling_error, AvionicsEvaluationRequest,
};
use ksa64_sim::phase8_campaign::{
    derive_spatial_uncertainty, materialize_spatial_case, SpatialCampaignConfig,
};
use std::collections::BTreeMap;

pub const STUDY_A_ID: u32 = 0x3941_0001;
pub const STUDY_B_ID: u32 = 0x3942_0001;
pub const COUPLED_STUDY_ID: u32 = 0x3943_0001;
pub const EXPERIMENTAL_AIRFRAME_ID: u32 = 0x3945_0001;
pub const PHASE9_SEED: u32 = 0x4b53_4139;

pub mod variable {
    pub const FIN_ROOT_SCALE: u16 = 1;
    pub const FIN_SPAN_SCALE: u16 = 2;
    pub const FIN_TIP_SCALE: u16 = 3;
    pub const FIN_SWEEP_SCALE: u16 = 4;
    pub const BALLAST_MASS_Q21: u16 = 5;
    pub const BALLAST_POSITION_Q28: u16 = 6;
    pub const DROGUE_CDA_SCALE: u16 = 7;
    pub const MAIN_CDA_SCALE: u16 = 8;
    pub const MAIN_ALTITUDE_Q13: u16 = 9;
    pub const DROGUE_INFLATION_SCALE: u16 = 10;
    pub const MAIN_INFLATION_SCALE: u16 = 11;
    pub const RAIL_LENGTH_Q13: u16 = 12;
    pub const GIMBAL_TRAVEL_Q16: u16 = 20;
    pub const GIMBAL_SLEW_Q16: u16 = 21;
    pub const GIMBAL_LAG: u16 = 22;
    pub const ACTUATOR_MASS_Q21: u16 = 23;
    pub const PROPORTIONAL_GAIN_Q15: u16 = 24;
    pub const DERIVATIVE_GAIN_Q15: u16 = 25;
    pub const BODY_LENGTH_SCALE: u16 = 30;
    pub const BODY_DIAMETER_SCALE: u16 = 31;
    pub const OGIVE_FINENESS_Q16: u16 = 32;
}
pub mod metric {
    use ksa64_core::evaluation::MetricSlot;
    pub const APOGEE: u16 = MetricSlot::ApogeeAltitude as u16 + 1;
    pub const LANDING_DISTANCE: u16 = MetricSlot::LandingDistance as u16 + 1;
    pub const RAIL_EXIT_VELOCITY: u16 = MetricSlot::RailExitVelocity as u16 + 1;
    pub const MIN_STATIC_MARGIN: u16 = MetricSlot::MinimumStaticMargin as u16 + 1;
    pub const IMPACT_VELOCITY: u16 = MetricSlot::ImpactVelocity as u16 + 1;
    pub const MAX_MACH: u16 = MetricSlot::MaxMach as u16 + 1;
    pub const MAX_AOA: u16 = MetricSlot::MaximumAngleOfAttack as u16 + 1;
    pub const DRY_MASS: u16 = 100;
    pub const ATTITUDE_ERROR: u16 = 101;
    pub const SATURATION_COUNT: u16 = 102;
    pub const DEPLOYMENT_ACK: u16 = 103;
    pub const ALARMS: u16 = 104;
    pub const SETTLING_ERROR: u16 = 105;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudyId {
    PassiveRecovery,
    GimbalControl,
    Coupled,
    ExperimentalAirframe,
}
impl StudyId {
    pub const fn raw(self) -> u32 {
        match self {
            Self::PassiveRecovery => STUDY_A_ID,
            Self::GimbalControl => STUDY_B_ID,
            Self::Coupled => COUPLED_STUDY_ID,
            Self::ExperimentalAirframe => EXPERIMENTAL_AIRFRAME_ID,
        }
    }
    pub const fn gimbal(self) -> bool {
        matches!(self, Self::GimbalControl | Self::Coupled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase9EvaluationError {
    Configuration,
    Candidate,
    World,
    Metric,
}

pub const CASE_METRIC_COUNT: usize = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseEvidence {
    pub outcome: EvaluationOutcome,
    pub metrics: [i32; CASE_METRIC_COUNT],
    pub valid: u16,
    pub checksum: u32,
    pub kas8: [u8; KAS8_LENGTH],
}
impl CaseEvidence {
    fn metric(&self, index: usize) -> Option<i32> {
        if self.valid & (1 << index) != 0 {
            Some(self.metrics[index])
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateEvaluation {
    pub aggregate: CandidateAggregate,
    pub cases: Vec<CaseEvidence>,
}

fn variable(id: u16, min: i32, max: i32, quantum: u32) -> VariableSpec {
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
fn objective(metric_id: u16, aggregate: AggregateId, direction: Direction) -> ObjectiveSpec {
    ObjectiveSpec {
        metric_id,
        aggregate,
        direction,
    }
}
fn constraint(
    metric_id: u16,
    aggregate: AggregateId,
    op: ConstraintOp,
    threshold: i32,
    scale: u32,
) -> ConstraintSpec {
    ConstraintSpec {
        metric_id,
        aggregate,
        op,
        threshold,
        scale,
    }
}

pub fn built_in_manifest(
    study: StudyId,
    engine: SearchEngineId,
    preset: SearchPresetId,
) -> SearchManifest {
    let mut variables = [VariableSpec::EMPTY; MAX_VARIABLES];
    let mut objectives = [ObjectiveSpec::EMPTY; MAX_OBJECTIVES];
    let mut constraints = [ConstraintSpec::EMPTY; MAX_CONSTRAINTS];
    let (vc, oc, cc) = match study {
        StudyId::PassiveRecovery => {
            variables[0] = variable(variable::FIN_SPAN_SCALE, 850_000, 1_150_000, 10_000);
            variables[1] = variable(variable::MAIN_ALTITUDE_Q13, 150 << 13, 300 << 13, 5 << 13);
            variables[2] = variable(variable::FIN_ROOT_SCALE, 850_000, 1_150_000, 10_000);
            variables[3] = variable(variable::FIN_TIP_SCALE, 850_000, 1_150_000, 10_000);
            variables[4] = variable(variable::FIN_SWEEP_SCALE, 850_000, 1_150_000, 10_000);
            variables[5] = variable(variable::BALLAST_MASS_Q21, 0, 419_430, 20_971);
            variables[6] = variable(
                variable::BALLAST_POSITION_Q28,
                53_687_091,
                214_748_365,
                5_368_709,
            );
            variables[7] = variable(variable::DROGUE_CDA_SCALE, 800_000, 1_200_000, 25_000);
            variables[8] = variable(variable::MAIN_CDA_SCALE, 800_000, 1_200_000, 25_000);
            variables[9] = variable(variable::DROGUE_INFLATION_SCALE, 750_000, 1_250_000, 25_000);
            variables[10] = variable(variable::MAIN_INFLATION_SCALE, 750_000, 1_250_000, 25_000);
            variables[11] = variable(variable::RAIL_LENGTH_Q13, 12_288, 24_576, 1_024);
            objectives[0] = objective(metric::APOGEE, AggregateId::Mean, Direction::Maximize);
            objectives[1] = objective(
                metric::LANDING_DISTANCE,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[2] = objective(metric::DRY_MASS, AggregateId::Maximum, Direction::Minimize);
            (12, 3, common_constraints(&mut constraints))
        }
        StudyId::GimbalControl => {
            variables[0] = variable(variable::PROPORTIONAL_GAIN_Q15, 6_000, 24_000, 500);
            variables[1] = variable(variable::DERIVATIVE_GAIN_Q15, 1_024, 12_000, 256);
            variables[2] = variable(variable::GIMBAL_TRAVEL_Q16, 2 << 16, 7 << 16, 1 << 15);
            variables[3] = variable(variable::GIMBAL_SLEW_Q16, 15 << 16, 60 << 16, 5 << 16);
            variables[4] = variable(variable::GIMBAL_LAG, 1, 4, 1);
            variables[5] = variable(variable::ACTUATOR_MASS_Q21, 20_971, 167_772, 4_194);
            objectives[0] = objective(
                metric::ATTITUDE_ERROR,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[1] = objective(metric::DRY_MASS, AggregateId::Maximum, Direction::Minimize);
            objectives[2] = objective(
                metric::SATURATION_COUNT,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            let mut c = common_constraints(&mut constraints);
            constraints[c] = constraint(
                metric::SETTLING_ERROR,
                AggregateId::Maximum,
                ConstraintOp::AtMost,
                547,
                1,
            );
            c += 1;
            (6, 3, c)
        }
        StudyId::Coupled => {
            let a = built_in_manifest(StudyId::PassiveRecovery, engine, preset);
            let b = built_in_manifest(StudyId::GimbalControl, engine, preset);
            let ac = a.variable_count as usize;
            let bc = b.variable_count as usize;
            variables[..ac].copy_from_slice(&a.variables[..ac]);
            variables[ac..ac + bc].copy_from_slice(&b.variables[..bc]);
            objectives[0] = objective(metric::APOGEE, AggregateId::Mean, Direction::Maximize);
            objectives[1] = objective(metric::DRY_MASS, AggregateId::Maximum, Direction::Minimize);
            objectives[2] = objective(
                metric::LANDING_DISTANCE,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            objectives[3] = objective(
                metric::ATTITUDE_ERROR,
                AggregateId::Quantile95,
                Direction::Minimize,
            );
            let mut c = common_constraints(&mut constraints);
            constraints[c] = constraint(
                metric::SETTLING_ERROR,
                AggregateId::Maximum,
                ConstraintOp::AtMost,
                547,
                1,
            );
            c += 1;
            (18, 4, c)
        }
        StudyId::ExperimentalAirframe => {
            variables[0] = variable(variable::BODY_LENGTH_SCALE, 750_000, 1_250_000, 25_000);
            variables[1] = variable(variable::BODY_DIAMETER_SCALE, 750_000, 1_250_000, 25_000);
            variables[2] = variable(variable::OGIVE_FINENESS_Q16, 3 << 16, 7 << 16, 1 << 14);
            variables[3] = variable(variable::FIN_ROOT_SCALE, 600_000, 1_400_000, 25_000);
            variables[4] = variable(variable::FIN_SPAN_SCALE, 600_000, 1_400_000, 25_000);
            variables[5] = variable(variable::FIN_TIP_SCALE, 600_000, 1_400_000, 25_000);
            variables[6] = variable(variable::FIN_SWEEP_SCALE, 600_000, 1_400_000, 25_000);
            variables[7] = variable(variable::BALLAST_MASS_Q21, 0, 1_048_576, 52_429);
            objectives[0] = objective(metric::APOGEE, AggregateId::Mean, Direction::Maximize);
            objectives[1] = objective(metric::DRY_MASS, AggregateId::Maximum, Direction::Minimize);
            (8, 2, common_constraints(&mut constraints))
        }
    };
    let gimbal = study.gimbal();
    let reference = checked_in_reference(gimbal).expect("checked-in Phase 8.5 reference");
    let mut m = SearchManifest {
        identity: 0,
        base_ids: [
            reference.vehicle.identity,
            reference.motor.identity,
            reference.mission.identity,
            reference.wind.identity,
            reference.avionics.identity,
            reference.capability.identity,
            study.raw(),
            0,
        ],
        engine,
        preset,
        master_seed: PHASE9_SEED,
        budgets: SearchBudgets::for_preset(preset),
        variable_count: vc,
        objective_count: oc,
        constraint_count: cc as u8,
        variables,
        objectives,
        constraints,
    };
    if study == StudyId::Coupled {
        m.budgets.population = 32;
        m.budgets.generations = 16;
        m.budgets.finalists = 16
    }
    if study == StudyId::ExperimentalAirframe {
        m.budgets.population = 32;
        m.budgets.generations = 12;
        m.budgets.finalists = 0
    }
    m.seal().expect("built-in manifest")
}
fn common_constraints(c: &mut [ConstraintSpec; MAX_CONSTRAINTS]) -> usize {
    c[0] = constraint(
        metric::RAIL_EXIT_VELOCITY,
        AggregateId::Minimum,
        ConstraintOp::AtLeast,
        15 << 19,
        1 << 19,
    );
    c[1] = constraint(
        metric::MIN_STATIC_MARGIN,
        AggregateId::Minimum,
        ConstraintOp::AtLeast,
        1 << 24,
        1 << 24,
    );
    c[2] = constraint(
        metric::IMPACT_VELOCITY,
        AggregateId::Maximum,
        ConstraintOp::AtMost,
        8 << 19,
        1 << 19,
    );
    c[3] = constraint(
        metric::DEPLOYMENT_ACK,
        AggregateId::Minimum,
        ConstraintOp::AtLeast,
        1,
        1,
    );
    c[4] = constraint(
        metric::ALARMS,
        AggregateId::Maximum,
        ConstraintOp::AtMost,
        0,
        1,
    );
    5
}

pub fn baseline_vector(manifest: &SearchManifest) -> DesignVector {
    let mut values = [0; 32];
    for (i, value) in values
        .iter_mut()
        .enumerate()
        .take(manifest.variable_count as usize)
    {
        let spec = manifest.variables[i];
        *value = match spec.id {
            variable::FIN_ROOT_SCALE
            | variable::FIN_SPAN_SCALE
            | variable::FIN_TIP_SCALE
            | variable::FIN_SWEEP_SCALE
            | variable::DROGUE_CDA_SCALE
            | variable::MAIN_CDA_SCALE
            | variable::DROGUE_INFLATION_SCALE
            | variable::MAIN_INFLATION_SCALE
            | variable::BODY_LENGTH_SCALE
            | variable::BODY_DIAMETER_SCALE => 1_000_000,
            variable::OGIVE_FINENESS_Q16 => 5 << 16,
            variable::MAIN_ALTITUDE_Q13 => 200 << 13,
            variable::RAIL_LENGTH_Q13 => 2 << 13,
            variable::BALLAST_POSITION_Q28 => spec.minimum,
            variable::GIMBAL_TRAVEL_Q16 => 5 << 16,
            variable::GIMBAL_SLEW_Q16 => 30 << 16,
            variable::GIMBAL_LAG => 2,
            variable::ACTUATOR_MASS_Q21 => 41_943,
            variable::PROPORTIONAL_GAIN_Q15 => 14_000,
            variable::DERIVATIVE_GAIN_Q15 => 4_096,
            _ => spec.minimum,
        };
    }
    design_from_values(manifest, values)
}

pub fn design_from_values(manifest: &SearchManifest, mut values: [i32; 32]) -> DesignVector {
    let mut ballast_zero = false;
    for (i, value) in values
        .iter_mut()
        .enumerate()
        .take(manifest.variable_count as usize)
    {
        *value = quantize_clamp(*value, manifest.variables[i]);
        if manifest.variables[i].id == variable::BALLAST_MASS_Q21 && *value == 0 {
            ballast_zero = true;
        }
    }
    if ballast_zero {
        for (i, value) in values
            .iter_mut()
            .enumerate()
            .take(manifest.variable_count as usize)
        {
            if manifest.variables[i].id == variable::BALLAST_POSITION_Q28 {
                *value = manifest.variables[i].minimum;
            }
        }
    }
    let mut materialized = [0u32; 4];
    let mut bytes = Vec::with_capacity(8 + manifest.variable_count as usize * 4);
    bytes.extend_from_slice(&manifest.identity.to_le_bytes());
    for value in &values[..manifest.variable_count as usize] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for (slot, identity) in materialized.iter_mut().enumerate() {
        bytes.extend_from_slice(&(slot as u32 + 1).to_le_bytes());
        *identity = crc32_ieee(&bytes);
        bytes.truncate(bytes.len() - 4);
    }
    DesignVector {
        identity: 0,
        manifest_identity: manifest.identity,
        value_count: manifest.variable_count,
        values,
        materialized_ids: materialized,
    }
    .seal()
    .unwrap()
}

fn quantize_clamp(value: i32, spec: VariableSpec) -> i32 {
    let v = value.clamp(spec.minimum, spec.maximum);
    let q = spec.quantum as i64;
    let k = (i64::from(v) - i64::from(spec.minimum) + q / 2) / q;
    (i64::from(spec.minimum) + k * q).clamp(i64::from(spec.minimum), i64::from(spec.maximum)) as i32
}
fn scale(raw: i32, ppm: i32) -> i32 {
    ((i64::from(raw) * i64::from(ppm)) / 1_000_000).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
        as i32
}

fn value_for(manifest: &SearchManifest, design: &DesignVector, id: u16, default: i32) -> i32 {
    for i in 0..design.value_count as usize {
        if manifest.variables[i].id == id {
            return design.values[i];
        }
    }
    default
}
fn apply_point_mass(
    vehicle: &mut ksa64_core::phase8_pack::SpatialVehiclePack,
    delta_mass: i32,
    position_q28: i32,
) {
    if delta_mass == 0 {
        return;
    }
    let old_mass = vehicle.dry_mass.raw();
    let new_mass = old_mass.saturating_add(delta_mass);
    if new_mass <= 0 {
        return;
    }
    let old_cg = vehicle.dry_cg_from_nose.raw();
    let new_cg = ((i128::from(old_mass) * i128::from(old_cg)
        + i128::from(delta_mass) * i128::from(position_q28))
        / i128::from(new_mass))
    .clamp(1, i128::from(i32::MAX)) as i32;
    let old_shift = i128::from(old_cg - new_cg);
    let point_shift = i128::from(position_q28 - new_cg);
    let correction = ((i128::from(old_mass) * old_shift * old_shift
        + i128::from(delta_mass) * point_shift * point_shift)
        >> 58)
        .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32;
    for axis in 1..3 {
        vehicle.dry_inertia[axis] = ksa64_core::phase8_numeric::SpatialInertia::from_raw(
            vehicle.dry_inertia[axis]
                .raw()
                .saturating_add(correction)
                .max(1),
        )
    }
    vehicle.dry_mass = ksa64_core::phase8_numeric::SpatialMass::from_raw(new_mass);
    vehicle.dry_cg_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(new_cg)
}

fn materialize(
    manifest: &SearchManifest,
    design: &DesignVector,
    study: StudyId,
) -> Result<crate::phase8_5::ReferenceConfiguration, Phase9EvaluationError> {
    design
        .validate_against(manifest)
        .map_err(|_| Phase9EvaluationError::Candidate)?;
    let mut r =
        checked_in_reference(study.gimbal()).map_err(|_| Phase9EvaluationError::Configuration)?;
    let body_length = value_for(manifest, design, variable::BODY_LENGTH_SCALE, 1_000_000);
    let body_diameter = value_for(manifest, design, variable::BODY_DIAMETER_SCALE, 1_000_000);
    let ogive_fineness = value_for(manifest, design, variable::OGIVE_FINENESS_Q16, 5 << 16);
    if study == StudyId::ExperimentalAirframe {
        let old_length_q13 = r.vehicle.length.raw();
        let new_length_q13 = scale(old_length_q13, body_length);
        let new_length_q28 = i64::from(new_length_q13) << 15;
        let diameter_area_scale = ((i64::from(body_diameter) * i64::from(body_diameter))
            / 1_000_000)
            .clamp(250_000, 2_000_000) as i32;
        let shell_mass_scale = ((i64::from(body_length) * i64::from(body_diameter)) / 1_000_000)
            .clamp(250_000, 2_000_000) as i32;
        r.vehicle.length = ksa64_core::phase8_numeric::SpatialPosition::from_raw(new_length_q13);
        r.vehicle.diameter = ksa64_core::phase8_numeric::SpatialPosition::from_raw(scale(
            r.vehicle.diameter.raw(),
            body_diameter,
        ));
        r.vehicle.reference_area = ksa64_core::phase8_numeric::SpatialArea::from_raw(scale(
            r.vehicle.reference_area.raw(),
            diameter_area_scale,
        ));
        r.vehicle.dry_cg_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(scale(
            r.vehicle.dry_cg_from_nose.raw(),
            body_length,
        ));
        r.vehicle.motor_aft_from_tail = ksa64_core::phase8_numeric::SpatialPosition::from_raw(
            scale(r.vehicle.motor_aft_from_tail.raw(), body_length),
        );
        r.vehicle.aft_rail_guide_from_tail = ksa64_core::phase8_numeric::SpatialPosition::from_raw(
            scale(r.vehicle.aft_rail_guide_from_tail.raw(), body_length),
        );
        r.vehicle.forward_rail_guide_from_tail =
            ksa64_core::phase8_numeric::SpatialPosition::from_raw(scale(
                r.vehicle.forward_rail_guide_from_tail.raw(),
                body_length,
            ));
        let old_mass = r.vehicle.dry_mass.raw();
        let new_mass = scale(old_mass, shell_mass_scale).max(1);
        r.vehicle.dry_mass = ksa64_core::phase8_numeric::SpatialMass::from_raw(new_mass);
        let axial_scale = ((i64::from(shell_mass_scale) * i64::from(diameter_area_scale))
            / 1_000_000)
            .clamp(125_000, 4_000_000) as i32;
        let length_squared = ((i64::from(body_length) * i64::from(body_length)) / 1_000_000)
            .clamp(250_000, 2_000_000) as i32;
        let transverse_scale = ((i64::from(shell_mass_scale) * i64::from(length_squared))
            / 1_000_000)
            .clamp(125_000, 4_000_000) as i32;
        r.vehicle.dry_inertia[0] = ksa64_core::phase8_numeric::SpatialInertia::from_raw(
            scale(r.vehicle.dry_inertia[0].raw(), axial_scale).max(1),
        );
        for axis in 1..3 {
            r.vehicle.dry_inertia[axis] = ksa64_core::phase8_numeric::SpatialInertia::from_raw(
                scale(r.vehicle.dry_inertia[axis].raw(), transverse_scale).max(1),
            );
        }
        let cp_shift_q28 =
            new_length_q28 * i64::from(ogive_fineness - (5 << 16)) / i64::from(40 << 16);
        for knot in &mut r.vehicle.aero_knots[..r.vehicle.aero_knot_count as usize] {
            knot.axial_cd = ksa64_core::phase8_numeric::SpatialCoefficient::from_raw(scale(
                knot.axial_cd.raw(),
                diameter_area_scale,
            ));
            knot.cp_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(
                (i64::from(scale(knot.cp_from_nose.raw(), body_length)) + cp_shift_q28)
                    .clamp(1, new_length_q28) as i32,
            );
        }
    }
    let root = value_for(manifest, design, variable::FIN_ROOT_SCALE, 1_000_000);
    let span = value_for(manifest, design, variable::FIN_SPAN_SCALE, 1_000_000);
    let tip = value_for(manifest, design, variable::FIN_TIP_SCALE, 1_000_000);
    let sweep = value_for(manifest, design, variable::FIN_SWEEP_SCALE, 1_000_000);
    let chord_mean = (i64::from(root) + i64::from(tip)) / 2;
    let area_scale = ((i64::from(span) * chord_mean) / 1_000_000).clamp(500_000, 2_000_000) as i32;
    let fin_mass_base = r.vehicle.dry_mass.raw() / 12;
    let fin_delta =
        ((i64::from(fin_mass_base) * i64::from(area_scale - 1_000_000)) / 1_000_000) as i32;
    let fin_position_q28 = ((i64::from(r.vehicle.length.raw()) << 15) * 4 / 5) as i32;
    apply_point_mass(&mut r.vehicle, fin_delta, fin_position_q28);
    r.vehicle.pitch_damping = ksa64_core::phase8_numeric::SpatialCoefficient::from_raw(scale(
        r.vehicle.pitch_damping.raw(),
        area_scale,
    ));
    r.vehicle.yaw_damping = ksa64_core::phase8_numeric::SpatialCoefficient::from_raw(scale(
        r.vehicle.yaw_damping.raw(),
        area_scale,
    ));
    let length_q28 = (i64::from(r.vehicle.length.raw())) << 15;
    let sweep_shift = length_q28 * i64::from(sweep - 1_000_000) / 10_000_000;
    for knot in &mut r.vehicle.aero_knots[..r.vehicle.aero_knot_count as usize] {
        knot.normal_force_slope = ksa64_core::phase8_numeric::SpatialCoefficient::from_raw(scale(
            knot.normal_force_slope.raw(),
            area_scale,
        ));
        knot.cp_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(
            (i64::from(knot.cp_from_nose.raw()) + sweep_shift).clamp(1, length_q28) as i32,
        )
    }
    let ballast = value_for(manifest, design, variable::BALLAST_MASS_Q21, 0);
    let ballast_position = value_for(manifest, design, variable::BALLAST_POSITION_Q28, 53_687_091);
    apply_point_mass(&mut r.vehicle, ballast, ballast_position);
    for i in 0..design.value_count as usize {
        let id = manifest.variables[i].id;
        let v = design.values[i];
        match id {
            variable::FIN_ROOT_SCALE
            | variable::FIN_SPAN_SCALE
            | variable::FIN_TIP_SCALE
            | variable::FIN_SWEEP_SCALE
            | variable::BALLAST_MASS_Q21
            | variable::BALLAST_POSITION_Q28
            | variable::BODY_LENGTH_SCALE
            | variable::BODY_DIAMETER_SCALE
            | variable::OGIVE_FINENESS_Q16 => {}
            variable::DROGUE_CDA_SCALE => {
                r.vehicle.drogue_cda = ksa64_core::phase8_numeric::SpatialArea::from_raw(scale(
                    r.vehicle.drogue_cda.raw(),
                    v,
                ))
            }
            variable::MAIN_CDA_SCALE => {
                r.vehicle.main_cda = ksa64_core::phase8_numeric::SpatialArea::from_raw(scale(
                    r.vehicle.main_cda.raw(),
                    v,
                ))
            }
            variable::MAIN_ALTITUDE_Q13 => {
                r.mission.main_deployment_altitude =
                    ksa64_core::phase8_numeric::SpatialPosition::from_raw(v)
            }
            variable::DROGUE_INFLATION_SCALE => {
                r.mission.drogue_inflation_time = ksa64_core::phase8_numeric::SpatialTime::from_raw(
                    scale(r.mission.drogue_inflation_time.raw(), v),
                )
            }
            variable::MAIN_INFLATION_SCALE => {
                r.mission.main_inflation_time = ksa64_core::phase8_numeric::SpatialTime::from_raw(
                    scale(r.mission.main_inflation_time.raw(), v),
                )
            }
            variable::RAIL_LENGTH_Q13 => {
                r.mission.rail_length = ksa64_core::phase8_numeric::SpatialPosition::from_raw(v)
            }
            variable::GIMBAL_TRAVEL_Q16 => r.capability.gimbal_limit_q16_deg = v,
            variable::GIMBAL_SLEW_Q16 => r.capability.slew_q16_deg_per_s = v,
            variable::GIMBAL_LAG => r.capability.lag_releases = v as u8,
            variable::ACTUATOR_MASS_Q21 => {
                let delta = v.saturating_sub(r.capability.actuator_mass_q21);
                r.capability.actuator_mass_q21 = v;
                apply_point_mass(&mut r.vehicle, delta, r.capability.pivot_from_nose_q28)
            }
            variable::PROPORTIONAL_GAIN_Q15 => r.capability.proportional_gain_q15 = v,
            variable::DERIVATIVE_GAIN_Q15 => r.capability.derivative_gain_q15 = v,
            _ => {}
        }
    }
    let identity = design.materialized_ids[0];
    r.vehicle.identity = identity;
    r.mission.identity = design.materialized_ids[1];
    r.mission.vehicle_identity = identity;
    r.capability.identity = design.materialized_ids[3];
    r.capability.vehicle_identity = identity;
    r.avionics.identity = design.materialized_ids[2];
    if !r.vehicle.is_valid() || !r.mission.is_valid() {
        return Err(Phase9EvaluationError::Candidate);
    }
    Ok(r)
}

fn extract_metrics(
    summary: &ksa64_core::phase8_5_contract::AvionicsEvaluationSummary,
    vehicle_mass: i32,
    settling_error: i16,
) -> ([i32; CASE_METRIC_COUNT], u16) {
    let mut values = [0; CASE_METRIC_COUNT];
    // Avionics metrics occupy stable internal indices 8..11.
    values[0] = summary.physical.metrics[MetricSlot::ApogeeAltitude as usize];
    values[1] = summary.physical.metrics[MetricSlot::LandingDistance as usize];
    values[2] = summary.physical.metrics[MetricSlot::RailExitVelocity as usize];
    values[3] = summary.physical.metrics[MetricSlot::RailExitStaticMargin as usize]
        .min(summary.physical.metrics[MetricSlot::BurnoutStaticMargin as usize]);
    values[4] = summary.physical.metrics[MetricSlot::ImpactVelocity as usize];
    values[5] = summary.physical.metrics[MetricSlot::MaxMach as usize];
    values[6] = summary.physical.metrics[MetricSlot::MaximumAngleOfAttack as usize];
    values[7] = vehicle_mass;
    values[8] = i32::from(summary.max_attitude_error_turn16.unsigned_abs());
    values[9] = i32::from(summary.saturation_count);
    values[10] = i32::from(
        summary.deployment_feedback
            & (ksa64_core::phase8_mission::EVENT_DROGUE | ksa64_core::phase8_mission::EVENT_MAIN)
            == (ksa64_core::phase8_mission::EVENT_DROGUE | ksa64_core::phase8_mission::EVENT_MAIN),
    );
    values[11] = i32::from(summary.alarms & (8 | 32) != 0);
    values[12] = i32::from(settling_error.unsigned_abs());
    (values, 0x1fff)
}
fn metric_index(id: u16) -> Option<usize> {
    match id {
        metric::APOGEE => Some(0),
        metric::LANDING_DISTANCE => Some(1),
        metric::RAIL_EXIT_VELOCITY => Some(2),
        metric::MIN_STATIC_MARGIN => Some(3),
        metric::IMPACT_VELOCITY => Some(4),
        metric::MAX_MACH => Some(5),
        metric::MAX_AOA => Some(6),
        metric::DRY_MASS => Some(7),
        metric::ATTITUDE_ERROR => Some(8),
        metric::SATURATION_COUNT => Some(9),
        metric::DEPLOYMENT_ACK => Some(10),
        metric::ALARMS => Some(11),
        metric::SETTLING_ERROR => Some(12),
        _ => None,
    }
}

pub fn evaluate_candidate(
    manifest: &SearchManifest,
    design: &DesignVector,
    study: StudyId,
    tier: u8,
) -> Result<CandidateEvaluation, Phase9EvaluationError> {
    if !matches!(tier, 1 | 8 | 64) {
        return Err(Phase9EvaluationError::Configuration);
    }
    let base = materialize(manifest, design, study)?;
    let settling_error = if study.gimbal() {
        five_mps_crosswind_settling_error(
            &base.vehicle,
            &base.motor,
            base.mission,
            base.avionics,
            base.capability,
        )
        .unwrap_or(i16::MAX)
    } else {
        0
    };
    let mut cases = Vec::with_capacity(tier as usize);
    for run in 0..u32::from(tier) {
        let spatial = derive_spatial_uncertainty(
            SpatialCampaignConfig {
                master_seed: PHASE9_SEED,
                run_count: 64,
            },
            run,
        );
        let (mission, wind, physical) =
            materialize_spatial_case(base.mission, &base.wind, spatial, run);
        let (avionics, actuator, acrc) = derive_avionics_case(run);
        let mut cap = base.capability;
        if run != 0 && study.gimbal() {
            cap.lag_releases = (i32::from(cap.lag_releases) + actuator[0]).clamp(0, 8) as u8;
            cap.slew_q16_deg_per_s = scale(cap.slew_q16_deg_per_s, actuator[1]);
            cap.proportional_gain_q15 = scale(cap.proportional_gain_q15, actuator[2]);
            cap.derivative_gain_q15 = scale(cap.derivative_gain_q15, actuator[2]);
            cap.gimbal_limit_q16_deg = scale(cap.gimbal_limit_q16_deg, actuator[3])
        }
        let sum = evaluate_with_avionics(AvionicsEvaluationRequest {
            vehicle: &base.vehicle,
            motor: &base.motor,
            mission,
            wind: &wind,
            variation: physical,
            variation_checksum: spatial.checksum.rotate_left(7) ^ acrc,
            avionics: base.avionics,
            capability: cap,
            uncertainty_case: avionics,
        })
        .map_err(|_| Phase9EvaluationError::World)?;
        let (metrics, valid) = extract_metrics(&sum, base.vehicle.dry_mass.raw(), settling_error);
        let mut bytes = [0u8; 64];
        bytes[0] = sum.physical.outcome as u8;
        bytes[4..8].copy_from_slice(&sum.physical_summary_identity.to_le_bytes());
        for (i, c) in sum.checksum_chains.iter().enumerate() {
            bytes[8 + i * 4..12 + i * 4].copy_from_slice(&c.to_le_bytes())
        }
        let mut kas8 = [0; KAS8_LENGTH];
        write_avionics_summary(sum, &mut kas8).map_err(|_| Phase9EvaluationError::World)?;
        cases.push(CaseEvidence {
            outcome: sum.physical.outcome,
            metrics,
            valid,
            checksum: crc32_ieee(&bytes),
            kas8,
        })
    }
    let aggregate = aggregate_candidate(manifest, design, &cases)?;
    Ok(CandidateEvaluation { aggregate, cases })
}

fn aggregate_value(
    cases: &[CaseEvidence],
    index: usize,
    kind: AggregateId,
) -> Result<i32, Phase9EvaluationError> {
    let mut values = Vec::with_capacity(cases.len());
    for c in cases {
        values.push(c.metric(index).ok_or(Phase9EvaluationError::Metric)?)
    }
    match kind {
        AggregateId::Nominal => Ok(values[0]),
        AggregateId::Minimum => Ok(*values.iter().min().unwrap()),
        AggregateId::Maximum => Ok(*values.iter().max().unwrap()),
        AggregateId::Mean => {
            let sum: i128 = values.iter().map(|v| i128::from(*v)).sum();
            let n = values.len() as i128;
            let sign = if sum < 0 { -1 } else { 1 };
            Ok(
                ((sum + sign * (n / 2)) / n).clamp(i128::from(i32::MIN), i128::from(i32::MAX))
                    as i32,
            )
        }
        AggregateId::Quantile95 => {
            values.sort_unstable();
            let rank = (95 * values.len()).div_ceil(100).max(1) - 1;
            Ok(values[rank])
        }
        AggregateId::FailureRate => {
            let failed = cases
                .iter()
                .filter(|c| {
                    !matches!(
                        c.outcome,
                        EvaluationOutcome::Complete | EvaluationOutcome::GroundContact
                    )
                })
                .count() as i128;
            Ok(((failed * 1_000_000 + cases.len() as i128 / 2) / cases.len() as i128) as i32)
        }
    }
}
fn aggregate_candidate(
    m: &SearchManifest,
    d: &DesignVector,
    cases: &[CaseEvidence],
) -> Result<CandidateAggregate, Phase9EvaluationError> {
    let mut objectives = [0; MAX_METRICS];
    for (i, output) in objectives
        .iter_mut()
        .enumerate()
        .take(m.objective_count as usize)
    {
        *output = aggregate_value(
            cases,
            metric_index(m.objectives[i].metric_id).ok_or(Phase9EvaluationError::Metric)?,
            m.objectives[i].aggregate,
        )?
    }
    let mut values = [0; MAX_CONSTRAINT_RESULTS];
    let mut violated = 0u8;
    let mut normalized = 0i128;
    for (i, output) in values
        .iter_mut()
        .enumerate()
        .take(m.constraint_count as usize)
    {
        let spec = m.constraints[i];
        let value = aggregate_value(
            cases,
            metric_index(spec.metric_id).ok_or(Phase9EvaluationError::Metric)?,
            spec.aggregate,
        )?;
        *output = value;
        let delta = match spec.op {
            ConstraintOp::AtLeast => (i128::from(spec.threshold) - i128::from(value)).max(0),
            ConstraintOp::AtMost => (i128::from(value) - i128::from(spec.threshold)).max(0),
            ConstraintOp::Equal => (i128::from(value) - i128::from(spec.threshold)).abs(),
        };
        if delta > 0 {
            violated = violated.saturating_add(1);
            normalized = normalized
                .saturating_add(delta.saturating_mul(1_000_000) / i128::from(spec.scale.max(1)))
        }
    }
    let fatal = cases
        .iter()
        .map(|c| fatal_class(c.outcome))
        .max()
        .unwrap_or(0);
    let feasible = fatal == 0 && violated == 0;
    let mut crc_bytes = Vec::with_capacity(cases.len() * 4);
    for c in cases {
        crc_bytes.extend_from_slice(&c.checksum.to_le_bytes())
    }
    Ok(CandidateAggregate {
        identity: 0,
        manifest_identity: m.identity,
        candidate_identity: d.identity,
        uncertainty_tier: cases.len() as u8,
        case_count: cases.len() as u8,
        fatal_class: fatal,
        violated_constraints: violated,
        feasible,
        case_crc: crc32_ieee(&crc_bytes),
        normalized_violation: normalized.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        objective_count: m.objective_count,
        constraint_count: m.constraint_count,
        objectives,
        constraint_values: values,
    }
    .seal())
}
fn fatal_class(o: EvaluationOutcome) -> u8 {
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

pub fn collapse_duplicates(vectors: Vec<DesignVector>) -> (Vec<DesignVector>, Vec<usize>) {
    let mut first = BTreeMap::new();
    let mut unique = Vec::new();
    let mut refs = Vec::with_capacity(vectors.len());
    for v in vectors {
        let key = (
            v.values[..v.value_count as usize].to_vec(),
            v.materialized_ids,
        );
        let index = *first.entry(key).or_insert_with(|| {
            let i = unique.len();
            unique.push(v);
            i
        });
        refs.push(index)
    }
    (unique, refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn built_ins_validate_and_baselines_materialize() {
        for study in [
            StudyId::PassiveRecovery,
            StudyId::GimbalControl,
            StudyId::Coupled,
            StudyId::ExperimentalAirframe,
        ] {
            let m = built_in_manifest(study, SearchEngineId::Nsga2V1, SearchPresetId::Quick);
            m.validate().unwrap();
            let d = baseline_vector(&m);
            d.validate_against(&m).unwrap();
            materialize(&m, &d, study).unwrap();
        }
    }
    #[test]
    fn exact_nominal_candidate_is_repeatable() {
        let m = built_in_manifest(
            StudyId::GimbalControl,
            SearchEngineId::Nsga2V1,
            SearchPresetId::Quick,
        );
        let d = baseline_vector(&m);
        let a = evaluate_candidate(&m, &d, StudyId::GimbalControl, 1).unwrap();
        let b = evaluate_candidate(&m, &d, StudyId::GimbalControl, 1).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.aggregate.case_count, 1)
    }
    #[test]
    fn duplicate_collapse_is_first_occurrence_ordered() {
        let m = built_in_manifest(
            StudyId::PassiveRecovery,
            SearchEngineId::GridV1,
            SearchPresetId::Quick,
        );
        let a = baseline_vector(&m);
        let (u, r) = collapse_duplicates(vec![a, a]);
        assert_eq!(u.len(), 1);
        assert_eq!(r, vec![0, 0])
    }
}
