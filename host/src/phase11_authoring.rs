//! Headless Phase 11 mission authoring, execution, replay, and verification SDK.

use crate::phase11_debrief::{
    build_gnss_loss_debrief, debrief_html, debrief_json, encode_debrief, DebriefEvidence,
};
use crate::phase11_scenarios::{
    run_gnss_loss_procedure, run_ground_blackout_probe, run_guidance_update_probe,
    run_invalid_operations_probe, run_nominal_operations_probe, OperationalScenarioEvidence,
};
use crate::phase11_session::{
    scan_session_bundle, verify_complete_session, SessionBundleBuilder, SessionBundleError,
    SessionBundleIdentity, SessionBundleScan, SessionSegmentKind,
};
use ksa64_flight::phase11::{ksa_g10r_reference_mission_plan, ksa_g10r_reference_ops_manifest};
use ksa64_flight::phase11_safehold::safehold_recovery_manifest;
use ksa64_interface::crc32_ieee;
use ksa64_interface::phase11::{
    write_kal11_header, write_kal11_record, write_kfs11, write_kmp11, write_kpc11, ActionLogHeader,
    ActionLogRecord, FlightSoftwarePackageManifest, OperationalRole, KAL11_HEADER_LENGTH,
    KAL11_RECORD_LENGTH, KFS11_LENGTH, KMP11_LENGTH, KPC11_LENGTH,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub const KSD11_LENGTH: usize = 256;
pub const KSD11_COMPILER_ID: u32 = 0x11d1_0001;
pub const PHASE11_REFERENCE_SOURCE_ID: u32 = 0x11d1_1001;
pub const PHASE11_SAFEHOLD_SOURCE_ID: u32 = 0x11d1_1002;

const EARTH_PACK: &[u8] = include_bytes!("../../phase10/generated/ksa-g10r.kem10");
const FRAME_PACK: &[u8] = include_bytes!("../../phase10/generated/ksa-g10r.kft10");
const ATMOSPHERE_PACK: &[u8] = include_bytes!("../../phase10/generated/ksa-g10r.kat10");
const VEHICLE_PACK: &[u8] = include_bytes!("../../phase10/generated/ksa-g10r.kgv10");
const MISSION_PACK: &[u8] = include_bytes!("../../phase10/generated/ksa-g10r.kgm10");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MissionScenario {
    Nominal = 1,
    GnssLoss = 2,
    GuidanceUpdate = 3,
    GroundBlackout = 4,
    InvalidOperations = 5,
    SafeholdRecovery = 6,
}

impl MissionScenario {
    fn parse(value: &str) -> Result<Self, AuthoringError> {
        match value {
            "nominal" => Ok(Self::Nominal),
            "gnss-loss" => Ok(Self::GnssLoss),
            "guidance-update" => Ok(Self::GuidanceUpdate),
            "ground-blackout" => Ok(Self::GroundBlackout),
            "invalid-operations" => Ok(Self::InvalidOperations),
            "safehold-recovery" => Ok(Self::SafeholdRecovery),
            _ => Err(AuthoringError::Scenario),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionPackage {
    ReferenceOps,
    SafeholdRecovery,
}

impl MissionPackage {
    fn parse(value: &str) -> Result<Self, AuthoringError> {
        match value {
            "KsaG10rReferenceOpsV1" => Ok(Self::ReferenceOps),
            "SafeholdRecoveryV1" => Ok(Self::SafeholdRecovery),
            _ => Err(AuthoringError::Package),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceSource {
    pub kind: String,
    pub source: String,
    pub identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MissionProjectSource {
    pub schema: String,
    pub name: String,
    pub scenario: String,
    pub package: String,
    pub role: String,
    pub definition_identity: String,
    pub master_seed: String,
    pub hints: bool,
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledMissionProject {
    pub source: MissionProjectSource,
    pub canonical_source: Vec<u8>,
    pub definition_identity: u32,
    pub master_seed: u32,
    pub scenario: MissionScenario,
    pub package: MissionPackage,
    pub role: OperationalRole,
    pub definition_pack: [u8; KSD11_LENGTH],
    pub package_manifest: [u8; KFS11_LENGTH],
    pub mission_plan: Option<[u8; KMP11_LENGTH]>,
    pub procedure_pack: Option<[u8; KPC11_LENGTH]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRunEvidence {
    pub scenario_identity: u32,
    pub releases: u32,
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub prediction_checksum: u32,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub rejected_loads: u16,
    pub safe: bool,
    pub evidence_identity: u32,
    pub actions: Vec<ActionLogRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedMissionSession {
    pub evidence: SessionRunEvidence,
    pub bundle: Vec<u8>,
    pub debrief: Option<DebriefEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthoringError {
    Json,
    Schema,
    Identity,
    Scenario,
    Package,
    Role,
    Compatibility,
    Provenance,
    Codec,
    Bundle(SessionBundleError),
    Missing,
    Replay,
    Io,
}

impl From<SessionBundleError> for AuthoringError {
    fn from(value: SessionBundleError) -> Self {
        Self::Bundle(value)
    }
}

pub fn lint_project_source(input: &str) -> Result<MissionProjectSource, AuthoringError> {
    let source: MissionProjectSource =
        serde_json::from_str(input).map_err(|_| AuthoringError::Json)?;
    if source.schema != "ksa64.phase11.mission-project.v1"
        || source.name.trim().is_empty()
        || source.provenance.is_empty()
    {
        return Err(AuthoringError::Schema);
    }
    let scenario = MissionScenario::parse(&source.scenario)?;
    let package = MissionPackage::parse(&source.package)?;
    let _ = role(&source.role)?;
    let definition = parse_exact_u32(&source.definition_identity)?;
    let _ = parse_exact_u32(&source.master_seed)?;
    if definition == 0
        || source.provenance.iter().any(|item| {
            item.kind.trim().is_empty()
                || item.source.trim().is_empty()
                || parse_exact_u32(&item.identity).is_err()
        })
    {
        return Err(AuthoringError::Provenance);
    }
    if matches!(package, MissionPackage::SafeholdRecovery)
        != matches!(scenario, MissionScenario::SafeholdRecovery)
    {
        return Err(AuthoringError::Compatibility);
    }
    Ok(source)
}

pub fn compile_project_source(input: &str) -> Result<CompiledMissionProject, AuthoringError> {
    let source = lint_project_source(input)?;
    let canonical_source = serde_json::to_vec(&source).map_err(|_| AuthoringError::Json)?;
    let declared_definition = parse_exact_u32(&source.definition_identity)?;
    let master_seed = parse_exact_u32(&source.master_seed)?;
    let scenario = MissionScenario::parse(&source.scenario)?;
    let package = MissionPackage::parse(&source.package)?;
    let role = role(&source.role)?;
    let manifest = match package {
        MissionPackage::ReferenceOps => ksa_g10r_reference_ops_manifest(),
        MissionPackage::SafeholdRecovery => safehold_recovery_manifest(),
    };
    let definition_identity = hash_bytes(&[
        canonical_source.as_slice(),
        &declared_definition.to_le_bytes(),
        &KSD11_COMPILER_ID.to_le_bytes(),
    ]);
    let mut definition_pack = [0; KSD11_LENGTH];
    write_definition_pack(
        &mut definition_pack,
        definition_identity,
        declared_definition,
        master_seed,
        scenario,
        manifest,
        role,
        source.hints,
        hash_bytes(&[&canonical_source]),
    );
    let mut package_manifest = [0; KFS11_LENGTH];
    write_kfs11(&manifest, &mut package_manifest).map_err(|_| AuthoringError::Codec)?;
    let (mission_plan, procedure_pack) = if package == MissionPackage::ReferenceOps {
        let plan = ksa_g10r_reference_mission_plan();
        let mut plan_bytes = [0; KMP11_LENGTH];
        write_kmp11(&plan, &mut plan_bytes).map_err(|_| AuthoringError::Codec)?;
        let procedure = crate::phase11_operations::gnss_loss_procedure_pack(plan.plan_identity);
        let mut procedure_bytes = [0; KPC11_LENGTH];
        write_kpc11(&procedure, &mut procedure_bytes).map_err(|_| AuthoringError::Codec)?;
        (Some(plan_bytes), Some(procedure_bytes))
    } else {
        (None, None)
    };
    Ok(CompiledMissionProject {
        source,
        canonical_source,
        definition_identity,
        master_seed,
        scenario,
        package,
        role,
        definition_pack,
        package_manifest,
        mission_plan,
        procedure_pack,
    })
}

pub fn build_definition_bundle(
    project: &CompiledMissionProject,
) -> Result<Vec<u8>, AuthoringError> {
    let mut builder = SessionBundleBuilder::new(SessionBundleIdentity {
        definition: project.definition_identity,
        actions: 0,
        completed_evidence: 0,
    })?;
    push_definition_segments(&mut builder, project)?;
    builder.encode().map_err(Into::into)
}

pub fn run_project(
    project: &CompiledMissionProject,
    scripted: bool,
) -> Result<SessionRunEvidence, AuthoringError> {
    let evidence = match project.scenario {
        MissionScenario::Nominal => from_operational(run_nominal_operations_probe()),
        MissionScenario::GnssLoss => from_operational(run_gnss_loss_procedure(scripted)),
        MissionScenario::GuidanceUpdate => from_operational(run_guidance_update_probe()),
        MissionScenario::GroundBlackout => from_operational(run_ground_blackout_probe()),
        MissionScenario::InvalidOperations => from_operational(run_invalid_operations_probe()),
        MissionScenario::SafeholdRecovery => {
            let value = ksa64_sim::run_safehold_probe();
            SessionRunEvidence {
                scenario_identity: PHASE11_SAFEHOLD_SOURCE_ID,
                releases: u32::from(value.releases),
                flight_checksum: value.flight_checksum,
                navigation_checksum: value.navigation_checksum,
                command_checksum: value.command_checksum,
                prediction_checksum: 0,
                procedure_chain: 0,
                journal_chain: value.journal_chain,
                action_chain: 0x811c_9dc5,
                rejected_loads: value.failures,
                safe: value.safe,
                evidence_identity: ksa64_sim::phase11_safehold_probe_signature(),
                actions: Vec::new(),
            }
        }
    };
    Ok(evidence)
}

pub fn complete_project_session(
    project: &CompiledMissionProject,
    scripted: bool,
) -> Result<CompletedMissionSession, AuthoringError> {
    if project.package == MissionPackage::ReferenceOps
        && project.scenario == MissionScenario::GnssLoss
    {
        let mut session = crate::phase11_live::LiveMissionSession::compiled(project.clone())
            .map_err(|_| AuthoringError::Compatibility)?;
        session.prepare().map_err(|_| AuthoringError::Replay)?;
        return session
            .run_scripted_to_completion()
            .map_err(|_| AuthoringError::Replay);
    }
    let evidence = run_project(project, scripted)?;
    complete_project_session_from_evidence(project, evidence)
}

pub(crate) fn complete_project_session_from_evidence(
    project: &CompiledMissionProject,
    evidence: SessionRunEvidence,
) -> Result<CompletedMissionSession, AuthoringError> {
    let action_identity = evidence.action_chain.max(1);
    let completed_identity = evidence.evidence_identity.max(1);
    let debrief = (project.scenario == MissionScenario::GnssLoss).then(|| {
        build_gnss_loss_debrief(
            project.definition_identity,
            action_identity,
            completed_identity,
        )
    });
    let mut builder = SessionBundleBuilder::new(SessionBundleIdentity {
        definition: project.definition_identity,
        actions: action_identity,
        completed_evidence: completed_identity,
    })?;
    push_definition_segments(&mut builder, project)?;
    builder.push(
        SessionSegmentKind::GroundObservations,
        evidence_payload(b"KGO1", &evidence),
    )?;
    builder.push(
        SessionSegmentKind::CanonicalTelemetry,
        evidence_payload(b"KTE1", &evidence),
    )?;
    builder.push(
        SessionSegmentKind::PredictionProducts,
        evidence_payload(b"KPR1", &evidence),
    )?;
    builder.push(
        SessionSegmentKind::ActionLog,
        encode_action_log(project, &evidence)?,
    )?;
    builder.push(
        SessionSegmentKind::PackageJournal,
        evidence_payload(b"KJR1", &evidence),
    )?;
    builder.push(
        SessionSegmentKind::ProcedureEvidence,
        evidence_payload(b"KPE1", &evidence),
    )?;

    let debrief_bytes = if let Some(value) = &debrief {
        encode_debrief(&value.summary).to_vec()
    } else {
        generic_debrief(project, &evidence)
    };
    builder.push(SessionSegmentKind::Debrief, debrief_bytes)?;
    let bundle = builder.encode()?;
    Ok(CompletedMissionSession {
        evidence,
        bundle,
        debrief,
    })
}

pub fn project_from_bundle(input: &[u8]) -> Result<CompiledMissionProject, AuthoringError> {
    let scan = scan_session_bundle(input)?;
    project_from_scan(&scan)
}

pub fn inspect_bundle(input: &[u8]) -> Result<Value, AuthoringError> {
    let scan = scan_session_bundle(input)?;
    Ok(json!({
        "schema": "ksa64.phase11.session-inspection.v1",
        "definition_identity": format!("0x{:08x}", scan.identity.definition),
        "action_identity": format!("0x{:08x}", scan.identity.actions),
        "completed_evidence_identity": format!("0x{:08x}", scan.identity.completed_evidence),
        "sealed": scan.sealed,
        "completed": scan.completed,
        "validated_bytes": scan.valid_length,
        "segments": scan.segments.iter().map(|item| format!("{:?}", item.kind)).collect::<Vec<_>>(),
        "manifest_sha256": scan.manifest_sha256.map(hex)
    }))
}

pub fn replay_completed_session(input: &[u8]) -> Result<SessionRunEvidence, AuthoringError> {
    let scan = verify_complete_session(input)?;
    let project = project_from_scan(&scan)?;
    let replay = run_project(&project, true)?;
    if replay.action_chain.max(1) != scan.identity.actions
        || replay.evidence_identity.max(1) != scan.identity.completed_evidence
    {
        return Err(AuthoringError::Replay);
    }
    Ok(replay)
}

pub fn verify_session(input: &[u8]) -> Result<SessionBundleScan, AuthoringError> {
    verify_complete_session(input).map_err(Into::into)
}

pub fn write_debrief_reports(
    completed: &CompletedMissionSession,
    directory: &Path,
) -> Result<(), AuthoringError> {
    fs::create_dir_all(directory).map_err(|_| AuthoringError::Io)?;
    let Some(debrief) = &completed.debrief else {
        return Err(AuthoringError::Missing);
    };
    let json_bytes =
        serde_json::to_vec_pretty(&debrief_json(debrief)).map_err(|_| AuthoringError::Json)?;
    fs::write(directory.join("debrief.json"), json_bytes).map_err(|_| AuthoringError::Io)?;
    fs::write(directory.join("debrief.html"), debrief_html(debrief)).map_err(|_| AuthoringError::Io)
}

fn push_definition_segments(
    builder: &mut SessionBundleBuilder,
    project: &CompiledMissionProject,
) -> Result<(), AuthoringError> {
    builder.push(
        SessionSegmentKind::SourceLedger,
        project.canonical_source.clone(),
    )?;
    let mut earth = Vec::with_capacity(EARTH_PACK.len() + FRAME_PACK.len());
    earth.extend_from_slice(EARTH_PACK);
    earth.extend_from_slice(FRAME_PACK);
    builder.push(SessionSegmentKind::EarthPack, earth)?;
    builder.push(
        SessionSegmentKind::EnvironmentPack,
        ATMOSPHERE_PACK.to_vec(),
    )?;
    builder.push(SessionSegmentKind::VehiclePack, VEHICLE_PACK.to_vec())?;
    builder.push(SessionSegmentKind::MissionPack, MISSION_PACK.to_vec())?;
    builder.push(
        SessionSegmentKind::AvionicsPack,
        project.definition_pack.to_vec(),
    )?;
    builder.push(
        SessionSegmentKind::PackageManifest,
        project.package_manifest.to_vec(),
    )?;
    if let Some(plan) = project.mission_plan {
        builder.push(SessionSegmentKind::MissionPlan, plan.to_vec())?;
    }
    if let Some(procedure) = project.procedure_pack {
        builder.push(SessionSegmentKind::ProcedurePack, procedure.to_vec())?;
    }
    builder.push(
        SessionSegmentKind::FaultSchedule,
        fault_schedule(project).to_vec(),
    )?;
    Ok(())
}

fn project_from_scan(scan: &SessionBundleScan) -> Result<CompiledMissionProject, AuthoringError> {
    let source = scan
        .segments
        .iter()
        .find(|segment| segment.kind == SessionSegmentKind::SourceLedger)
        .ok_or(AuthoringError::Missing)?;
    let text = std::str::from_utf8(&source.payload).map_err(|_| AuthoringError::Json)?;
    let project = compile_project_source(text)?;
    if project.definition_identity != scan.identity.definition {
        return Err(AuthoringError::Identity);
    }
    Ok(project)
}

fn encode_action_log(
    project: &CompiledMissionProject,
    evidence: &SessionRunEvidence,
) -> Result<Vec<u8>, AuthoringError> {
    let plan_identity = project.mission_plan.as_ref().map_or(
        safehold_recovery_manifest().mission_compatibility_identity,
        |_| ksa_g10r_reference_mission_plan().plan_identity,
    );
    let mut output = vec![0; KAL11_HEADER_LENGTH + evidence.actions.len() * KAL11_RECORD_LENGTH];
    write_kal11_header(
        &ActionLogHeader {
            session_definition_identity: project.definition_identity,
            transcript_identity: evidence.action_chain.max(1),
            package_manifest_identity: manifest(project.package).manifest_identity,
            plan_identity,
            action_count: evidence.actions.len() as u32,
            final_chain: evidence.action_chain,
            complete: true,
        },
        &mut output[..KAL11_HEADER_LENGTH],
    )
    .map_err(|_| AuthoringError::Codec)?;
    for (index, record) in evidence.actions.iter().enumerate() {
        let start = KAL11_HEADER_LENGTH + index * KAL11_RECORD_LENGTH;
        write_kal11_record(record, &mut output[start..start + KAL11_RECORD_LENGTH])
            .map_err(|_| AuthoringError::Codec)?;
    }
    Ok(output)
}

fn evidence_payload(magic: &[u8; 4], evidence: &SessionRunEvidence) -> Vec<u8> {
    let mut output = vec![0; 64];
    output[..4].copy_from_slice(magic);
    for (offset, value) in [
        (4, evidence.scenario_identity),
        (8, evidence.releases),
        (12, evidence.flight_checksum),
        (16, evidence.navigation_checksum),
        (20, evidence.command_checksum),
        (24, evidence.prediction_checksum),
        (28, evidence.procedure_chain),
        (32, evidence.journal_chain),
        (36, evidence.action_chain),
        (40, evidence.evidence_identity),
    ] {
        output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    output[44..46].copy_from_slice(&evidence.rejected_loads.to_le_bytes());
    output[46] = u8::from(evidence.safe);
    let crc = crc32_ieee(&output[..60]);
    output[60..64].copy_from_slice(&crc.to_le_bytes());
    output
}

fn generic_debrief(project: &CompiledMissionProject, evidence: &SessionRunEvidence) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema": "ksa64.phase11.generic-debrief.v1",
        "definition": format!("0x{:08x}", project.definition_identity),
        "scenario": format!("0x{:08x}", evidence.scenario_identity),
        "evidence": format!("0x{:08x}", evidence.evidence_identity),
        "classification": "direct observation and model-derived outcome",
        "causal_claim": false,
        "real_world_reliability_claim": false
    }))
    .expect("static JSON values serialize")
}

fn fault_schedule(project: &CompiledMissionProject) -> [u8; 64] {
    let mut output = [0; 64];
    output[..4].copy_from_slice(b"KFD1");
    output[4] = 11;
    output[5] = project.scenario as u8;
    output[8..12].copy_from_slice(&project.master_seed.to_le_bytes());
    output[12..16].copy_from_slice(&project.definition_identity.to_le_bytes());
    let crc = crc32_ieee(&output[..60]);
    output[60..64].copy_from_slice(&crc.to_le_bytes());
    output
}

#[allow(clippy::too_many_arguments)]
fn write_definition_pack(
    output: &mut [u8; KSD11_LENGTH],
    identity: u32,
    declared: u32,
    seed: u32,
    scenario: MissionScenario,
    package: FlightSoftwarePackageManifest,
    role: OperationalRole,
    hints: bool,
    source_identity: u32,
) {
    output.fill(0);
    output[..4].copy_from_slice(b"KSD1");
    output[4] = 11;
    output[5] = scenario as u8;
    output[6] = package.package as u8;
    output[7] = role as u8;
    for (offset, value) in [
        (8, identity),
        (12, declared),
        (16, seed),
        (20, package.manifest_identity),
        (24, package.mission_compatibility_identity),
        (28, source_identity),
        (32, KSD11_COMPILER_ID),
    ] {
        output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    output[36] = u8::from(hints);
    let crc = crc32_ieee(&output[..KSD11_LENGTH - 4]);
    output[KSD11_LENGTH - 4..].copy_from_slice(&crc.to_le_bytes());
}

pub(crate) fn from_operational(value: OperationalScenarioEvidence) -> SessionRunEvidence {
    SessionRunEvidence {
        scenario_identity: value.scenario_identity,
        releases: value.releases,
        flight_checksum: value.flight_checksum,
        navigation_checksum: value.navigation_checksum,
        command_checksum: value.command_checksum,
        prediction_checksum: value.prediction_checksum,
        procedure_chain: value.procedure_chain,
        journal_chain: value.journal_chain,
        action_chain: value.action_chain,
        rejected_loads: value.rejected_loads,
        safe: value.safe,
        evidence_identity: value.evidence_identity,
        actions: value.actions,
    }
}

fn manifest(package: MissionPackage) -> FlightSoftwarePackageManifest {
    match package {
        MissionPackage::ReferenceOps => ksa_g10r_reference_ops_manifest(),
        MissionPackage::SafeholdRecovery => safehold_recovery_manifest(),
    }
}

fn role(value: &str) -> Result<OperationalRole, AuthoringError> {
    match value {
        "observer" => Ok(OperationalRole::Observer),
        "guided-operator" => Ok(OperationalRole::GuidedOperator),
        "flight-controller" => Ok(OperationalRole::FlightController),
        "flight-software-engineer" => Ok(OperationalRole::FlightSoftwareEngineer),
        "sim-director" => Ok(OperationalRole::SimDirector),
        "scripted-operator" => Ok(OperationalRole::ScriptedOperator),
        _ => Err(AuthoringError::Role),
    }
}

fn parse_exact_u32(value: &str) -> Result<u32, AuthoringError> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).map_err(|_| AuthoringError::Identity)
    } else {
        value.parse::<u32>().map_err(|_| AuthoringError::Identity)
    }
}

fn hash_bytes(parts: &[&[u8]]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for part in parts {
        for byte in *part {
            hash = (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(scenario: &str, package: &str) -> String {
        serde_json::to_string(&json!({
            "schema": "ksa64.phase11.mission-project.v1",
            "name": "Phase 11 acceptance",
            "scenario": scenario,
            "package": package,
            "role": "scripted-operator",
            "definition_identity": "0x11d10011",
            "master_seed": "0x4b5341b0",
            "hints": false,
            "provenance": [{
                "kind": "accepted-model",
                "source": "KSA64 frozen Phase 10 evidence",
                "identity": "0x10a00001"
            }]
        }))
        .unwrap()
    }

    #[test]
    fn exact_source_compiles_to_a_sealed_definition_bundle() {
        let project =
            compile_project_source(&source("gnss-loss", "KsaG10rReferenceOpsV1")).unwrap();
        let bundle = build_definition_bundle(&project).unwrap();
        let scan = scan_session_bundle(&bundle).unwrap();
        assert!(scan.sealed);
        assert!(!scan.completed);
        assert_eq!(scan.identity.definition, project.definition_identity);
    }

    #[test]
    fn completed_session_replays_exactly_and_corruption_fails() {
        let project =
            compile_project_source(&source("gnss-loss", "KsaG10rReferenceOpsV1")).unwrap();
        let completed = complete_project_session(&project, true).unwrap();
        verify_session(&completed.bundle).unwrap();
        assert_eq!(
            replay_completed_session(&completed.bundle).unwrap(),
            completed.evidence
        );
        let mut corrupt = completed.bundle;
        corrupt[100] ^= 1;
        assert!(verify_session(&corrupt).is_err());
    }

    #[test]
    fn safehold_source_runs_the_same_accepted_portable_fixture() {
        let project =
            compile_project_source(&source("safehold-recovery", "SafeholdRecoveryV1")).unwrap();
        let evidence = run_project(&project, true).unwrap();
        assert_eq!(evidence.evidence_identity, 0xe3c5_6a95);
        assert_eq!(evidence.rejected_loads, 0);
    }

    #[test]
    fn package_scenario_mismatch_and_numeric_json_fail_closed() {
        assert_eq!(
            compile_project_source(&source("gnss-loss", "SafeholdRecoveryV1")),
            Err(AuthoringError::Compatibility)
        );
        let bad =
            source("nominal", "KsaG10rReferenceOpsV1").replace("\"0x4b5341b0\"", "1263747504");
        assert_eq!(compile_project_source(&bad), Err(AuthoringError::Json));
    }
}
