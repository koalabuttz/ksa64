//! Explicit target and historical-audit automation.
//!
//! Stored verification is read-only and never launches VICE.  Process
//! execution is reserved for commands whose names make that mutation clear;
//! live emulator work additionally requires an explicit flag.

use crate::application::{ApplicationError, ApplicationOutcome, Ksa64Application};
use crate::product::{HistoricalDescriptor, TargetDescriptor};
use ksa64_interface::crc32_ieee;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

impl Ksa64Application {
    pub fn verify_target_stored(&self, id: &str) -> Result<ApplicationOutcome, ApplicationError> {
        let target = self.target(id)?;
        let evidence = self.workspace().join(target.stored_evidence);
        let bytes = fs::read(&evidence).map_err(|error| {
            ApplicationError::integrity(
                "target.evidence-missing",
                format!("{}: {error}", evidence.display()),
            )
        })?;
        let parsed_json = if evidence.extension().is_some_and(|value| value == "json") {
            Some(
                serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| {
                    ApplicationError::integrity(
                        "target.evidence-json",
                        format!("{}: {error}", evidence.display()),
                    )
                })?,
            )
        } else {
            None
        };
        Ok(ApplicationOutcome::new(
            "target.verify",
            format!("verified stored evidence for `{id}`; no emulator launched"),
            json!({
                "target": target,
                "evidence": evidence.display().to_string(),
                "bytes": bytes.len(),
                "crc32": format!("0x{:08x}", crc32_ieee(&bytes)),
                "json": parsed_json,
                "live": false,
            }),
        )
        .artifact(&evidence))
    }

    pub fn build_target(&self, id: &str) -> Result<ApplicationOutcome, ApplicationError> {
        let target = self.target(id)?;
        let invocation = target_build_invocation(self.workspace(), target)?;
        run_process(&invocation)?;
        Ok(ApplicationOutcome::new(
            "target.build",
            format!("built `{id}` without starting VICE"),
            json!({
                "target": target,
                "program": invocation.program.display().to_string(),
                "arguments": invocation.arguments,
                "live": false,
            }),
        ))
    }

    pub fn probe_target_live(
        &self,
        id: &str,
        live: bool,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if !live {
            return Err(ApplicationError::usage(
                "target.live-required",
                "live target probes require the explicit `--live` flag",
            ));
        }
        let target = self.target(id)?;
        let invocation = target_live_invocation(self.workspace(), target)?;
        run_process(&invocation)?;
        Ok(ApplicationOutcome::new(
            "target.probe",
            format!("completed explicit live probe for `{id}`"),
            json!({
                "target": target,
                "program": invocation.program.display().to_string(),
                "arguments": invocation.arguments,
                "live": true,
                "policy": "one VICE instance, warp disabled, close after success or proven failure",
            }),
        ))
    }

    pub fn run_audit(
        &self,
        phase: &str,
        live_vice: bool,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.catalog().historical(phase).ok_or_else(|| {
            ApplicationError::not_found(
                "audit.phase-not-found",
                format!("unknown historical phase `{phase}`"),
            )
        })?;
        let invocation = audit_invocation(self.workspace(), descriptor, live_vice)?;
        run_process(&invocation)?;
        Ok(ApplicationOutcome::new(
            "audit.run",
            format!(
                "Phase {} audit completed{}",
                descriptor.phase,
                if live_vice {
                    " with explicit live VICE evidence"
                } else {
                    " without live VICE"
                }
            ),
            json!({
                "phase": descriptor.phase,
                "audit_script": descriptor.audit_script,
                "live_vice": live_vice,
                "program": invocation.program.display().to_string(),
                "arguments": invocation.arguments,
            }),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInvocation {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
}

fn target_build_invocation(
    workspace: &Path,
    target: &TargetDescriptor,
) -> Result<ProcessInvocation, ApplicationError> {
    if target.id == "c64.ksa-g10r.reference-ops" {
        return Ok(powershell(workspace, "phase11/c64-banked/build.ps1", &[]));
    }
    let (features, package) = match target.id {
        "c64.firestorm.vertical" => ("c64", "ksa64-core"),
        "c64.firestorm.spatial-replay" => ("c64,fixtures", "ksa64-core"),
        "c64.firestorm.advanced-flight"
        | "c64.ksa-g10r.global-flight"
        | "c64.ksa-g10r.safehold"
        | "c64.ksa-g10r.global-replay" => ("c64", target.cargo_package),
        _ => {
            return Err(ApplicationError::unsupported(
                "target.build-adapter",
                format!("no non-live build adapter for `{}`", target.id),
            ))
        }
    };
    let wrapper = workspace.join("tools/toolchains/rust-mos.ps1");
    Ok(ProcessInvocation {
        program: powershell_program(),
        arguments: vec![
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            wrapper.display().to_string(),
            "-ReturnToCaller".into(),
            "-WorkingDirectory".into(),
            ".".into(),
            "cargo".into(),
            "build".into(),
            "--profile".into(),
            "c64".into(),
            "--target".into(),
            "mos-c64-none".into(),
            "--features".into(),
            features.into(),
            "-Z".into(),
            "build-std=core".into(),
            "-Z".into(),
            "build-std-features=compiler-builtins-mem".into(),
            "-p".into(),
            package.into(),
            "--bin".into(),
            target.cargo_binary.into(),
        ],
        working_directory: workspace.to_path_buf(),
    })
}

fn target_live_invocation(
    workspace: &Path,
    target: &TargetDescriptor,
) -> Result<ProcessInvocation, ApplicationError> {
    match target.id {
        "c64.firestorm.vertical" | "c64.firestorm.spatial-replay" => {
            Ok(powershell(workspace, target.live_probe_owner, &[]))
        }
        "c64.firestorm.advanced-flight" => Ok(powershell(
            workspace,
            target.live_probe_owner,
            &["-SkipLegacy", "-RunVice"],
        )),
        "c64.ksa-g10r.global-flight" | "c64.ksa-g10r.global-replay" => Ok(powershell(
            workspace,
            target.live_probe_owner,
            &["-SkipLegacy", "-RunVice"],
        )),
        "c64.ksa-g10r.safehold" | "c64.ksa-g10r.reference-ops" => Ok(powershell(
            workspace,
            target.live_probe_owner,
            &["-SkipLegacy", "-RunVice"],
        )),
        _ => Err(ApplicationError::unsupported(
            "target.live-adapter",
            format!("no live probe adapter for `{}`", target.id),
        )),
    }
}

fn audit_invocation(
    workspace: &Path,
    descriptor: &HistoricalDescriptor,
    live_vice: bool,
) -> Result<ProcessInvocation, ApplicationError> {
    let script = workspace.join(descriptor.audit_script);
    if !script.is_file() {
        return Err(ApplicationError::integrity(
            "audit.script-missing",
            format!(
                "historical audit script does not exist: {}",
                script.display()
            ),
        ));
    }
    let arguments: Vec<&str> = match descriptor.phase {
        "5" | "6" | "7" | "8" if !live_vice => vec!["-SkipMos"],
        "8.5" | "9" | "9.5" | "10" | "11" if !live_vice => {
            vec!["-SkipLegacy", "-SkipMos"]
        }
        "8.5" | "9" | "9.5" | "10" | "11" if live_vice => {
            vec!["-SkipLegacy", "-RunVice"]
        }
        "7" | "8" if live_vice => Vec::new(),
        "0" | "1" | "2" | "3" | "4" if live_vice => {
            return Err(ApplicationError::unsupported(
                "audit.live-unsupported",
                format!(
                    "Phase {} has no explicit live VICE audit mode",
                    descriptor.phase
                ),
            ))
        }
        _ => Vec::new(),
    };
    Ok(powershell(workspace, descriptor.audit_script, &arguments))
}

fn powershell(workspace: &Path, script: &str, arguments: &[&str]) -> ProcessInvocation {
    let mut args = vec![
        "-NoProfile".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-File".into(),
        workspace.join(script).display().to_string(),
    ];
    args.extend(arguments.iter().map(|value| (*value).to_owned()));
    ProcessInvocation {
        program: powershell_program(),
        arguments: args,
        working_directory: workspace.to_path_buf(),
    }
}

fn powershell_program() -> PathBuf {
    PathBuf::from("powershell")
}

fn run_process(invocation: &ProcessInvocation) -> Result<(), ApplicationError> {
    let status = Command::new(&invocation.program)
        .args(&invocation.arguments)
        .current_dir(&invocation.working_directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| {
            ApplicationError::new_tool(
                "automation.tool-unavailable",
                format!("could not start {}: {error}", invocation.program.display()),
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ApplicationError::execution(
            "automation.failed",
            format!("{} exited with {}", invocation.program.display(), status),
        ))
    }
}

impl ApplicationError {
    fn new_tool(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            exit_code: 8,
            diagnostic: crate::application::ApplicationDiagnostic {
                kind: crate::application::DiagnosticKind::ToolUnavailable,
                code,
                message: message.into(),
                hint: Some(
                    "install/configure the documented toolchain, or use stored verification".into(),
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_verification_and_live_probe_are_distinct() {
        let app = Ksa64Application::default();
        let target = app.target("c64.ksa-g10r.global-flight").unwrap();
        assert_ne!(
            target_build_invocation(app.workspace(), target)
                .unwrap()
                .arguments,
            target_live_invocation(app.workspace(), target)
                .unwrap()
                .arguments
        );
        assert!(!target_build_invocation(app.workspace(), target)
            .unwrap()
            .arguments
            .iter()
            .any(|value| value == "-RunVice"));
        assert!(target_live_invocation(app.workspace(), target)
            .unwrap()
            .arguments
            .iter()
            .any(|value| value == "-RunVice"));
    }

    #[test]
    fn audit_default_never_requests_live_vice() {
        let app = Ksa64Application::default();
        for descriptor in app.catalog().historical {
            let invocation = audit_invocation(app.workspace(), descriptor, false).unwrap();
            assert!(!invocation.arguments.iter().any(|value| value == "-RunVice"));
        }
    }

    #[test]
    fn live_probe_requires_explicit_flag() {
        let app = Ksa64Application::default();
        let error = app
            .probe_target_live("c64.ksa-g10r.global-flight", false)
            .unwrap_err();
        assert_eq!(error.diagnostic.code, "target.live-required");
    }
}
