//! Host compatibility plus atomic KSB11 filesystem persistence.

pub use ksa64_session::phase11_session::*;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Compatibility extension for callers that historically imported
/// ksa64_host::phase11_session::* and invoked builder.write_atomic(...).
pub trait SessionBundleBuilderWriteExt {
    fn write_atomic(&self, path: &Path) -> Result<(), SessionBundleError>;
}

impl SessionBundleBuilderWriteExt for SessionBundleBuilder {
    fn write_atomic(&self, path: &Path) -> Result<(), SessionBundleError> {
        write_session_bundle_atomic(self, path)
    }
}

pub fn write_session_bundle_atomic(
    builder: &SessionBundleBuilder,
    path: &Path,
) -> Result<(), SessionBundleError> {
    let bytes = builder.encode()?;
    let temporary = temporary_path(path);
    {
        let mut file = File::create(&temporary).map_err(|_| SessionBundleError::Io)?;
        file.write_all(&bytes).map_err(|_| SessionBundleError::Io)?;
        file.sync_all().map_err(|_| SessionBundleError::Io)?;
    }
    fs::rename(temporary, path).map_err(|_| SessionBundleError::Io)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}
