//! PTY process management (M2). No Tauri dependency (ADR 0002).
//!
//! * [`SpawnSpec`] describes a launch with an executable + argument array. There is no shell-string
//!   construction anywhere in this module.
//! * [`PtySession`] owns one child process on a PTY.
//! * [`Supervisor`] is the registry of live sessions and owns shutdown semantics.
//!
//! Terminal output is never stored or logged here; it is only forwarded over a bounded channel.

mod forward;
mod session;
mod spec;
mod supervisor;

pub use forward::{forward, Flow, Outbound, HIGH_WATER_BYTES, MAX_BATCH_BYTES};
pub use session::{ExitInfo, PtySession, SessionEvent, TerminateOutcome};
pub use spec::{resolve_executable, SpawnSpec, MAX_DIMENSION, MAX_INPUT_BYTES};
pub use supervisor::{SessionId, Supervisor};

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("executable not found or not executable: {0}")]
    ExecutableNotFound(String),
    #[error("working directory does not exist or is not a directory: {0}")]
    BadWorkingDirectory(String),
    #[error("failed to start process: {0}")]
    Spawn(String),
    #[error("session not found")]
    NotFound,
    #[error("session has already exited")]
    Exited,
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
