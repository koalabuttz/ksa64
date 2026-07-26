//! Product, project, and session discovery domains for Phase 12.
//!
//! Accepted built-ins, user-authored projects, and generated sessions are
//! intentionally separate types.  A project may reuse an accepted physical
//! profile without acquiring the accepted product's evidence maturity.

use crate::product::{ExperienceDescriptor, ProductCatalog};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub struct AcceptedProductCatalog {
    catalog: ProductCatalog,
}

impl AcceptedProductCatalog {
    pub const fn frozen() -> Self {
        Self {
            catalog: ProductCatalog::accepted(),
        }
    }

    pub const fn catalog(self) -> ProductCatalog {
        self.catalog
    }

    pub fn experience(self, id: &str) -> Option<&'static ExperienceDescriptor> {
        self.catalog.experience(id)
    }
}

impl Default for AcceptedProductCatalog {
    fn default() -> Self {
        Self::frozen()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectValidationState {
    Draft,
    Linted,
    Compiled,
    Evaluated,
    Compared,
    Reviewed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectDescriptor {
    pub id: String,
    pub display_name: String,
    pub model_profile: String,
    pub base_product: Option<String>,
    pub validation: ProjectValidationState,
    pub compiled_identity: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectWorkspaceError {
    EmptyId,
    DuplicateId(String),
    ReservedAcceptedId(String),
    UnknownBaseProduct(String),
}

#[derive(Clone, Debug, Default)]
pub struct ProjectWorkspace {
    projects: BTreeMap<String, ProjectDescriptor>,
}

impl ProjectWorkspace {
    pub fn insert(
        &mut self,
        project: ProjectDescriptor,
        accepted: AcceptedProductCatalog,
    ) -> Result<(), ProjectWorkspaceError> {
        if project.id.is_empty() {
            return Err(ProjectWorkspaceError::EmptyId);
        }
        if accepted.experience(&project.id).is_some() {
            return Err(ProjectWorkspaceError::ReservedAcceptedId(project.id));
        }
        if let Some(base) = project.base_product.as_deref() {
            if accepted.experience(base).is_none() {
                return Err(ProjectWorkspaceError::UnknownBaseProduct(base.to_owned()));
            }
        }
        if self.projects.contains_key(&project.id) {
            return Err(ProjectWorkspaceError::DuplicateId(project.id));
        }
        self.projects.insert(project.id.clone(), project);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ProjectDescriptor> {
        self.projects.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProjectDescriptor> {
        self.projects.values()
    }

    pub fn len(&self) -> usize {
        self.projects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SessionOrigin {
    AcceptedProduct { product_id: String },
    AuthoredProject { project_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionDescriptor {
    pub id: String,
    pub artifact: PathBuf,
    pub evidence_identity: Option<u32>,
    pub origin: SessionOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionLibraryError {
    ZeroCapacity,
    EmptyId,
    UnknownAcceptedProduct(String),
    UnknownProject(String),
}

#[derive(Clone, Debug)]
pub struct RecentSessions {
    capacity: usize,
    sessions: VecDeque<SessionDescriptor>,
}

impl RecentSessions {
    pub fn new(capacity: usize) -> Result<Self, SessionLibraryError> {
        if capacity == 0 {
            return Err(SessionLibraryError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            sessions: VecDeque::with_capacity(capacity),
        })
    }

    pub fn record(
        &mut self,
        session: SessionDescriptor,
        accepted: AcceptedProductCatalog,
        projects: &ProjectWorkspace,
    ) -> Result<(), SessionLibraryError> {
        if session.id.is_empty() {
            return Err(SessionLibraryError::EmptyId);
        }
        match &session.origin {
            SessionOrigin::AcceptedProduct { product_id } => {
                if accepted.experience(product_id).is_none() {
                    return Err(SessionLibraryError::UnknownAcceptedProduct(
                        product_id.clone(),
                    ));
                }
            }
            SessionOrigin::AuthoredProject { project_id } => {
                if projects.get(project_id).is_none() {
                    return Err(SessionLibraryError::UnknownProject(project_id.clone()));
                }
            }
        }
        if let Some(index) = self.sessions.iter().position(|item| item.id == session.id) {
            self.sessions.remove(index);
        }
        if self.sessions.len() == self.capacity {
            self.sessions.pop_front();
        }
        self.sessions.push_back(session);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SessionDescriptor> {
        self.sessions.iter()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, base: Option<&str>) -> ProjectDescriptor {
        ProjectDescriptor {
            id: id.to_owned(),
            display_name: "User mission".to_owned(),
            model_profile: "GlobalEcef6DofV1".to_owned(),
            base_product: base.map(str::to_owned),
            validation: ProjectValidationState::Draft,
            compiled_identity: None,
        }
    }

    #[test]
    fn authored_projects_cannot_masquerade_as_accepted_products() {
        let accepted = AcceptedProductCatalog::frozen();
        let mut workspace = ProjectWorkspace::default();
        assert_eq!(
            workspace.insert(project("ksa-g10r.global", None), accepted),
            Err(ProjectWorkspaceError::ReservedAcceptedId(
                "ksa-g10r.global".to_owned()
            ))
        );
        workspace
            .insert(
                project("david.research-flight", Some("ksa-g10r.global")),
                accepted,
            )
            .unwrap();
        assert_eq!(workspace.len(), 1);
        assert_eq!(accepted.catalog().experiences.len(), 13);
        assert!(accepted
            .catalog()
            .experience("david.research-flight")
            .is_none());
    }

    #[test]
    fn authored_projects_may_reference_but_not_inherit_accepted_maturity() {
        let accepted = AcceptedProductCatalog::frozen();
        let mut workspace = ProjectWorkspace::default();
        workspace
            .insert(
                project("david.global-test", Some("ksa-g10r.global")),
                accepted,
            )
            .unwrap();
        let record = workspace.get("david.global-test").unwrap();
        assert_eq!(record.model_profile, "GlobalEcef6DofV1");
        assert_eq!(record.validation, ProjectValidationState::Draft);
    }

    #[test]
    fn recent_sessions_validate_their_origin_without_merging_domains() {
        let accepted = AcceptedProductCatalog::frozen();
        let mut projects = ProjectWorkspace::default();
        projects
            .insert(project("david.ops", Some("ksa-g10r.operations")), accepted)
            .unwrap();
        let mut sessions = RecentSessions::new(2).unwrap();
        sessions
            .record(
                SessionDescriptor {
                    id: "accepted-run".to_owned(),
                    artifact: "accepted.ksb11".into(),
                    evidence_identity: Some(1),
                    origin: SessionOrigin::AcceptedProduct {
                        product_id: "ksa-g10r.operations".to_owned(),
                    },
                },
                accepted,
                &projects,
            )
            .unwrap();
        sessions
            .record(
                SessionDescriptor {
                    id: "project-run".to_owned(),
                    artifact: "project.ksb11".into(),
                    evidence_identity: Some(2),
                    origin: SessionOrigin::AuthoredProject {
                        project_id: "david.ops".to_owned(),
                    },
                },
                accepted,
                &projects,
            )
            .unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(accepted.catalog().experiences.len(), 13);
    }
}
