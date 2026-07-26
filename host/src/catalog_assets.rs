//! Workspace-backed validation for noncanonical product metadata.

use crate::product::{ApplicationService, ProductCatalog};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogAssetError {
    Missing(PathBuf),
    MissingAdapter(&'static str),
}

impl ProductCatalog {
    pub fn validate_assets(&self, workspace: &Path) -> Result<(), CatalogAssetError> {
        for experience in self.experiences {
            if !service_has_adapter(experience.service) {
                return Err(CatalogAssetError::MissingAdapter(experience.id));
            }
        }
        for target in self.targets {
            for relative in [
                target.build_owner,
                target.live_probe_owner,
                target.stored_evidence,
            ] {
                require_file(workspace, relative)?;
            }
        }
        for historical in self.historical {
            require_file(workspace, historical.audit_script)?;
        }
        Ok(())
    }
}

const fn service_has_adapter(service: ApplicationService) -> bool {
    match service {
        ApplicationService::VerticalMission
        | ApplicationService::SpatialMission
        | ApplicationService::LocalAvionics
        | ApplicationService::AdvancedCanard
        | ApplicationService::AdvancedRcs
        | ApplicationService::AdvancedMixed
        | ApplicationService::PassiveDesignStudy
        | ApplicationService::ControlStudy
        | ApplicationService::AdvancedEffectorStudy
        | ApplicationService::GlobalMission
        | ApplicationService::MissionOperations
        | ApplicationService::SafeholdRecovery
        | ApplicationService::Ksa5aOrbitCoast => true,
    }
}

fn require_file(workspace: &Path, relative: &str) -> Result<(), CatalogAssetError> {
    let path = workspace.join(relative);
    if path.is_file() {
        Ok(())
    } else {
        Err(CatalogAssetError::Missing(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_catalog_references_existing_workspace_assets() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        ProductCatalog::accepted()
            .validate_assets(workspace)
            .unwrap();
    }
}
