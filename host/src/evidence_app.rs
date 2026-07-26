//! Strict and deliberately opaque evidence adapters.

use crate::application::{
    authoring_error, read_file, ApplicationError, ApplicationOutcome, Ksa64Application,
};
use crate::phase11_authoring::{
    complete_project_session, inspect_bundle, project_from_bundle, replay_completed_session,
    verify_session, write_debrief_reports,
};
use ksa64_interface::crc32_ieee;
use serde_json::{json, Value};
use std::path::Path;

impl Ksa64Application {
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
                "inspected opaque binary artifact; owning strict parser required ({} bytes)",
                bytes.len()
            ),
            json!({
                "bytes": bytes.len(),
                "prefix": magic,
                "crc32": format!("0x{:08x}", crc32_ieee(&bytes)),
                "recognized_format": false,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn unknown_binary_is_reported_as_opaque_not_recognized() {
        let path =
            std::env::temp_dir().join(format!("ksa64-phase11-5-opaque-{}.bin", std::process::id()));
        fs::write(&path, [0x01, 0x02, 0x03, 0x04, 0xff]).unwrap();
        let outcome = Ksa64Application::default().inspect_evidence(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(outcome.summary.contains("opaque binary artifact"));
        assert!(!outcome.summary.contains("recognized bounded"));
        assert_eq!(outcome.details["recognized_format"], false);
        assert_eq!(
            outcome.details["strict_parser"],
            "use the owning experience or historical audit"
        );
    }
}
