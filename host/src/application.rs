//! Shared host application facade used by the `ksa64` command and Phase 12.
//!
//! This layer owns product discovery and orchestration only.  Accepted
//! simulation, flight-software, campaign, optimization, and evidence modules
//! remain the sole authorities for their domains.

use crate::phase10::{encode_kra10, run_global_campaign, validate_kra10, GlobalFixtureSet};
use crate::phase10_mission::{
    capture_nominal_global_mission, mission_json, write_global_mission_artifacts,
};
use crate::phase10_tui::{run_global_console, GlobalConsoleConfig, GlobalConsolePace};
use crate::phase11_authoring::{
    build_definition_bundle, compile_project_source, complete_project_session, inspect_bundle,
    lint_project_source, project_from_bundle, replay_completed_session, verify_session,
    write_debrief_reports, AuthoringError, CompletedMissionSession,
};
use crate::phase11_tui::run_operations_console;
use crate::phase7::{capture_hobby_mission, telemetry_frame_count};
use crate::phase7_campaign::{encode_kra7, run_hobby_campaign};
use crate::phase7_plot::build_stock_kph7;
use crate::phase8::{run_checked_in_phase8, run_checked_in_phase8_crosswind};
use crate::phase8_5::run_host_host;
use crate::phase8_5_campaign::{encode_phase85_campaign, run_phase85_campaign};
use crate::phase8_5_tui::{run_local_console, ConsolePace, LocalConsoleConfig};
use crate::phase8_campaign::{encode_kra8, run_spatial_campaign};
use crate::phase9_5_workbench::{
    baseline_advanced_vector, built_in_advanced_manifest, evaluate_advanced_candidate,
    run_advanced_campaign, AdvancedStudyId,
};
use crate::product::{
    ApplicationService, ExperienceDescriptor, ProductCatalog, SupportedAction, TargetDescriptor,
};
use ksa64_core::evaluation::MetricSlot;
use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KSC7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_core::phase8_format::{KMC8_LENGTH, KMP8_LENGTH, KSC8_LENGTH, KVP8_LENGTH, KWP8_LENGTH};
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack,
};
use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase10_corroboration::coast_frozen_ksa5_one_orbit;
use ksa64_sim::phase7_campaign::{encode_ksc7, HobbyCampaignConfig, HobbyDesignVector};
use ksa64_sim::phase8_campaign::{encode_ksc8, SpatialCampaignConfig, SPATIAL_REFERENCE_SEED};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const PHASE7_VEHICLE: &[u8; KVP7_LENGTH] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const PHASE7_MOTOR: &[u8; KMP7_LENGTH] =
    include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const PHASE7_MISSION: &[u8; KMC7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm-i211.kmc7");
const PHASE8_VEHICLE: &[u8; KVP8_LENGTH] = include_bytes!("../../phase8/examples/firestorm54.kvp8");
const PHASE8_MOTOR: &[u8; KMP8_LENGTH] =
    include_bytes!("../../phase8/examples/aerotech-i211w.kmp8");
const PHASE8_MISSION: &[u8; KMC8_LENGTH] =
    include_bytes!("../../phase8/examples/firestorm-i211.kmc8");
const PHASE8_WIND: &[u8; KWP8_LENGTH] = include_bytes!("../../phase8/examples/firestorm-calm.kwp8");
const GNSS_LOSS_SOURCE: &str = include_str!("../../phase11/examples/gnss-loss.json");
const SAFEHOLD_SOURCE: &str = include_str!("../../phase11/examples/safehold-recovery.json");

pub const APPLICATION_SCHEMA: &str = "ksa64.application-outcome.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticKind {
    Usage,
    NotFound,
    Unsupported,
    InvalidInput,
    Integrity,
    ToolUnavailable,
    Execution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ApplicationDiagnostic {
    pub kind: DiagnosticKind,
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationError {
    pub exit_code: u8,
    pub diagnostic: ApplicationDiagnostic,
}

impl ApplicationError {
    pub fn usage(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(2, DiagnosticKind::Usage, code, message)
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(3, DiagnosticKind::NotFound, code, message)
    }

    pub fn unsupported(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(4, DiagnosticKind::Unsupported, code, message)
    }

    pub fn invalid(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(5, DiagnosticKind::InvalidInput, code, message)
    }

    pub fn integrity(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(6, DiagnosticKind::Integrity, code, message)
    }

    pub fn execution(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(7, DiagnosticKind::Execution, code, message)
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.diagnostic.hint = Some(hint.into());
        self
    }

    fn new(
        exit_code: u8,
        kind: DiagnosticKind,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            exit_code,
            diagnostic: ApplicationDiagnostic {
                kind,
                code,
                message: message.into(),
                hint: None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApplicationOutcome {
    pub schema: &'static str,
    pub operation: String,
    pub summary: String,
    pub identity: Option<String>,
    pub artifacts: Vec<String>,
    pub details: Value,
}

impl ApplicationOutcome {
    pub fn new(operation: impl Into<String>, summary: impl Into<String>, details: Value) -> Self {
        Self {
            schema: APPLICATION_SCHEMA,
            operation: operation.into(),
            summary: summary.into(),
            identity: None,
            artifacts: Vec::new(),
            details,
        }
    }

    pub fn identity(mut self, value: u32) -> Self {
        self.identity = Some(format!("0x{value:08x}"));
        self
    }

    pub fn artifact(mut self, path: &Path) -> Self {
        self.artifacts.push(path.display().to_string());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionDisplay {
    Tui,
    Summary,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionPace {
    Fast,
    Realtime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionRequest {
    pub id: String,
    pub scenario: Option<String>,
    pub role: Option<String>,
    pub display: MissionDisplay,
    pub pace: MissionPace,
    pub scripted: bool,
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignRequest {
    pub id: String,
    pub runs: u32,
    pub workers: usize,
    pub output: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimizationRequest {
    pub id: String,
    pub engine: SearchEngineId,
    pub preset: SearchPresetId,
    pub workers: usize,
    pub output: PathBuf,
    pub tui: bool,
    pub resume: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRequest {
    Mission(MissionRequest),
    Campaign(CampaignRequest),
    Optimization(OptimizationRequest),
    EvidenceInspect(PathBuf),
    EvidenceVerify(PathBuf),
    EvidenceReplay(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Ksa64Application {
    catalog: ProductCatalog,
    workspace: PathBuf,
}

impl Default for Ksa64Application {
    fn default() -> Self {
        Self::new(workspace_root())
    }
}

impl Ksa64Application {
    pub fn new(workspace: PathBuf) -> Self {
        let catalog = ProductCatalog::accepted();
        debug_assert!(catalog.validate().is_ok());
        debug_assert!(catalog.validate_assets(&workspace).is_ok());
        Self { catalog, workspace }
    }

    pub const fn catalog(&self) -> ProductCatalog {
        self.catalog
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn execute(
        &self,
        request: ApplicationRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        match request {
            ApplicationRequest::Mission(request) => self.run_mission(&request),
            ApplicationRequest::Campaign(request) => self.run_campaign(&request),
            ApplicationRequest::Optimization(request) => self.run_optimization(&request),
            ApplicationRequest::EvidenceInspect(path) => self.inspect_evidence(&path),
            ApplicationRequest::EvidenceVerify(path) => self.verify_evidence(&path),
            ApplicationRequest::EvidenceReplay(path) => self.replay_evidence(&path),
        }
    }

    pub fn experience(&self, id: &str) -> Result<&'static ExperienceDescriptor, ApplicationError> {
        self.catalog.experience(id).ok_or_else(|| {
            ApplicationError::not_found(
                "catalog.experience-not-found",
                format!("unknown experience `{id}`"),
            )
            .with_hint("run `ksa64 catalog list` to see current experience IDs")
        })
    }

    pub fn target(&self, id: &str) -> Result<&'static TargetDescriptor, ApplicationError> {
        self.catalog.target(id).ok_or_else(|| {
            ApplicationError::not_found(
                "catalog.target-not-found",
                format!("unknown target `{id}`"),
            )
            .with_hint("run `ksa64 target list` to see target IDs")
        })
    }

    pub fn lint_project(&self, source: &str) -> Result<ApplicationOutcome, ApplicationError> {
        let project = lint_project_source(source).map_err(authoring_error("project.lint"))?;
        Ok(ApplicationOutcome::new(
            "project.lint",
            format!(
                "valid project `{}` using {} for {}",
                project.name, project.package, project.scenario
            ),
            serde_json::to_value(project).map_err(json_error)?,
        ))
    }

    pub fn compile_project(
        &self,
        source: &str,
        output: &Path,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let project = compile_project_source(source).map_err(authoring_error("project.compile"))?;
        let bundle =
            build_definition_bundle(&project).map_err(authoring_error("project.compile"))?;
        write_file(output, &bundle)?;
        Ok(ApplicationOutcome::new(
            "project.compile",
            format!(
                "compiled definition 0x{:08x} ({} bytes)",
                project.definition_identity,
                bundle.len()
            ),
            json!({
                "definition_identity": format!("0x{:08x}", project.definition_identity),
                "bytes": bundle.len(),
            }),
        )
        .identity(project.definition_identity)
        .artifact(output))
    }

    pub fn run_project(
        &self,
        source: &str,
        output: &Path,
        scripted: bool,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let project = compile_project_source(source).map_err(authoring_error("project.run"))?;
        let completed =
            complete_project_session(&project, scripted).map_err(authoring_error("project.run"))?;
        write_file(output, &completed.bundle)?;
        Ok(session_outcome(
            if scripted {
                "project.script"
            } else {
                "project.run"
            },
            &completed,
            Some(output),
        ))
    }

    pub fn control_project(
        &self,
        source: &str,
        output: Option<&Path>,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let project = compile_project_source(source).map_err(authoring_error("project.control"))?;
        let completed = run_operations_console(&project).map_err(debug_error("project.control"))?;
        if let Some(path) = output {
            write_file(path, &completed.bundle)?;
        }
        Ok(session_outcome("project.control", &completed, output))
    }

    pub fn run_mission(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Run)?;
        match descriptor.service {
            ApplicationService::VerticalMission => self.run_vertical(request),
            ApplicationService::SpatialMission => self.run_spatial(request),
            ApplicationService::LocalAvionics => self.run_local_avionics(request),
            ApplicationService::AdvancedCanard => {
                self.run_advanced_mission(request, AdvancedStudyId::Canard)
            }
            ApplicationService::AdvancedRcs => {
                self.run_advanced_mission(request, AdvancedStudyId::Rcs)
            }
            ApplicationService::AdvancedMixed => {
                self.run_advanced_mission(request, AdvancedStudyId::Mixed)
            }
            ApplicationService::GlobalMission => self.run_global(request),
            ApplicationService::MissionOperations => self.run_operations(request, false),
            ApplicationService::SafeholdRecovery => self.run_operations(request, true),
            ApplicationService::Ksa5aOrbitCoast => self.run_ksa5a_coast(request),
            _ => Err(ApplicationError::unsupported(
                "mission.not-runnable",
                format!("`{}` is a workbench, not a mission", request.id),
            )),
        }
    }

    pub fn mission_control(
        &self,
        mut request: MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::MissionControl)?;
        request.display = MissionDisplay::Tui;
        self.run_mission(&request)
    }

    pub fn run_campaign(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs == 0 || request.workers == 0 {
            return Err(ApplicationError::invalid(
                "campaign.invalid-budget",
                "campaign runs and workers must both be nonzero",
            ));
        }
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Campaign)?;
        fs::create_dir_all(&request.output).map_err(io_error("campaign.output"))?;
        match descriptor.service {
            ApplicationService::VerticalMission => self.campaign_vertical(request),
            ApplicationService::SpatialMission => self.campaign_spatial(request),
            ApplicationService::LocalAvionics => self.campaign_avionics(request),
            ApplicationService::AdvancedCanard => {
                self.campaign_advanced(request, AdvancedStudyId::Canard)
            }
            ApplicationService::AdvancedRcs => {
                self.campaign_advanced(request, AdvancedStudyId::Rcs)
            }
            ApplicationService::AdvancedMixed => {
                self.campaign_advanced(request, AdvancedStudyId::Mixed)
            }
            ApplicationService::GlobalMission => self.campaign_global(request),
            _ => Err(ApplicationError::unsupported(
                "campaign.unsupported",
                format!("`{}` has no campaign adapter", request.id),
            )),
        }
    }

    pub fn run_optimization(
        &self,
        request: &OptimizationRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Optimize)?;
        crate::optimization_app::run_product_optimization(request)
    }

    pub fn inspect_evidence(&self, path: &Path) -> Result<ApplicationOutcome, ApplicationError> {
        let bytes = read_file(path)?;
        if bytes.starts_with(b"KSB1") {
            let report = inspect_bundle(&bytes).map_err(authoring_error("evidence.inspect"))?;
            return Ok(ApplicationOutcome::new(
                "evidence.inspect",
                format!("inspected Phase 11 session bundle ({} bytes)", bytes.len()),
                report,
            )
            .artifact(path));
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            return Ok(ApplicationOutcome::new(
                "evidence.inspect",
                format!("inspected JSON evidence ({} bytes)", bytes.len()),
                value,
            )
            .artifact(path));
        }
        let magic = bytes
            .get(0..8)
            .map(|prefix| String::from_utf8_lossy(prefix).to_string())
            .unwrap_or_default();
        Ok(ApplicationOutcome::new(
            "evidence.inspect",
            format!(
                "recognized bounded binary evidence container ({} bytes)",
                bytes.len()
            ),
            json!({
                "bytes": bytes.len(),
                "prefix": magic,
                "crc32": format!("0x{:08x}", crc32_ieee(&bytes)),
                "strict_parser": "use the owning experience or historical audit",
            }),
        )
        .artifact(path))
    }

    pub fn verify_evidence(&self, path: &Path) -> Result<ApplicationOutcome, ApplicationError> {
        let bytes = read_file(path)?;
        if bytes.starts_with(b"KSB1") {
            let scan = verify_session(&bytes).map_err(authoring_error("evidence.verify"))?;
            return Ok(ApplicationOutcome::new(
                "evidence.verify",
                format!(
                    "verified completed session 0x{:08x}; {} segments",
                    scan.identity.completed_evidence,
                    scan.segments.len()
                ),
                json!({
                    "completed_evidence": format!("0x{:08x}", scan.identity.completed_evidence),
                    "segments": scan.segments.len(),
                }),
            )
            .identity(scan.identity.completed_evidence)
            .artifact(path));
        }
        if serde_json::from_slice::<Value>(&bytes).is_ok() {
            return Ok(ApplicationOutcome::new(
                "evidence.verify",
                format!(
                    "verified syntactically valid JSON evidence ({} bytes)",
                    bytes.len()
                ),
                json!({
                    "bytes": bytes.len(),
                    "crc32": format!("0x{:08x}", crc32_ieee(&bytes)),
                    "scope": "JSON syntax and byte identity",
                }),
            )
            .artifact(path));
        }
        Err(ApplicationError::unsupported(
            "evidence.owner-required",
            format!(
                "{} requires its owning strict parser or historical audit",
                path.display()
            ),
        )
        .with_hint("use `ksa64 audit run PHASE` for frozen non-session evidence"))
    }

    pub fn replay_evidence(&self, path: &Path) -> Result<ApplicationOutcome, ApplicationError> {
        let bytes = read_file(path)?;
        let replay =
            replay_completed_session(&bytes).map_err(authoring_error("evidence.replay"))?;
        Ok(ApplicationOutcome::new(
            "evidence.replay",
            format!(
                "exact replay 0x{:08x}; flight 0x{:08x}; nav 0x{:08x}",
                replay.evidence_identity, replay.flight_checksum, replay.navigation_checksum
            ),
            json!({
                "evidence_identity": format!("0x{:08x}", replay.evidence_identity),
                "flight_checksum": format!("0x{:08x}", replay.flight_checksum),
                "navigation_checksum": format!("0x{:08x}", replay.navigation_checksum),
                "command_checksum": format!("0x{:08x}", replay.command_checksum),
            }),
        )
        .identity(replay.evidence_identity)
        .artifact(path))
    }

    pub fn debrief_evidence(
        &self,
        path: &Path,
        output: &Path,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let bytes = read_file(path)?;
        let project = project_from_bundle(&bytes).map_err(authoring_error("evidence.debrief"))?;
        let completed = complete_project_session(&project, true)
            .map_err(authoring_error("evidence.debrief"))?;
        if completed.bundle != bytes {
            return Err(ApplicationError::integrity(
                "evidence.replay-mismatch",
                "session replay differs; refusing derived debrief",
            ));
        }
        write_debrief_reports(&completed, output).map_err(authoring_error("evidence.debrief"))?;
        Ok(ApplicationOutcome::new(
            "evidence.debrief",
            "wrote deterministic debrief reports",
            json!({
                "evidence_identity": format!("0x{:08x}", completed.evidence.evidence_identity),
            }),
        )
        .identity(completed.evidence.evidence_identity)
        .artifact(output))
    }

    fn run_vertical(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "firestorm.vertical")?;
        ensure_scenario(request, &["nominal"])?;
        let vehicle = parse_vehicle_pack(PHASE7_VEHICLE).map_err(codec_error("phase7.vehicle"))?;
        let motor = parse_motor_pack(PHASE7_MOTOR).map_err(codec_error("phase7.motor"))?;
        let mission = parse_mission_pack(PHASE7_MISSION).map_err(codec_error("phase7.mission"))?;
        let capture = capture_hobby_mission(vehicle, &motor, mission)
            .map_err(debug_error("mission.vertical"))?;
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm vertical mission: {:?}; {} telemetry frames",
                capture.evaluation.outcome,
                telemetry_frame_count(&capture)
            ),
            json!({
                "experience": request.id,
                "scenario": "nominal",
                "outcome": format!("{:?}", capture.evaluation.outcome),
                "frames": telemetry_frame_count(&capture),
                "apogee_raw": capture.evaluation.metric(MetricSlot::ApogeeAltitude),
                "impact_velocity_raw": capture.evaluation.metric(MetricSlot::ImpactVelocity),
                "checksum": format!("0x{:08x}", capture.evaluation.source_checksums[0]),
            }),
        );
        if let Some(output) = &request.output {
            fs::create_dir_all(output).map_err(io_error("mission.output"))?;
            let telemetry = output.join("firestorm-i211.kst7");
            let summary = output.join("firestorm-i211.ksr7");
            let plot = output.join("firestorm-i211.kph7");
            write_file(&telemetry, &capture.telemetry)?;
            write_file(&summary, &capture.summary_record)?;
            let plot_bytes =
                build_stock_kph7(&capture.telemetry).map_err(debug_error("mission.plot"))?;
            write_file(&plot, &plot_bytes)?;
            outcome = outcome
                .artifact(&telemetry)
                .artifact(&summary)
                .artifact(&plot);
        }
        Ok(outcome)
    }

    fn run_spatial(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "firestorm.spatial")?;
        let scenario = scenario(request, "calm");
        let evidence = match scenario {
            "calm" => run_checked_in_phase8(),
            "crosswind5" => run_checked_in_phase8_crosswind(5),
            _ => return Err(bad_scenario(request, &["calm", "crosswind5"])),
        }
        .map_err(debug_error("mission.spatial"))?;
        let details = serde_json::to_value(&evidence).map_err(json_error)?;
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm spatial mission `{scenario}`: {:?}",
                evidence.outcome
            ),
            details.clone(),
        );
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_local_avionics(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let scenario = scenario(request, "monitor");
        let gimbal = match scenario {
            "monitor" => false,
            "gimbal" => true,
            _ => return Err(bad_scenario(request, &["monitor", "gimbal"])),
        };
        let evidence = if request.display == MissionDisplay::Tui {
            run_local_console(LocalConsoleConfig {
                gimbal,
                pace: match request.pace {
                    MissionPace::Fast => ConsolePace::Fast,
                    MissionPace::Realtime => ConsolePace::Realtime,
                },
                title: format!("KSA64 // {}", request.id),
            })
            .map_err(debug_error("mission.local-console"))?
        } else {
            run_host_host(gimbal, None).map_err(debug_error("mission.local-avionics"))?
        };
        let details = json!({
            "experience": request.id,
            "scenario": scenario,
            "placement": format!("{:?}", evidence.placement),
            "outcome": format!("{:?}", evidence.summary.physical.outcome),
            "releases": evidence.releases,
            "checksum_chains": evidence.summary.checksum_chains,
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm avionics `{scenario}`: {:?}",
                evidence.summary.physical.outcome
            ),
            details.clone(),
        );
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_advanced_mission(
        &self,
        request: &MissionRequest,
        study: AdvancedStudyId,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.display == MissionDisplay::Tui {
            return Err(ApplicationError::unsupported(
                "mission.advanced-tui",
                "advanced-effector live Mission Control currently uses the Phase 9.5 bridge",
            )
            .with_hint(
                "run with `--display summary`, or use the documented split-endpoint launcher",
            ));
        }
        ensure_scenario(request, &["nominal"])?;
        let manifest = built_in_advanced_manifest(study, SearchEngineId::Nsga2V1);
        let vector = baseline_advanced_vector(&manifest);
        let evidence = evaluate_advanced_candidate(&manifest, &vector, study, 1)
            .map_err(debug_error("mission.advanced"))?;
        let details = json!({
            "experience": request.id,
            "study": format!("{study:?}"),
            "manifest_identity": format!("0x{:08x}", manifest.identity),
            "candidate_identity": format!("0x{:08x}", vector.identity),
            "feasible": evidence.aggregate.feasible,
            "objectives": evidence.aggregate.objectives,
            "constraints": evidence.aggregate.constraint_values,
            "case_count": evidence.cases.len(),
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "{} nominal evaluation: feasible={}",
                request.id, evidence.aggregate.feasible
            ),
            details.clone(),
        )
        .identity(vector.identity);
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_global(&self, request: &MissionRequest) -> Result<ApplicationOutcome, ApplicationError> {
        ensure_scenario(request, &["nominal"])?;
        let capture = if request.display == MissionDisplay::Tui {
            run_global_console(GlobalConsoleConfig {
                title: "KSA64 // KSA-G10R GLOBAL MISSION CONTROL".into(),
                pace: match request.pace {
                    MissionPace::Fast => GlobalConsolePace::Fast,
                    MissionPace::Realtime => GlobalConsolePace::Realtime,
                },
                auto_exit: false,
            })
            .map_err(debug_error("mission.global-console"))?
        } else {
            if request.pace == MissionPace::Realtime {
                return Err(ApplicationError::unsupported(
                    "mission.realtime-without-display",
                    "realtime pacing requires the live Mission Control display",
                ));
            }
            capture_nominal_global_mission(|_| {}).map_err(debug_error("mission.global"))?
        };
        let details = mission_json(&capture);
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "KSA-G10R global mission: {:?}; {} releases",
                capture.summary.common.outcome, capture.releases
            ),
            details,
        )
        .identity(ksa64_core::phase10_telemetry::global_evaluation_identity(
            &capture.summary,
        ));
        if let Some(output) = &request.output {
            write_global_mission_artifacts(&capture, output)
                .map_err(|message| ApplicationError::execution("mission.output", message))?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_operations(
        &self,
        request: &MissionRequest,
        safehold: bool,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let source = if safehold {
            if scenario(request, "safehold-recovery") != "safehold-recovery" {
                return Err(bad_scenario(request, &["safehold-recovery"]));
            }
            SAFEHOLD_SOURCE.to_owned()
        } else {
            operation_source(
                scenario(request, "gnss-loss"),
                request.role.as_deref().unwrap_or("guided-operator"),
            )?
        };
        let project =
            compile_project_source(&source).map_err(authoring_error("mission.operations"))?;
        let completed = if request.display == MissionDisplay::Tui {
            run_operations_console(&project).map_err(debug_error("mission.operations-console"))?
        } else {
            complete_project_session(&project, request.scripted)
                .map_err(authoring_error("mission.operations"))?
        };
        if let Some(output) = &request.output {
            write_file(output, &completed.bundle)?;
        }
        Ok(session_outcome(
            "mission.run",
            &completed,
            request.output.as_deref(),
        ))
    }

    fn run_ksa5a_coast(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "ksa-5a.orbit-coast")?;
        ensure_scenario(request, &["nominal"])?;
        let fixtures = GlobalFixtureSet::embedded();
        let summary = coast_frozen_ksa5_one_orbit(&fixtures.earth, &fixtures.transforms)
            .map_err(debug_error("mission.ksa5a-coast"))?;
        let details = json!({
            "handoff_identity": format!("0x{:08x}", summary.handoff.identity),
            "phase5_summary_checksum": format!("0x{:08x}", summary.handoff.phase5_summary_checksum),
            "duration_q16": summary.duration_q16,
            "steps": summary.steps,
            "terminal_position_q12_km": summary.terminal_position_q12_km,
            "terminal_velocity_q24_km_s": summary.terminal_velocity_q24_km_s,
            "minimum_altitude_q12_km": summary.minimum_altitude_q12_km,
            "maximum_altitude_q12_km": summary.maximum_altitude_q12_km,
            "checksum": format!("0x{:08x}", summary.checksum),
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "KSA-5A one-orbit corroboration: {} steps; checksum 0x{:08x}",
                summary.steps, summary.checksum
            ),
            details.clone(),
        )
        .identity(summary.checksum);
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn campaign_vertical(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let vehicle = parse_vehicle_pack(PHASE7_VEHICLE).map_err(codec_error("phase7.vehicle"))?;
        let motor = parse_motor_pack(PHASE7_MOTOR).map_err(codec_error("phase7.motor"))?;
        let mission = parse_mission_pack(PHASE7_MISSION).map_err(codec_error("phase7.mission"))?;
        let config = HobbyCampaignConfig {
            master_seed: 0x4b53_4137,
            run_count: request.runs,
        };
        let campaign = run_hobby_campaign(
            vehicle,
            motor,
            mission,
            HobbyDesignVector::NOMINAL,
            config,
            request.workers,
        );
        let mut ksc = [0; KSC7_LENGTH];
        encode_ksc7(config, &mut ksc).map_err(debug_error("campaign.ksc7"))?;
        let ksc_path = request
            .output
            .join(format!("campaign-{}.ksc7", request.runs));
        let kra_path = request
            .output
            .join(format!("campaign-{}.kra7", request.runs));
        write_file(&ksc_path, &ksc)?;
        let archive = encode_kra7(&campaign);
        write_file(&kra_path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} Firestorm vertical runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "aggregate": format!("{:?}", campaign.aggregate),
                "archive_crc32": format!("0x{:08x}", crc32_ieee(&archive)),
            }),
        )
        .artifact(&ksc_path)
        .artifact(&kra_path))
    }

    fn campaign_spatial(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let vehicle =
            parse_spatial_vehicle_pack(PHASE8_VEHICLE).map_err(codec_error("phase8.vehicle"))?;
        let motor = parse_spatial_motor_pack(PHASE8_MOTOR).map_err(codec_error("phase8.motor"))?;
        let mission =
            parse_spatial_mission_pack(PHASE8_MISSION).map_err(codec_error("phase8.mission"))?;
        let wind = parse_wind_profile_pack(PHASE8_WIND).map_err(codec_error("phase8.wind"))?;
        let run_count = u16::try_from(request.runs).map_err(|_| {
            ApplicationError::invalid("campaign.run-count", "spatial run count exceeds u16")
        })?;
        let config = SpatialCampaignConfig {
            master_seed: SPATIAL_REFERENCE_SEED,
            run_count: u32::from(run_count),
        };
        let campaign = run_spatial_campaign(vehicle, motor, mission, wind, config, request.workers);
        let mut ksc = [0; KSC8_LENGTH];
        encode_ksc8(config, &mut ksc).map_err(debug_error("campaign.ksc8"))?;
        let ksc_path = request
            .output
            .join(format!("campaign-{}.ksc8", request.runs));
        let kra_path = request
            .output
            .join(format!("campaign-{}.kra8", request.runs));
        let archive = encode_kra8(&campaign);
        write_file(&ksc_path, &ksc)?;
        write_file(&kra_path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} Firestorm spatial runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "aggregate": format!("{:?}", campaign.aggregate),
                "archive_crc32": format!("0x{:08x}", crc32_ieee(&archive)),
            }),
        )
        .artifact(&ksc_path)
        .artifact(&kra_path))
    }

    fn campaign_avionics(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs != 64 {
            return Err(ApplicationError::invalid(
                "campaign.fixed-run-count",
                "firestorm.avionics uses the frozen 64-run campaign",
            ));
        }
        let result =
            run_phase85_campaign(request.workers).map_err(debug_error("campaign.avionics"))?;
        let archive = encode_phase85_campaign(&result);
        let path = request.output.join("campaign-64.kas8");
        write_file(&path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            "completed frozen 64-run Firestorm avionics campaign",
            json!({
                "workers": request.workers,
                "records_crc32": format!("0x{:08x}", result.aggregate.records_crc32),
                "completed": result.aggregate.completed,
                "alarmed": result.aggregate.alarmed,
            }),
        )
        .artifact(&path))
    }

    fn campaign_advanced(
        &self,
        request: &CampaignRequest,
        study: AdvancedStudyId,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs != 64 {
            return Err(ApplicationError::invalid(
                "campaign.fixed-run-count",
                "advanced-effector studies use the frozen 64-run campaign",
            ));
        }
        let result = run_advanced_campaign(study, request.workers)
            .map_err(debug_error("campaign.advanced"))?;
        let mut bytes = Vec::with_capacity(result.config.len() + result.records.len() * 512);
        bytes.extend_from_slice(&result.config);
        for record in &result.records {
            bytes.extend_from_slice(record);
        }
        let path = request
            .output
            .join(format!("{}-64.ksc9-kas9", advanced_study_name(study)));
        write_file(&path, &bytes)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!(
                "completed frozen 64-run {} campaign",
                advanced_study_name(study)
            ),
            json!({
                "workers": request.workers,
                "records": result.records.len(),
                "crc32": format!("0x{:08x}", result.crc32),
            }),
        )
        .artifact(&path))
    }

    fn campaign_global(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let runs = u16::try_from(request.runs).map_err(|_| {
            ApplicationError::invalid("campaign.run-count", "global run count exceeds u16")
        })?;
        let fixtures = GlobalFixtureSet::embedded();
        let result = run_global_campaign(&fixtures, runs, request.workers)
            .map_err(debug_error("campaign.global"))?;
        let archive = encode_kra10(&result).map_err(debug_error("campaign.kra10"))?;
        let verified = validate_kra10(&archive).map_err(debug_error("campaign.kra10"))?;
        if verified != result.aggregate {
            return Err(ApplicationError::integrity(
                "campaign.archive-mismatch",
                "global archive aggregate changed during validation",
            ));
        }
        let stem = format!("ksa-g10r-{}", request.runs);
        let archive_path = request.output.join(format!("{stem}.kra10"));
        let config_path = request.output.join(format!("{stem}.ksc10"));
        let mut config = [0; ksa64_core::phase10_telemetry::KSC10_LENGTH];
        result
            .config
            .encode(&mut config)
            .map_err(debug_error("campaign.ksc10"))?;
        write_file(&archive_path, &archive)?;
        write_file(&config_path, &config)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} KSA-G10R global runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "summaries_crc32": format!("0x{:08x}", result.aggregate.summaries_crc32),
                "ground_contacts": result.aggregate.ground_contacts,
                "physical_recoveries": result.aggregate.physical_recoveries,
                "numeric_frame_time_faults": result.aggregate.numeric_frame_time_faults,
            }),
        )
        .artifact(&config_path)
        .artifact(&archive_path))
    }
}

fn operation_source(scenario: &str, role: &str) -> Result<String, ApplicationError> {
    if scenario == "gnss-loss" && role == "guided-operator" {
        return Ok(GNSS_LOSS_SOURCE.to_owned());
    }
    if !matches!(
        scenario,
        "nominal" | "gnss-loss" | "guidance-update" | "ground-blackout" | "invalid-operations"
    ) {
        return Err(ApplicationError::invalid(
            "mission.scenario",
            format!("unsupported operations scenario `{scenario}`"),
        ));
    }
    let role_allowed = matches!(
        role,
        "observer"
            | "guided-operator"
            | "flight-controller"
            | "flight-software-engineer"
            | "sim-director"
            | "scripted-operator"
    );
    if !role_allowed {
        return Err(ApplicationError::invalid(
            "mission.role",
            format!("unsupported operational role `{role}`"),
        ));
    }
    let source = json!({
        "schema": "ksa64.phase11.mission-project.v1",
        "name": format!("KSA-G10R {} operations", scenario),
        "scenario": scenario,
        "package": "KsaG10rReferenceOpsV1",
        "role": role,
        "definition_identity": format!("0x{:08x}", operation_definition_identity(scenario)),
        "master_seed": "0x4b5341b0",
        "hints": role == "guided-operator",
        "provenance": [{
            "kind": "accepted-model",
            "source": "KSA64 frozen Phase 10 KSA-G10R evidence",
            "identity": "0x10a00001"
        }]
    });
    serde_json::to_string_pretty(&source).map_err(json_error)
}

const fn operation_definition_identity(scenario: &str) -> u32 {
    match scenario.as_bytes() {
        b"nominal" => 0x11d1_0020,
        b"gnss-loss" => 0x11d1_0011,
        b"guidance-update" => 0x11d1_0021,
        b"ground-blackout" => 0x11d1_0022,
        b"invalid-operations" => 0x11d1_0023,
        _ => 0x11d1_00ff,
    }
}

fn session_outcome(
    operation: &str,
    completed: &CompletedMissionSession,
    output: Option<&Path>,
) -> ApplicationOutcome {
    let mut outcome = ApplicationOutcome::new(
        operation,
        format!(
            "completed evidence 0x{:08x} ({} bytes)",
            completed.evidence.evidence_identity,
            completed.bundle.len()
        ),
        json!({
            "scenario_identity": format!("0x{:08x}", completed.evidence.scenario_identity),
            "evidence_identity": format!("0x{:08x}", completed.evidence.evidence_identity),
            "releases": completed.evidence.releases,
            "flight_checksum": format!("0x{:08x}", completed.evidence.flight_checksum),
            "navigation_checksum": format!("0x{:08x}", completed.evidence.navigation_checksum),
            "command_checksum": format!("0x{:08x}", completed.evidence.command_checksum),
            "procedure_chain": format!("0x{:08x}", completed.evidence.procedure_chain),
            "action_chain": format!("0x{:08x}", completed.evidence.action_chain),
            "safe": completed.evidence.safe,
        }),
    )
    .identity(completed.evidence.evidence_identity);
    if let Some(path) = output {
        outcome = outcome.artifact(path);
    }
    outcome
}

fn advanced_study_name(study: AdvancedStudyId) -> &'static str {
    match study {
        AdvancedStudyId::Canard => "canard",
        AdvancedStudyId::Rcs => "rcs",
        AdvancedStudyId::Mixed => "mixed",
        AdvancedStudyId::Research => "research",
    }
}

fn require_action(
    descriptor: &ExperienceDescriptor,
    action: SupportedAction,
) -> Result<(), ApplicationError> {
    descriptor
        .actions
        .contains(&action)
        .then_some(())
        .ok_or_else(|| {
            ApplicationError::unsupported(
                "catalog.action-unsupported",
                format!("`{}` does not support {action:?}", descriptor.id),
            )
        })
}

fn scenario<'a>(request: &'a MissionRequest, default: &'a str) -> &'a str {
    request.scenario.as_deref().unwrap_or(default)
}

fn ensure_scenario(request: &MissionRequest, allowed: &[&str]) -> Result<(), ApplicationError> {
    let selected = request
        .scenario
        .as_deref()
        .unwrap_or_else(|| allowed.first().copied().unwrap_or("nominal"));
    allowed
        .contains(&selected)
        .then_some(())
        .ok_or_else(|| bad_scenario(request, allowed))
}

fn bad_scenario(request: &MissionRequest, allowed: &[&str]) -> ApplicationError {
    ApplicationError::invalid(
        "mission.scenario",
        format!(
            "scenario `{}` is not supported by `{}`; expected {}",
            request.scenario.as_deref().unwrap_or(""),
            request.id,
            allowed.join(", ")
        ),
    )
}

fn reject_tui(request: &MissionRequest, id: &str) -> Result<(), ApplicationError> {
    if request.display == MissionDisplay::Tui {
        Err(ApplicationError::unsupported(
            "mission.tui-unavailable",
            format!("`{id}` currently provides summary/evidence presentation, not a live TUI"),
        ))
    } else {
        Ok(())
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ApplicationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io_error("filesystem.create-directory"))?;
    }
    fs::write(path, bytes).map_err(io_error("filesystem.write"))
}

fn write_json(path: &Path, value: &Value) -> Result<(), ApplicationError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    write_file(path, &bytes)
}

fn read_file(path: &Path) -> Result<Vec<u8>, ApplicationError> {
    fs::read(path).map_err(io_error("filesystem.read"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host crate has workspace parent")
        .to_path_buf()
}

fn authoring_error(operation: &'static str) -> impl FnOnce(AuthoringError) -> ApplicationError {
    move |error| {
        ApplicationError::invalid(
            "project.authoring",
            format!("{operation} failed: {error:?}"),
        )
    }
}

fn codec_error<E: std::fmt::Debug>(operation: &'static str) -> impl FnOnce(E) -> ApplicationError {
    move |error| {
        ApplicationError::integrity(
            "artifact.codec",
            format!("{operation} rejected frozen input: {error:?}"),
        )
    }
}

fn debug_error<E: std::fmt::Debug>(operation: &'static str) -> impl FnOnce(E) -> ApplicationError {
    move |error| {
        ApplicationError::execution(
            "application.execution",
            format!("{operation} failed: {error:?}"),
        )
    }
}

fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> ApplicationError {
    move |error| {
        ApplicationError::execution("application.io", format!("{operation} failed: {error}"))
    }
}

fn json_error(error: serde_json::Error) -> ApplicationError {
    ApplicationError::execution(
        "application.json",
        format!("JSON operation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str) -> MissionRequest {
        MissionRequest {
            id: id.into(),
            scenario: None,
            role: None,
            display: MissionDisplay::None,
            pace: MissionPace::Fast,
            scripted: true,
            output: None,
        }
    }

    #[test]
    fn flagship_uses_accepted_phase11_services() {
        let application = Ksa64Application::default();
        let outcome = application
            .run_mission(&MissionRequest {
                scenario: Some("gnss-loss".into()),
                ..request("ksa-g10r.operations")
            })
            .unwrap();
        assert_eq!(outcome.operation, "mission.run");
        assert_eq!(outcome.details["releases"], 9);
        assert!(outcome.summary.contains("completed evidence"));
    }

    #[test]
    fn operations_sources_cover_every_catalog_scenario() {
        for scenario in [
            "nominal",
            "gnss-loss",
            "guidance-update",
            "ground-blackout",
            "invalid-operations",
        ] {
            let source = operation_source(scenario, "guided-operator").unwrap();
            compile_project_source(&source).unwrap();
        }
    }

    #[test]
    fn mission_workbench_mismatch_fails_cleanly() {
        let application = Ksa64Application::default();
        let error = application
            .run_mission(&request("firestorm.design"))
            .unwrap_err();
        assert_eq!(error.diagnostic.kind, DiagnosticKind::Unsupported);
    }

    #[test]
    fn catalog_lookup_diagnostic_is_stable() {
        let application = Ksa64Application::default();
        let error = application.experience("missing").unwrap_err();
        assert_eq!(error.exit_code, 3);
        assert_eq!(error.diagnostic.code, "catalog.experience-not-found");
    }
}
