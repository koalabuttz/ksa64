//! Project-authoring adapters for the consolidated host application.

use crate::application::{
    authoring_error, debug_error, json_error, write_file, ApplicationError, ApplicationOutcome,
    Ksa64Application,
};
use crate::phase11_authoring::{
    build_definition_bundle, compile_project_source, complete_project_session, lint_project_source,
    CompletedMissionSession,
};
use crate::phase11_tui::run_operations_console;
use serde_json::json;
use std::path::Path;

impl Ksa64Application {
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
}

pub(crate) fn session_outcome(
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
