//! Running `git` safely against a repository we do not control (ADR 0005, ADR 0017).
//!
//! Git can execute programs named in a repository's own configuration. Everything here neutralises
//! the vectors that apply to read-only status/diff, and `config_risks` reports the one that cannot be
//! switched off per call (clean filters), so the caller can refuse to run until the user trusts the repository.

use super::GitError;
use crate::agents::{find_executable, search_dirs};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const STATUS_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_STATUS_BYTES: usize = 8 * 1024 * 1024;

pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: Option<i32>,
    /// Output exceeded the limit and the process was stopped.
    pub truncated: bool,
}

pub fn find_git() -> Result<PathBuf, GitError> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    find_executable(
        "git",
        &search_dirs(std::env::var_os("PATH").as_deref(), &home),
    )
    .ok_or(GitError::GitMissing)
}

/// Options placed before the subcommand. `-c` on the command line outranks the repository's config.
const HARDENING: &[&str] = &[
    "--no-optional-locks", // never take the index lock: do not interfere with the user's own git commands
    "-c",
    "core.fsmonitor=false", // repo config could name a program to run on every status
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.pager=cat",
    "-c",
    "diff.external=",
    "-c",
    "core.untrackedCache=false",
    "-c",
    "color.ui=false",
    "-c",
    "credential.helper=",
];

pub fn run(
    git: &Path,
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    max_bytes: usize,
) -> Result<Output, GitError> {
    let mut cmd = Command::new(git);
    cmd.args(HARDENING).args(args).current_dir(cwd);
    // Do not let the caller's environment redirect git to another repository or program.
    for v in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_EXTERNAL_DIFF",
        "GIT_PAGER",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
    ] {
        cmd.env_remove(v);
    }
    cmd.env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("LC_ALL", "C");
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitError::Failed(format!("Could not run git: {e}")))?;
    let mut out = child.stdout.take().expect("piped");
    let mut err = child.stderr.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        let mut over = false;
        loop {
            match out.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if buf.len() + n > max_bytes {
                        buf.extend_from_slice(&chunk[..max_bytes.saturating_sub(buf.len())]);
                        over = true;
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
        (buf, over)
    });
    let errt = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.by_ref().take(8 * 1024).read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let mut killed = false;
    let status = loop {
        match child
            .try_wait()
            .map_err(|e| GitError::Failed(e.to_string()))?
        {
            Some(st) => break st,
            None => {
                let over = reader.is_finished();
                if start.elapsed() >= timeout || over {
                    let _ = child.kill();
                    let st = child.wait().map_err(|e| GitError::Failed(e.to_string()))?;
                    killed = start.elapsed() >= timeout && !over;
                    break st;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let (stdout, over) = reader.join().unwrap_or_default();
    let stderr = errt.join().unwrap_or_default();
    if killed {
        return Err(GitError::Timeout);
    }
    Ok(Output {
        stdout,
        stderr,
        code: status.code(),
        truncated: over,
    })
}

/// Command-bearing keys in the repository's own (local) configuration that cannot be neutralised per
/// call. Present = do not run git here until the user trusts the repository.
pub fn config_risks(git: &Path, root: &Path) -> Result<Vec<String>, GitError> {
    let o = run(
        git,
        root,
        &["config", "--local", "--list", "-z", "--no-includes"],
        Duration::from_secs(5),
        256 * 1024,
    )?;
    if o.code != Some(0) {
        return Ok(vec![]); // no local config / unreadable: nothing to flag
    }
    let mut risky = vec![];
    for entry in o.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let text = String::from_utf8_lossy(entry);
        let key = text.split('\n').next().unwrap_or("").to_ascii_lowercase();
        let filter_cmd = key.starts_with("filter.")
            && (key.ends_with(".clean") || key.ends_with(".smudge") || key.ends_with(".process"));
        let include =
            key == "include.path" || (key.starts_with("includeif.") && key.ends_with(".path"));
        if filter_cmd || include {
            risky.push(key);
        }
    }
    risky.sort();
    risky.dedup();
    Ok(risky)
}
