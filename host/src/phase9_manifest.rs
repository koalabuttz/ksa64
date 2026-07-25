//! Human-readable JSON to strict KOM9 compiler.
use crate::phase9::{built_in_manifest, metric, variable, StudyId};
use ksa64_core::phase9_contract::{
    AggregateId, ConstraintOp, ConstraintSpec, Direction, ObjectiveSpec, SearchEngineId,
    SearchManifest, SearchPresetId, VariableKind, VariableSpec, MAX_CONSTRAINTS, MAX_OBJECTIVES,
    MAX_VARIABLES,
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct ManifestSource {
    pub study: String,
    pub engine: String,
    pub preset: String,
    pub master_seed: Option<u32>,
    pub budgets: Option<BudgetSource>,
    pub variables: Option<Vec<VariableSource>>,
    pub objectives: Option<Vec<ObjectiveSource>>,
    pub constraints: Option<Vec<ConstraintSource>>,
}
#[derive(Deserialize)]
pub struct BudgetSource {
    pub grid_points: Option<u16>,
    pub population: Option<u16>,
    pub generations: Option<u16>,
    pub finalists: Option<u16>,
    pub max_candidates: Option<u32>,
}
#[derive(Deserialize)]
pub struct VariableSource {
    pub id: String,
    pub kind: Option<String>,
    pub minimum: i32,
    pub maximum: i32,
    pub quantum: u32,
    pub catalogue_identity: Option<u32>,
}
#[derive(Deserialize)]
pub struct ObjectiveSource {
    pub metric: String,
    pub aggregate: String,
    pub direction: String,
}
#[derive(Deserialize)]
pub struct ConstraintSource {
    pub metric: String,
    pub aggregate: String,
    pub operator: String,
    pub threshold: i32,
    pub scale: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestCompileError {
    Json,
    Study,
    Engine,
    Preset,
    Variable,
    Metric,
    Aggregate,
    Direction,
    Operator,
    Count,
    Contract,
}
pub fn compile_manifest_json(
    input: &str,
) -> Result<(StudyId, SearchManifest), ManifestCompileError> {
    let s: ManifestSource = serde_json::from_str(input).map_err(|_| ManifestCompileError::Json)?;
    let study = parse_study(&s.study)?;
    let engine = parse_engine(&s.engine)?;
    let preset = parse_preset(&s.preset)?;
    let mut m = built_in_manifest(study, engine, preset);
    if let Some(seed) = s.master_seed {
        m.master_seed = seed
    }
    if let Some(b) = s.budgets {
        if let Some(v) = b.grid_points {
            m.budgets.grid_points = v
        }
        if let Some(v) = b.population {
            m.budgets.population = v
        }
        if let Some(v) = b.generations {
            m.budgets.generations = v
        }
        if let Some(v) = b.finalists {
            m.budgets.finalists = v
        }
        if let Some(v) = b.max_candidates {
            m.budgets.max_candidates = v
        }
    }
    if let Some(vars) = s.variables {
        if vars.is_empty() || vars.len() > MAX_VARIABLES {
            return Err(ManifestCompileError::Count);
        }
        m.variables = [VariableSpec::EMPTY; MAX_VARIABLES];
        m.variable_count = vars.len() as u8;
        for (i, v) in vars.into_iter().enumerate() {
            m.variables[i] = VariableSpec {
                id: variable_id(&v.id)?,
                kind: parse_kind(v.kind.as_deref().unwrap_or("fixed"))?,
                flags: 0,
                minimum: v.minimum,
                maximum: v.maximum,
                quantum: v.quantum,
                catalogue_id: v.catalogue_identity.unwrap_or(0),
            }
        }
    }
    if let Some(items) = s.objectives {
        if items.is_empty() || items.len() > MAX_OBJECTIVES {
            return Err(ManifestCompileError::Count);
        }
        m.objectives = [ObjectiveSpec::EMPTY; MAX_OBJECTIVES];
        m.objective_count = items.len() as u8;
        for (i, v) in items.into_iter().enumerate() {
            m.objectives[i] = ObjectiveSpec {
                metric_id: metric_id(&v.metric)?,
                aggregate: aggregate(&v.aggregate)?,
                direction: direction(&v.direction)?,
            }
        }
    }
    if let Some(items) = s.constraints {
        if items.len() > MAX_CONSTRAINTS {
            return Err(ManifestCompileError::Count);
        }
        m.constraints = [ConstraintSpec::EMPTY; MAX_CONSTRAINTS];
        m.constraint_count = items.len() as u8;
        for (i, v) in items.into_iter().enumerate() {
            m.constraints[i] = ConstraintSpec {
                metric_id: metric_id(&v.metric)?,
                aggregate: aggregate(&v.aggregate)?,
                op: operator(&v.operator)?,
                threshold: v.threshold,
                scale: v.scale,
            }
        }
    }
    m.identity = 0;
    m = m.seal().map_err(|_| ManifestCompileError::Contract)?;
    Ok((study, m))
}
fn parse_study(v: &str) -> Result<StudyId, ManifestCompileError> {
    match v {
        "study-a" | "passive-recovery" => Ok(StudyId::PassiveRecovery),
        "study-b" | "gimbal-control" => Ok(StudyId::GimbalControl),
        "coupled" => Ok(StudyId::Coupled),
        "experimental-airframe" => Ok(StudyId::ExperimentalAirframe),
        _ => Err(ManifestCompileError::Study),
    }
}
fn parse_engine(v: &str) -> Result<SearchEngineId, ManifestCompileError> {
    match v {
        "grid-v1" => Ok(SearchEngineId::GridV1),
        "nsga2-v1" => Ok(SearchEngineId::Nsga2V1),
        "de-v1" => Ok(SearchEngineId::DifferentialEvolutionV1),
        _ => Err(ManifestCompileError::Engine),
    }
}
fn parse_preset(v: &str) -> Result<SearchPresetId, ManifestCompileError> {
    match v {
        "quick" => Ok(SearchPresetId::Quick),
        "routine" => Ok(SearchPresetId::Routine),
        "accepted-balanced" => Ok(SearchPresetId::AcceptedBalanced),
        "custom" => Ok(SearchPresetId::Custom),
        _ => Err(ManifestCompileError::Preset),
    }
}
fn parse_kind(v: &str) -> Result<VariableKind, ManifestCompileError> {
    match v {
        "fixed" => Ok(VariableKind::Fixed),
        "integer" => Ok(VariableKind::Integer),
        "ordinal" => Ok(VariableKind::Ordinal),
        "catalogue" => Ok(VariableKind::Catalogue),
        "boolean" => Ok(VariableKind::Boolean),
        _ => Err(ManifestCompileError::Variable),
    }
}
fn variable_id(v: &str) -> Result<u16, ManifestCompileError> {
    match v {
        "fin-root-scale" => Ok(variable::FIN_ROOT_SCALE),
        "fin-span-scale" => Ok(variable::FIN_SPAN_SCALE),
        "fin-tip-scale" => Ok(variable::FIN_TIP_SCALE),
        "fin-sweep-scale" => Ok(variable::FIN_SWEEP_SCALE),
        "ballast-mass-q21" => Ok(variable::BALLAST_MASS_Q21),
        "ballast-position-q28" => Ok(variable::BALLAST_POSITION_Q28),
        "drogue-cda-scale" => Ok(variable::DROGUE_CDA_SCALE),
        "main-cda-scale" => Ok(variable::MAIN_CDA_SCALE),
        "main-altitude-q13" => Ok(variable::MAIN_ALTITUDE_Q13),
        "drogue-inflation-scale" => Ok(variable::DROGUE_INFLATION_SCALE),
        "main-inflation-scale" => Ok(variable::MAIN_INFLATION_SCALE),
        "rail-length-q13" => Ok(variable::RAIL_LENGTH_Q13),
        "gimbal-travel-q16" => Ok(variable::GIMBAL_TRAVEL_Q16),
        "gimbal-slew-q16" => Ok(variable::GIMBAL_SLEW_Q16),
        "gimbal-lag" => Ok(variable::GIMBAL_LAG),
        "actuator-mass-q21" => Ok(variable::ACTUATOR_MASS_Q21),
        "proportional-gain-q15" => Ok(variable::PROPORTIONAL_GAIN_Q15),
        "derivative-gain-q15" => Ok(variable::DERIVATIVE_GAIN_Q15),
        "body-length-scale" => Ok(variable::BODY_LENGTH_SCALE),
        "body-diameter-scale" => Ok(variable::BODY_DIAMETER_SCALE),
        "ogive-fineness-q16" => Ok(variable::OGIVE_FINENESS_Q16),
        _ => Err(ManifestCompileError::Variable),
    }
}
fn metric_id(v: &str) -> Result<u16, ManifestCompileError> {
    match v {
        "apogee" => Ok(metric::APOGEE),
        "landing-distance" => Ok(metric::LANDING_DISTANCE),
        "rail-exit-velocity" => Ok(metric::RAIL_EXIT_VELOCITY),
        "minimum-static-margin" => Ok(metric::MIN_STATIC_MARGIN),
        "impact-velocity" => Ok(metric::IMPACT_VELOCITY),
        "maximum-mach" => Ok(metric::MAX_MACH),
        "maximum-angle-of-attack" => Ok(metric::MAX_AOA),
        "dry-mass" => Ok(metric::DRY_MASS),
        "attitude-error" => Ok(metric::ATTITUDE_ERROR),
        "saturation-count" => Ok(metric::SATURATION_COUNT),
        "deployment-acknowledged" => Ok(metric::DEPLOYMENT_ACK),
        "fatal-avionics-alarm" => Ok(metric::ALARMS),
        "five-mps-settling-error" => Ok(metric::SETTLING_ERROR),
        _ => Err(ManifestCompileError::Metric),
    }
}
fn aggregate(v: &str) -> Result<AggregateId, ManifestCompileError> {
    match v {
        "nominal" => Ok(AggregateId::Nominal),
        "mean" => Ok(AggregateId::Mean),
        "minimum" => Ok(AggregateId::Minimum),
        "maximum" => Ok(AggregateId::Maximum),
        "quantile95" => Ok(AggregateId::Quantile95),
        "failure-rate" => Ok(AggregateId::FailureRate),
        _ => Err(ManifestCompileError::Aggregate),
    }
}
fn direction(v: &str) -> Result<Direction, ManifestCompileError> {
    match v {
        "minimize" => Ok(Direction::Minimize),
        "maximize" => Ok(Direction::Maximize),
        _ => Err(ManifestCompileError::Direction),
    }
}
fn operator(v: &str) -> Result<ConstraintOp, ManifestCompileError> {
    match v {
        "at-least" => Ok(ConstraintOp::AtLeast),
        "at-most" => Ok(ConstraintOp::AtMost),
        "equal" => Ok(ConstraintOp::Equal),
        _ => Err(ManifestCompileError::Operator),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimal_json_compiles_deterministically() {
        let source =
            r#"{"study":"study-a","engine":"grid-v1","preset":"quick","master_seed":1263747385}"#;
        let (a, x) = compile_manifest_json(source).unwrap();
        let (b, y) = compile_manifest_json(source).unwrap();
        assert_eq!(a, b);
        assert_eq!(x, y);
        assert_eq!(SearchManifest::parse(&x.encode().unwrap()).unwrap(), x)
    }
    #[test]
    fn unknown_variable_fails_closed() {
        let s = r#"{"study":"study-a","engine":"grid-v1","preset":"quick","variables":[{"id":"warp-drive","minimum":0,"maximum":1,"quantum":1}]}"#;
        assert_eq!(
            compile_manifest_json(s),
            Err(ManifestCompileError::Variable)
        )
    }
}
