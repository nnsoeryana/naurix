//! Git visibility (M9, ADR 0005 / 0017): branch, state, changed files and bounded diffs by shelling out
//! to the installed `git` with hardened options. Everything here is read-only except [`worktree`],
//! which creates and removes worktrees on request and only after an approval (ADR 0028).

mod run;
pub mod status;
pub mod worktree;

pub use run::{find_git, STATUS_TIMEOUT};
pub use status::{FileEntry, Kind};

use crate::persistence::Db;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub const MAX_ENTRIES: usize = 2000;
pub const MAX_DIFF_BYTES: usize = 512 * 1024;
const MAX_LINE_CHARS: usize = 2000;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git was not found. Install the Xcode command line tools (`xcode-select --install`) or Git.")]
    GitMissing,
    #[error("Git took too long to answer and was stopped.")]
    Timeout,
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    Invalid(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<crate::persistence::DbError> for GitError {
    fn from(e: crate::persistence::DbError) -> Self {
        Self::Storage(e.to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoStatus {
    pub root: String,
    pub branch: Option<String>,
    pub detached: bool,
    pub head_oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub stash: u32,
    /// `merging`, `rebasing`, `cherry-picking`, `reverting` or `bisecting` while one is in progress.
    pub operation: Option<&'static str>,
    pub counts: Counts,
    pub entries: Vec<FileEntry>,
    /// More changed files than `MAX_ENTRIES`, or Git's output was cut off.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum StatusResult {
    NotRepo,
    /// The repository's own configuration defines commands that Git would run; refused until trusted.
    Blocked {
        root: String,
        keys: Vec<String>,
    },
    Repo(Box<RepoStatus>),
}

enum Opened {
    NotRepo,
    Blocked {
        root: PathBuf,
        keys: Vec<String>,
    },
    Ready {
        git: PathBuf,
        root: PathBuf,
        git_dir: PathBuf,
    },
}

fn trust_key(root: &Path) -> String {
    let h = Sha256::digest(root.to_string_lossy().as_bytes());
    format!(
        "git.trust.{}",
        h.iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

pub fn is_trusted(db: &Db, root: &Path) -> bool {
    db.get_setting(&trust_key(root)).ok().flatten().is_some()
}

/// Remember that the user trusts this repository to run its own configured commands. The path is
/// stored so the Security settings can list (and revoke) it.
pub fn trust(db: &Db, root: &Path) -> Result<(), GitError> {
    db.set_setting(
        &trust_key(root),
        &serde_json::to_string(&root.to_string_lossy()).expect("serializable"),
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TrustedRepo {
    /// Opaque id used to revoke.
    pub key: String,
    pub path: String,
}

pub fn trusted_repos(db: &Db) -> Result<Vec<TrustedRepo>, GitError> {
    Ok(db
        .list_settings("git.trust.")?
        .into_iter()
        .map(|(key, v)| TrustedRepo {
            key,
            path: serde_json::from_str::<String>(&v)
                .unwrap_or_else(|_| "(folder not recorded)".into()),
        })
        .collect())
}

/// Stop trusting a repository (`key` as returned by [`trusted_repos`]).
pub fn untrust(db: &Db, key: &str) -> Result<(), GitError> {
    if !key.starts_with("git.trust.") {
        return Err(GitError::Invalid("Not a trust entry.".into()));
    }
    db.delete_setting(key)?;
    Ok(())
}

fn first_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("git failed")
        .chars()
        .take(300)
        .collect()
}

fn open(dir: &Path, trusted: &dyn Fn(&Path) -> bool) -> Result<Opened, GitError> {
    let git = find_git()?;
    let o = run::run(
        &git,
        dir,
        &["rev-parse", "--show-toplevel", "--absolute-git-dir"],
        Duration::from_secs(10),
        16 * 1024,
    )?;
    if o.code != Some(0) {
        let e = o.stderr.to_lowercase();
        if e.contains("not a git repository") || e.contains("work tree") {
            return Ok(Opened::NotRepo);
        }
        return Err(GitError::Failed(first_line(&o.stderr)));
    }
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    let mut lines = text.lines();
    let (Some(root), Some(git_dir)) = (lines.next(), lines.next()) else {
        return Ok(Opened::NotRepo);
    };
    let root = PathBuf::from(root);
    let keys = run::config_risks(&git, &root)?;
    if !keys.is_empty() && !trusted(&root) {
        return Ok(Opened::Blocked { root, keys });
    }
    Ok(Opened::Ready {
        git,
        root,
        git_dir: PathBuf::from(git_dir),
    })
}

/// The repository root containing `dir`, if any. Runs only `rev-parse`.
pub fn repo_root(dir: &Path) -> Result<Option<PathBuf>, GitError> {
    let git = find_git()?;
    let o = run::run(
        &git,
        dir,
        &["rev-parse", "--show-toplevel"],
        Duration::from_secs(10),
        16 * 1024,
    )?;
    Ok((o.code == Some(0)).then(|| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim())))
}

fn operation(git_dir: &Path) -> Option<&'static str> {
    [
        ("MERGE_HEAD", "merging"),
        ("rebase-merge", "rebasing"),
        ("rebase-apply", "rebasing"),
        ("CHERRY_PICK_HEAD", "cherry-picking"),
        ("REVERT_HEAD", "reverting"),
        ("BISECT_LOG", "bisecting"),
    ]
    .into_iter()
    .find(|(f, _)| git_dir.join(f).exists())
    .map(|(_, name)| name)
}

pub fn status(dir: &Path, trusted: &dyn Fn(&Path) -> bool) -> Result<StatusResult, GitError> {
    let (git, root, git_dir) = match open(dir, trusted)? {
        Opened::NotRepo => return Ok(StatusResult::NotRepo),
        Opened::Blocked { root, keys } => {
            return Ok(StatusResult::Blocked {
                root: root.to_string_lossy().into_owned(),
                keys,
            })
        }
        Opened::Ready { git, root, git_dir } => (git, root, git_dir),
    };
    let o = run::run(
        &git,
        &root,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--show-stash",
            "--untracked-files=normal",
            "--ignore-submodules=dirty",
        ],
        STATUS_TIMEOUT,
        run::MAX_STATUS_BYTES,
    )?;
    if o.code != Some(0) && !o.truncated {
        return Err(GitError::Failed(first_line(&o.stderr)));
    }
    let p = status::parse(&o.stdout);
    let mut counts = Counts::default();
    for e in &p.entries {
        match e.kind {
            Kind::Untracked => counts.untracked += 1,
            Kind::Unmerged => counts.conflicted += 1,
            _ => {
                counts.staged += e.index.is_some() as usize;
                counts.unstaged += e.worktree.is_some() as usize;
            }
        }
    }
    let truncated = o.truncated || p.entries.len() > MAX_ENTRIES;
    let entries = p.entries.into_iter().take(MAX_ENTRIES).collect();
    Ok(StatusResult::Repo(Box::new(RepoStatus {
        root: root.to_string_lossy().into_owned(),
        branch: p.branch,
        detached: p.detached,
        head_oid: p.head_oid,
        upstream: p.upstream,
        ahead: p.ahead,
        behind: p.behind,
        stash: p.stash,
        operation: operation(&git_dir),
        counts,
        entries,
        truncated,
    })))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    Staged,
    Unstaged,
    Untracked,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
}

fn valid_path(p: &str) -> Result<(), GitError> {
    let path = Path::new(p);
    let bad = p.is_empty()
        || p.len() > 4096
        || p.contains('\0')
        || p.starts_with('-')
        || path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir));
    if bad {
        Err(GitError::Invalid("That path is not valid.".into()))
    } else {
        Ok(())
    }
}

/// A bounded textual diff of one file. Binary files report as such; external diff drivers and
/// textconv filters are disabled. Untracked files are shown as a new file.
pub fn diff(
    dir: &Path,
    path: &str,
    orig_path: Option<&str>,
    area: Area,
    trusted: &dyn Fn(&Path) -> bool,
) -> Result<DiffResult, GitError> {
    valid_path(path)?;
    if let Some(o) = orig_path {
        valid_path(o)?;
    }
    let (git, root) = match open(dir, trusted)? {
        Opened::Ready { git, root, .. } => (git, root),
        Opened::NotRepo => {
            return Err(GitError::Invalid(
                "This folder is not a Git repository.".into(),
            ))
        }
        Opened::Blocked { .. } => {
            return Err(GitError::Invalid(
                "This repository must be trusted first.".into(),
            ))
        }
    };
    let common = [
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--unified=3",
    ];
    let mut args: Vec<&str> = vec!["diff"];
    match area {
        Area::Staged => args.extend(["--cached", "-M"]),
        Area::Unstaged => {}
        Area::Untracked => args.push("--no-index"),
    }
    args.extend(common);
    args.push("--");
    if area == Area::Untracked {
        args.push("/dev/null");
    }
    args.push(path);
    if area == Area::Staged {
        if let Some(o) = orig_path {
            args.push(o);
        }
    }
    let o = run::run(&git, &root, &args, Duration::from_secs(15), MAX_DIFF_BYTES)?;
    // `git diff --no-index` exits with 1 when the files differ; that is the normal case.
    if !matches!(o.code, Some(0) | Some(1)) && !o.truncated {
        return Err(GitError::Failed(first_line(&o.stderr)));
    }
    let raw = String::from_utf8_lossy(&o.stdout).into_owned();
    let binary = raw.contains("Binary files ") && raw.contains(" differ");
    let mut text = String::with_capacity(raw.len());
    for line in raw.lines() {
        if line.chars().count() > MAX_LINE_CHARS {
            text.extend(line.chars().take(MAX_LINE_CHARS));
            text.push_str(" …[line shortened]");
        } else {
            text.push_str(line);
        }
        text.push('\n');
    }
    Ok(DiffResult {
        text,
        truncated: o.truncated,
        binary,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
}

/// Linked worktrees of the repository (read-only; creating or removing them is a later, confirmed feature).
pub fn worktrees(dir: &Path, trusted: &dyn Fn(&Path) -> bool) -> Result<Vec<Worktree>, GitError> {
    let (git, root) = match open(dir, trusted)? {
        Opened::Ready { git, root, .. } => (git, root),
        _ => return Ok(vec![]),
    };
    let o = run::run(
        &git,
        &root,
        &["worktree", "list", "--porcelain", "-z"],
        Duration::from_secs(10),
        256 * 1024,
    )?;
    if o.code != Some(0) {
        return Ok(vec![]);
    }
    let mut out: Vec<Worktree> = vec![];
    for tok in o.stdout.split(|b| *b == 0) {
        let t = String::from_utf8_lossy(tok);
        if let Some(p) = t.strip_prefix("worktree ") {
            out.push(Worktree {
                path: p.to_string(),
                branch: None,
                head: None,
                detached: false,
                bare: false,
                locked: false,
            });
        } else if let Some(w) = out.last_mut() {
            if let Some(h) = t.strip_prefix("HEAD ") {
                w.head = Some(h.chars().take(12).collect());
            } else if let Some(b) = t.strip_prefix("branch ") {
                w.branch = Some(b.trim_start_matches("refs/heads/").to_string());
            } else if t == "detached" {
                w.detached = true;
            } else if t == "bare" {
                w.bare = true;
            } else if t.starts_with("locked") {
                w.locked = true;
            }
        }
    }
    Ok(out)
}
