//! Shared host application facade used by the ksa64 command and Phase 12.
//!
//! This layer owns product discovery and orchestration only. Accepted domain
//! adapters live in focused modules; simulation, flight software, campaigns,
//! optimization, and strict evidence parsers remain their sole authorities.

use crate::phase11_authoring::AuthoringError;
use crate::product::{ExperienceDescriptor, ProductCatalog, SupportedAction, TargetDescriptor};
use crate::workspace_model::AcceptedProductCatalog;
use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

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
pub enum MissionApplicationRequest {
    Run(MissionRequest),
    Control(MissionRequest),
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
pub enum ProjectRequest {
    Lint {
        source: String,
    },
    Compile {
        source: String,
        output: PathBuf,
    },
    Run {
        source: String,
        output: PathBuf,
        scripted: bool,
    },
    Control {
        source: String,
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvidenceRequest {
    Inspect { artifact: PathBuf },
    Verify { artifact: PathBuf },
    Replay { artifact: PathBuf },
    Debrief { session: PathBuf, output: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetRequest {
    Build { id: String },
    VerifyStored { id: String },
    ProbeLive { id: String, live: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRequest {
    pub phase: String,
    pub live_vice: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRequest {
    Project(ProjectRequest),
    Mission(MissionApplicationRequest),
    Campaign(CampaignRequest),
    Optimization(OptimizationRequest),
    Evidence(EvidenceRequest),
    Target(TargetRequest),
    Audit(AuditRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationPermission {
    ReadOnly,
    WorkspaceWrite,
    ExternalProcess,
    LiveTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancellationBoundary {
    Immediate,
    MissionRelease,
    CampaignRun,
    OptimizationGeneration,
    ExternalProcess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ApplicationRequestPolicy {
    pub permission: ApplicationPermission,
    pub cancellation: CancellationBoundary,
    pub explicit_confirmation_required: bool,
}

impl ApplicationRequest {
    pub const fn policy(&self) -> ApplicationRequestPolicy {
        use ApplicationPermission as Permission;
        use CancellationBoundary as Boundary;
        match self {
            Self::Project(ProjectRequest::Lint { .. })
            | Self::Evidence(EvidenceRequest::Inspect { .. })
            | Self::Evidence(EvidenceRequest::Verify { .. })
            | Self::Evidence(EvidenceRequest::Replay { .. })
            | Self::Target(TargetRequest::VerifyStored { .. }) => ApplicationRequestPolicy {
                permission: Permission::ReadOnly,
                cancellation: Boundary::Immediate,
                explicit_confirmation_required: false,
            },
            Self::Project(ProjectRequest::Compile { .. })
            | Self::Evidence(EvidenceRequest::Debrief { .. }) => ApplicationRequestPolicy {
                permission: Permission::WorkspaceWrite,
                cancellation: Boundary::Immediate,
                explicit_confirmation_required: false,
            },
            Self::Project(ProjectRequest::Run { .. })
            | Self::Project(ProjectRequest::Control { .. })
            | Self::Mission(_) => ApplicationRequestPolicy {
                permission: Permission::WorkspaceWrite,
                cancellation: Boundary::MissionRelease,
                explicit_confirmation_required: false,
            },
            Self::Campaign(_) => ApplicationRequestPolicy {
                permission: Permission::WorkspaceWrite,
                cancellation: Boundary::CampaignRun,
                explicit_confirmation_required: false,
            },
            Self::Optimization(_) => ApplicationRequestPolicy {
                permission: Permission::WorkspaceWrite,
                cancellation: Boundary::OptimizationGeneration,
                explicit_confirmation_required: false,
            },
            Self::Target(TargetRequest::Build { .. })
            | Self::Audit(AuditRequest {
                live_vice: false, ..
            }) => ApplicationRequestPolicy {
                permission: Permission::ExternalProcess,
                cancellation: Boundary::ExternalProcess,
                explicit_confirmation_required: false,
            },
            Self::Target(TargetRequest::ProbeLive { .. })
            | Self::Audit(AuditRequest {
                live_vice: true, ..
            }) => ApplicationRequestPolicy {
                permission: Permission::LiveTarget,
                cancellation: Boundary::ExternalProcess,
                explicit_confirmation_required: true,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ksa64Application {
    accepted_products: AcceptedProductCatalog,
    workspace: PathBuf,
}

impl Default for Ksa64Application {
    fn default() -> Self {
        Self::new(workspace_root())
    }
}

impl Ksa64Application {
    pub fn new(workspace: PathBuf) -> Self {
        let accepted_products = AcceptedProductCatalog::frozen();
        let catalog = accepted_products.catalog();
        debug_assert!(catalog.validate().is_ok());
        debug_assert!(catalog.validate_assets(&workspace).is_ok());
        Self {
            accepted_products,
            workspace,
        }
    }

    pub const fn catalog(&self) -> ProductCatalog {
        self.accepted_products.catalog()
    }

    pub const fn accepted_products(&self) -> AcceptedProductCatalog {
        self.accepted_products
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn execute(
        &self,
        request: ApplicationRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        match request {
            ApplicationRequest::Project(request) => match request {
                ProjectRequest::Lint { source } => self.lint_project(&source),
                ProjectRequest::Compile { source, output } => {
                    self.compile_project(&source, &output)
                }
                ProjectRequest::Run {
                    source,
                    output,
                    scripted,
                } => self.run_project(&source, &output, scripted),
                ProjectRequest::Control { source, output } => {
                    self.control_project(&source, output.as_deref())
                }
            },
            ApplicationRequest::Mission(request) => match request {
                MissionApplicationRequest::Run(request) => self.run_mission(&request),
                MissionApplicationRequest::Control(request) => self.mission_control(request),
            },
            ApplicationRequest::Campaign(request) => self.run_campaign(&request),
            ApplicationRequest::Optimization(request) => self.run_optimization(&request),
            ApplicationRequest::Evidence(request) => match request {
                EvidenceRequest::Inspect { artifact } => self.inspect_evidence(&artifact),
                EvidenceRequest::Verify { artifact } => self.verify_evidence(&artifact),
                EvidenceRequest::Replay { artifact } => self.replay_evidence(&artifact),
                EvidenceRequest::Debrief { session, output } => {
                    self.debrief_evidence(&session, &output)
                }
            },
            ApplicationRequest::Target(request) => match request {
                TargetRequest::Build { id } => self.build_target(&id),
                TargetRequest::VerifyStored { id } => self.verify_target_stored(&id),
                TargetRequest::ProbeLive { id, live } => self.probe_target_live(&id, live),
            },
            ApplicationRequest::Audit(request) => self.run_audit(&request.phase, request.live_vice),
        }
    }

    pub fn experience(&self, id: &str) -> Result<&'static ExperienceDescriptor, ApplicationError> {
        self.catalog().experience(id).ok_or_else(|| {
            ApplicationError::not_found(
                "catalog.experience-not-found",
                format!("unknown experience '{id}'"),
            )
            .with_hint("run ksa64 catalog list to see current experience IDs")
        })
    }

    pub fn target(&self, id: &str) -> Result<&'static TargetDescriptor, ApplicationError> {
        self.catalog().target(id).ok_or_else(|| {
            ApplicationError::not_found(
                "catalog.target-not-found",
                format!("unknown target '{id}'"),
            )
            .with_hint("run ksa64 target list to see target IDs")
        })
    }

    pub fn run_optimization(
        &self,
        request: &OptimizationRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Optimize)?;
        crate::optimization_app::run_product_optimization(request)
    }
}

pub(crate) fn require_action(
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
                format!("'{}' does not support {action:?}", descriptor.id),
            )
        })
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ApplicationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io_error("filesystem.create-directory"))?;
    }
    fs::write(path, bytes).map_err(io_error("filesystem.write"))
}

pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), ApplicationError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    write_file(path, &bytes)
}

pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, ApplicationError> {
    fs::read(path).map_err(io_error("filesystem.read"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host crate has workspace parent")
        .to_path_buf()
}

pub(crate) fn authoring_error(
    operation: &'static str,
) -> impl FnOnce(AuthoringError) -> ApplicationError {
    move |error| {
        ApplicationError::invalid(
            "project.authoring",
            format!("{operation} failed: {error:?}"),
        )
    }
}

pub(crate) fn codec_error<E: std::fmt::Debug>(
    operation: &'static str,
) -> impl FnOnce(E) -> ApplicationError {
    move |error| {
        ApplicationError::integrity(
            "artifact.codec",
            format!("{operation} rejected frozen input: {error:?}"),
        )
    }
}

pub(crate) fn debug_error<E: std::fmt::Debug>(
    operation: &'static str,
) -> impl FnOnce(E) -> ApplicationError {
    move |error| {
        ApplicationError::execution(
            "application.execution",
            format!("{operation} failed: {error:?}"),
        )
    }
}

pub(crate) fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> ApplicationError {
    move |error| {
        ApplicationError::execution("application.io", format!("{operation} failed: {error}"))
    }
}

pub(crate) fn json_error(error: serde_json::Error) -> ApplicationError {
    ApplicationError::execution(
        "application.json",
        format!("JSON operation failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookup_diagnostic_is_stable() {
        let application = Ksa64Application::default();
        let error = application.experience("missing").unwrap_err();
        assert_eq!(error.diagnostic.code, "catalog.experience-not-found");
        assert_eq!(application.catalog().experiences.len(), 13);
    }

    #[test]
    fn request_policy_keeps_live_and_external_work_explicit() {
        let stored = ApplicationRequest::Target(TargetRequest::VerifyStored {
            id: "c64.ksa-g10r.safehold".into(),
        });
        assert_eq!(stored.policy().permission, ApplicationPermission::ReadOnly);
        assert!(!stored.policy().explicit_confirmation_required);

        let live = ApplicationRequest::Target(TargetRequest::ProbeLive {
            id: "c64.ksa-g10r.safehold".into(),
            live: true,
        });
        assert_eq!(live.policy().permission, ApplicationPermission::LiveTarget);
        assert!(live.policy().explicit_confirmation_required);
        assert_eq!(
            live.policy().cancellation,
            CancellationBoundary::ExternalProcess
        );
    }

    #[test]
    fn request_policy_exposes_safe_queue_boundaries() {
        let mission = ApplicationRequest::Mission(MissionApplicationRequest::Run(MissionRequest {
            id: "firestorm.vertical".into(),
            scenario: None,
            role: None,
            display: MissionDisplay::None,
            pace: MissionPace::Fast,
            scripted: true,
            output: None,
        }));
        assert_eq!(
            mission.policy().cancellation,
            CancellationBoundary::MissionRelease
        );
        let campaign = ApplicationRequest::Campaign(CampaignRequest {
            id: "firestorm.vertical".into(),
            runs: 1,
            workers: 1,
            output: "campaign".into(),
        });
        assert_eq!(
            campaign.policy().cancellation,
            CancellationBoundary::CampaignRun
        );
    }
}
