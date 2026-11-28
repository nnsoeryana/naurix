//! One finding as Naurix shows it (ADR 0032 §3). Cleaned, bounded, and without any code snippet.

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_TOOL_CHARS: usize = 100;
pub const MAX_RULE_CHARS: usize = 200;
pub const MAX_PATH_CHARS: usize = 1024;
pub const MAX_MESSAGE_CHARS: usize = 500;

/// Most severe first, so sorting ascending lists critical findings at the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    /// From a `security-severity` score (0–10, a GitHub code-scanning convention). `None` when the
    /// value is not a finite number in range.
    pub fn from_score(score: f64) -> Option<Self> {
        if !score.is_finite() || !(0.0..=10.0).contains(&score) {
            return None;
        }
        Some(match score {
            s if s >= 9.0 => Self::Critical,
            s if s >= 7.0 => Self::High,
            s if s >= 4.0 => Self::Medium,
            s if s > 0.0 => Self::Low,
            _ => Self::Info,
        })
    }
}

/// SARIF `level`. A missing level is `warning`, as the SARIF 2.1.0 default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Error,
    Warning,
    Note,
    None,
}

impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "note" => Some(Self::Note),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    /// Severity when the report gives no score.
    pub fn severity(self) -> Severity {
        match self {
            Self::Error => Severity::High,
            Self::Warning => Severity::Medium,
            Self::Note => Severity::Low,
            Self::None => Severity::Info,
        }
    }
}

/// Where a finding is, relative to the workspace root (always `/`-separated).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub path: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub tool: String,
    pub rule_id: String,
    pub severity: Severity,
    pub level: Level,
    pub location: Option<Location>,
    pub message: String,
    /// Stable across line shifts; used to compare two reports (ADR 0032 §5).
    pub fingerprint: String,
}

/// Control characters other than newline and tab removed, trimmed, and at most `max` characters
/// (a cut string ends with `…`).
pub fn clean_text(raw: &str, max: usize) -> String {
    let s: String = raw
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Length-prefixed SHA-256 over the parts (as `approval::fingerprint`), so different part lists
/// never collide by concatenation.
pub fn fingerprint(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_bands() {
        assert_eq!(Severity::from_score(9.8), Some(Severity::Critical));
        assert_eq!(Severity::from_score(9.0), Some(Severity::Critical));
        assert_eq!(Severity::from_score(7.0), Some(Severity::High));
        assert_eq!(Severity::from_score(4.0), Some(Severity::Medium));
        assert_eq!(Severity::from_score(0.1), Some(Severity::Low));
        assert_eq!(Severity::from_score(0.0), Some(Severity::Info));
        assert_eq!(Severity::from_score(10.5), None);
        assert_eq!(Severity::from_score(-1.0), None);
        assert_eq!(Severity::from_score(f64::NAN), None);
    }

    #[test]
    fn clean_text_strips_controls_and_cuts() {
        assert_eq!(clean_text(" a\u{1b}[31mb\tc\n ", 10), "a[31mb\tc");
        assert_eq!(clean_text("abcdef", 4), "abc…");
        assert_eq!(clean_text("abcd", 4), "abcd");
    }

    #[test]
    fn fingerprint_parts_do_not_collide_by_concatenation() {
        assert_ne!(fingerprint(&["ab", "c"]), fingerprint(&["a", "bc"]));
    }
}
