//! Verified status signals from agents (ADR 0029, orchestration stage 7). No Tauri dependency.
//!
//! An agent reports that it is working, waiting for approval, waiting for input or finished a turn
//! by appending **one fixed word per line** to a private per-session file. Naurix follows that file.
//! The hook commands that write the words never read the agent's payload, so prompts, answers and
//! tool input (which the payloads do contain) are never read, written, stored or shown.
//!
//! Signals are agent-reported and unauthenticated: any program running as the same user could write
//! the file. They move a badge and nothing else; the approval gate never reads them.

use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Stands in for the spool path inside launch arguments. The reviewed (fingerprinted) arguments
/// contain this text, not a per-session path, so a one-time approval still matches at launch.
pub const SPOOL_PLACEHOLDER: &str = "{naurix-status-spool}";

/// A spool file larger than this stops being followed (the agent then shows no status). Real use
/// writes a few bytes per event.
pub const MAX_SPOOL_BYTES: u64 = 256 * 1024;

/// A line longer than this is garbage and is dropped without being kept in memory.
const MAX_LINE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Working,
    AwaitingApproval,
    Idle,
    TurnDone,
}

impl SignalKind {
    pub fn word(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Idle => "idle",
            Self::TurnDone => "turn_done",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        [
            Self::Working,
            Self::AwaitingApproval,
            Self::Idle,
            Self::TurnDone,
        ]
        .into_iter()
        .find(|k| k.word() == w)
    }
}

/// The shell command a hook runs to report `kind`. It ignores its standard input.
fn hook_command(kind: SignalKind) -> String {
    format!("printf '{}\\n' >> '{SPOOL_PLACEHOLDER}'", kind.word())
}

/// One `hooks` entry that runs `kind`'s command for an event, optionally only for a matcher.
fn hook_entry(kind: SignalKind, matcher: Option<&str>) -> serde_json::Value {
    let mut e = serde_json::Map::new();
    if let Some(m) = matcher {
        e.insert("matcher".into(), m.into());
    }
    e.insert(
        "hooks".into(),
        serde_json::json!([{ "type": "command", "command": hook_command(kind) }]),
    );
    serde_json::Value::Array(vec![e.into()])
}

/// The `--settings` JSON that makes Claude Code report status (ADR 0029 §2), with the spool
/// placeholder still in it. Events: a prompt or a finished tool means working, a permission prompt
/// means waiting for approval, an idle prompt means waiting for input, a stop means the turn ended.
pub fn claude_hook_settings() -> String {
    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": hook_entry(SignalKind::Working, None),
            "PostToolUse": hook_entry(SignalKind::Working, None),
            "PermissionRequest": hook_entry(SignalKind::AwaitingApproval, None),
            "Notification": hook_entry(SignalKind::Idle, Some("idle_prompt")),
            "Stop": hook_entry(SignalKind::TurnDone, None),
        }
    })
    .to_string()
}

/// True for a path that is safe to embed in the single-quoted hook command: no quote, backslash,
/// double quote or control character. Anything else disables status for that session instead of
/// risking a malformed command.
pub fn spool_path_is_embeddable(p: &Path) -> bool {
    p.to_str().is_some_and(|s| {
        !s.is_empty() && !s.chars().any(|c| c.is_control() || "'\"\\$`".contains(c))
    })
}

/// `args` with the placeholder replaced by the real spool path, or `None` if the path cannot be
/// embedded safely.
pub fn bind_spool(args: &[String], spool: &Path) -> Option<Vec<String>> {
    if !spool_path_is_embeddable(spool) {
        return None;
    }
    let s = spool.to_str()?;
    Some(
        args.iter()
            .map(|a| a.replace(SPOOL_PLACEHOLDER, s))
            .collect(),
    )
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Create a fresh, empty, owner-only spool file under `<data dir>/signals`.
pub fn create_spool(data_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = data_dir.join("signals");
    crate::platform::create_private_dir(&dir)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        nanos,
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = dir.join(name);
    crate::platform::create_private_file(&path)?;
    Ok(path)
}

/// Remove every spool file. No session survives a restart, so none is still wanted at start-up.
pub fn purge(data_dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(data_dir.join("signals")) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Reads the words appended to a spool file since the last call. Unknown words, overlong lines and
/// unreadable files yield nothing; none of them can raise an error or produce a state.
pub struct SpoolFollower {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    /// Inside an overlong line: ignore bytes until the next newline.
    skipping: bool,
    stopped: bool,
}

impl SpoolFollower {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: Vec::new(),
            skipping: false,
            stopped: false,
        }
    }

    /// True once the file grew past [`MAX_SPOOL_BYTES`]; nothing more is read.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn poll(&mut self) -> Vec<SignalKind> {
        if self.stopped {
            return Vec::new();
        }
        let Ok(mut f) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        let Ok(len) = f.metadata().map(|m| m.len()) else {
            return Vec::new();
        };
        if len > MAX_SPOOL_BYTES {
            self.stopped = true;
            return Vec::new();
        }
        if len < self.offset {
            // Truncated behind our back: start again rather than read garbage.
            self.offset = 0;
            self.partial.clear();
            self.skipping = false;
        }
        if f.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        if f.take(len - self.offset).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        self.offset += buf.len() as u64;
        let mut out = Vec::new();
        for b in buf {
            if b == b'\n' {
                if !self.skipping {
                    if let Some(k) = std::str::from_utf8(&self.partial)
                        .ok()
                        .and_then(|w| SignalKind::from_word(w.trim()))
                    {
                        out.push(k);
                    }
                }
                self.partial.clear();
                self.skipping = false;
            } else if self.skipping {
                // Too long to be a word: ignored up to the next newline.
            } else if self.partial.len() < MAX_LINE {
                self.partial.push(b);
            } else {
                self.skipping = true;
                self.partial.clear();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("naurix-signals-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn append(p: &Path, s: &[u8]) {
        let mut f = std::fs::OpenOptions::new().append(true).open(p).unwrap();
        f.write_all(s).unwrap();
    }

    #[test]
    fn only_the_four_known_words_are_accepted() {
        let d = tmp("words");
        let p = create_spool(&d).unwrap();
        let mut f = SpoolFollower::new(p.clone());
        append(
            &p,
            b"working\nawaiting_approval\nrm -rf /\nIDLE\n\n turn_done \nidle\nworking extra\n",
        );
        assert_eq!(
            f.poll(),
            [
                SignalKind::Working,
                SignalKind::AwaitingApproval,
                SignalKind::TurnDone,
                SignalKind::Idle
            ]
        );
        assert!(f.poll().is_empty());
    }

    #[test]
    fn a_line_split_across_reads_is_joined_and_only_new_lines_are_returned() {
        let d = tmp("split");
        let p = create_spool(&d).unwrap();
        let mut f = SpoolFollower::new(p.clone());
        append(&p, b"turn_");
        assert!(f.poll().is_empty());
        append(&p, b"done\nwork");
        assert_eq!(f.poll(), [SignalKind::TurnDone]);
        append(&p, b"ing\n");
        assert_eq!(f.poll(), [SignalKind::Working]);
    }

    #[test]
    fn an_overlong_line_is_dropped_not_kept_and_the_next_line_still_works() {
        let d = tmp("long");
        let p = create_spool(&d).unwrap();
        let mut f = SpoolFollower::new(p.clone());
        let mut junk = vec![b'a'; 10_000];
        junk.push(b'\n');
        append(&p, &junk);
        append(&p, b"idle\n");
        assert_eq!(f.poll(), [SignalKind::Idle]);
        assert!(f.partial.len() <= MAX_LINE);
    }

    #[test]
    fn an_oversize_file_stops_being_followed_and_shows_nothing() {
        let d = tmp("big");
        let p = create_spool(&d).unwrap();
        let mut f = SpoolFollower::new(p.clone());
        append(&p, b"working\n");
        assert_eq!(f.poll(), [SignalKind::Working]);
        append(&p, &vec![b'x'; MAX_SPOOL_BYTES as usize]);
        assert!(f.poll().is_empty());
        assert!(f.stopped());
        append(&p, b"idle\n");
        assert!(f.poll().is_empty());
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let mut f = SpoolFollower::new(PathBuf::from("/nonexistent/naurix/spool"));
        assert!(f.poll().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_spool_is_private_and_purge_removes_it() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("private");
        let p = create_spool(&d).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(d.join("signals"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_ne!(create_spool(&d).unwrap(), p, "every spool is a new file");
        purge(&d);
        assert!(!p.exists());
    }

    #[test]
    fn hook_commands_never_read_the_agent_payload() {
        let json = claude_hook_settings();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut seen = 0;
        for (event, entries) in v["hooks"].as_object().unwrap() {
            for e in entries.as_array().unwrap() {
                for h in e["hooks"].as_array().unwrap() {
                    let c = h["command"].as_str().unwrap();
                    seen += 1;
                    assert!(
                        c.starts_with("printf '")
                            && c.ends_with(&format!(">> '{SPOOL_PLACEHOLDER}'")),
                        "{event}: {c}"
                    );
                    for forbidden in ["cat", "read", "<", "tee", "$(", "`", "jq", "python"] {
                        assert!(!c.contains(forbidden), "{event} uses {forbidden}: {c}");
                    }
                }
            }
        }
        assert_eq!(seen, 5);
    }

    #[test]
    fn the_spool_is_bound_only_when_the_path_is_safe_to_embed() {
        let args = vec!["--settings".to_string(), claude_hook_settings()];
        let ok = bind_spool(&args, Path::new("/data/Application Support/signals/a-1")).unwrap();
        assert!(ok[1].contains("/data/Application Support/signals/a-1"));
        assert!(!ok[1].contains(SPOOL_PLACEHOLDER));
        for bad in [
            "/d/it's/x",
            "/d/\"x",
            "/d/a\\b",
            "/d/$HOME/x",
            "/d/`x`",
            "/d/a\nb",
            "",
        ] {
            assert!(bind_spool(&args, Path::new(bad)).is_none(), "{bad:?}");
        }
    }
}
