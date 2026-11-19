//! Agent skills manager core (M8, ADR 0016).
//!
//! Skills are **untrusted content**: they are fetched, inspected, copied and symlinked, never run.
//! One central copy lives in the app data folder; agents see it through a symlink that Naurix creates
//! only where nothing exists yet and removes only if it still points at that copy.

pub mod builtin;
pub mod frontmatter;
pub mod git;
pub mod scan;
pub mod store;

pub use git::CloneOptions;
pub use scan::{Candidate, Compat, Flag};
pub use store::{
    Inspection, LinkHealth, LinkTarget, Skill, SkillLink, SkillStore, SourceKind, UninstallReport,
    UpdatePlan,
};

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("{0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Git(String),
    #[error("file error: {0}")]
    Io(String),
    #[error("{0}")]
    Conflict(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<crate::persistence::DbError> for SkillError {
    fn from(e: crate::persistence::DbError) -> Self {
        Self::Storage(e.to_string())
    }
}
impl From<rusqlite::Error> for SkillError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}
