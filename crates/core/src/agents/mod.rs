//! Agent definitions and launch planning (M5).
//!
//! Everything here is the *generic PTY adapter*: an agent is an executable + argument array run in
//! a PTY. No agent-specific protocol is assumed (that is M6, per verified capability). The only
//! capability advertised is `pty_launch`; process state is the only status we can know.

mod adapter;
mod behavior;
mod def;
mod search;
mod store;
mod version;

pub use adapter::{
    adapter, all_adapters, detect_presence, AdapterInfo, CapabilityInfo, ConfigPath, Feature,
    Level, Presence, Scope,
};
pub use behavior::{
    adapter_for, agent_adapters, AgentAdapter, McpFileFormat, McpTarget, SkillCompat, SkillFacts,
};
pub use def::{
    launch_action, plan_launch, validate_custom, AgentCapability, AgentDef, AgentKind, CustomAgent,
    EnvVar, LaunchPlan, PRESETS,
};
pub use search::{find_executable, search_dirs};
pub use store::{migrate_legacy_approvals, AgentStore};
pub use version::detect_version;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("{0}")]
    Invalid(String),
    #[error("unknown agent")]
    Unknown,
    #[error("{name} was not found. Install it or choose its executable.")]
    NotFound { name: String },
    #[error("review required: confirm the command before launching")]
    ReviewRequired,
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<crate::persistence::DbError> for AgentError {
    fn from(e: crate::persistence::DbError) -> Self {
        Self::Storage(e.to_string())
    }
}
