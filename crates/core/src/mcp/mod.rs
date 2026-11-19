//! MCP integration manager core (M7, ADR 0015).
//!
//! * [`model`]: server definition, validation, fingerprint, secret placeholders.
//! * [`secrets`]: macOS Keychain storage (values never touch SQLite or agent config files).
//! * [`store`]: the registry in SQLite plus approval.
//! * [`probe`]: Test Connection (MCP handshake + `tools/list`) for approved stdio servers.
//! * [`sync`]: dry-run / approve / backup / apply / rollback into agent config files that M6
//!   verified; user-owned entries are never overwritten.
//! * [`catalog`]: a small static list of well-known servers (no marketplace).

pub mod catalog;
pub mod model;
pub mod probe;
pub mod secrets;
pub mod store;
pub mod sync;

pub use model::{validate_input, McpInput, McpServer, Pair, Transport};
pub use secrets::{KeychainStore, MemoryStore, SecretStore};
pub use store::McpStore;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("{0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error("approval required: review and approve this server first")]
    NotApproved,
    #[error("this server changed since you reviewed it; review it again")]
    ApprovalStale,
    #[error("secret storage error: {0}")]
    Secret(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("{0}")]
    Sync(String),
}

impl From<crate::persistence::DbError> for McpError {
    fn from(e: crate::persistence::DbError) -> Self {
        Self::Storage(e.to_string())
    }
}
impl From<rusqlite::Error> for McpError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}
