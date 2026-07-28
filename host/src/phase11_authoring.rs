//! Host compatibility plus Phase 11 filesystem persistence.

pub use ksa64_session::phase11_authoring::*;

use crate::phase11_debrief::{debrief_html, debrief_json};
use std::fs;
use std::path::Path;

/// Writes the same derived reports as the historical host entrypoint while the
/// portable session crate remains free of filesystem ownership.
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
