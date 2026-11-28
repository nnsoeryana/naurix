//! Security Suite core (ADR 0032). No Tauri dependency (ADR 0002).
//!
//! Naurix reads a security report only when the user opens one, shows it, and forgets it: findings
//! are **never stored** (no table, no log, no event). A report is untrusted input, so the reader is
//! bounded in size and count, cleans every string, and keeps a path only when it is lexically inside
//! the workspace. Nothing here writes, starts a process or touches any file except the one report.
//!
//! * [`finding`]: the finding model, severity and fingerprint.
//! * [`sarif`]: the bounded SARIF 2.1.0 reader.
//! * [`compare`]: baseline versus current (Purple regression).

pub mod compare;
pub mod finding;
pub mod sarif;

pub use compare::{compare, Comparison};
pub use finding::{Finding, Level, Location, Severity};
pub use sarif::{parse_report, read_report, Report, MAX_REPORT_BYTES, MAX_RESULTS};

/// Errors name the problem, never the report's content.
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("The report is larger than {} MiB.", MAX_REPORT_BYTES >> 20)]
    TooLarge,
    #[error("This is not a SARIF report: {0}.")]
    NotSarif(&'static str),
    #[error("Only SARIF version 2.1.0 is supported.")]
    UnsupportedVersion,
    #[error("The report could not be read: {0}.")]
    Io(String),
}
