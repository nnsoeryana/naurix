//! Creating and removing Git worktrees for agents (ADR 0028): the first place Naurix *writes* to Git,
//! limited to exactly these two operations. Everything goes through the hardened runner of ADR 0017
//! (hooks disabled, no prompts, scrubbed environment, bounded output and time), and nothing here
//! runs without an approval the caller obtained from the gate.
//!
//! * A worktree is created only on request, never copying untracked or ignored files (they can hold
//!   secrets), and its folder and branch names are chosen here, never supplied by the UI.
//! * Only worktrees recorded as created by Naurix can be removed, with plain `git worktree remove`
//!   (never `--force`), and the branch is always kept so no commit is lost.

use super::{first_line, open, run, status, GitError, Opened, StatusResult};
use crate::approval::{Action, ActionClass, Scope};
use crate::persistence::{iso8601_utc_now, new_id, Db};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Checking out a large repository can take a while.
const CREATE_TIMEOUT: Duration = Duration::from_secs(120);
const SHORT: usize = 10;

/// Lowercase letters, digits, `.`, `_` and `-` only, so a name can never carry a path separator,
/// option, space or shell syntax.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_');
        if ok {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let t = out.trim_matches(|c| c == '-' || c == '.').to_string();
    let t: String = t.chars().take(40).collect();
    if t.is_empty() {
        "agent".into()
    } else {
        t
    }
}

/// `2026-11-23T11:05:09Z` becomes `20261123-1105`.
pub fn stamp(iso: &str) -> String {
    let d: String = iso.chars().filter(char::is_ascii_digit).collect();
    if d.len() >= 12 {
        format!("{}-{}", &d[..8], &d[8..12])
    } else {
        "now".into()
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(SHORT).collect()
}

#[derive(Debug, Clone)]
pub struct CreatePlan {
    pub repo_root: PathBuf,
    pub agent_slug: String,
    pub branch: String,
    pub path: PathBuf,
    /// Full commit id `HEAD` pointed at when this was planned.
    pub base: String,
    /// The folder all worktrees live under (the approval covers it).
    pub root_dir: PathBuf,
    /// The command as shown for review. Display only: never executed through a shell.
    pub command: String,
    git: PathBuf,
}

impl CreatePlan {
    /// The full text the user reviews. Built here, never by the UI.
    pub fn display(&self) -> String {
        format!(
            "{}\n\nThis creates a new branch and a separate folder. Your current branch and files are not changed.\nNot copied into it: untracked or ignored files (such as node_modules or .env), and submodules are not initialised.\nThe repository's own Git hooks do not run while it is created.\nThe exact branch name is finalised when it is created (the time part may differ).",
            self.command
        )
    }
}

/// Decide where a worktree would go and what it would be called, without changing anything.
/// Fails clearly for anything that is not a plain repository with a commit, and for a repository
/// Naurix has not been told to trust when its configuration would run commands (ADR 0017).
pub fn plan_create(
    dir: &Path,
    trusted: &dyn Fn(&Path) -> bool,
    root_dir: &Path,
    workspace_id: &str,
    agent_id: &str,
    now_iso: &str,
) -> Result<CreatePlan, GitError> {
    let (git, repo_root) = match open(dir, trusted)? {
        Opened::Ready { git, root, .. } => (git, root),
        Opened::Blocked { .. } => {
            return Err(GitError::Invalid(
                "This repository's own configuration defines commands Git would run. Trust it in the Git view before creating a worktree.".into(),
            ))
        }
        Opened::NotRepo => {
            return Err(GitError::Invalid(
                "This folder is not a Git repository.".into(),
            ))
        }
    };
    let head = run::run(
        &git,
        &repo_root,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
        Duration::from_secs(10),
        4096,
    )?;
    let base = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if head.code != Some(0) || base.is_empty() {
        return Err(GitError::Invalid(
            "This repository has no commits yet, so there is nothing to base a worktree on.".into(),
        ));
    }
    let agent_slug = slug(agent_id);
    let stem = format!("naurix/{agent_slug}-{}", stamp(now_iso));
    let base_dir = root_dir.join(slug(workspace_id));
    for n in 0..100 {
        let branch = if n == 0 {
            stem.clone()
        } else {
            format!("{stem}-{n}")
        };
        let path = base_dir.join(branch.replace('/', "-"));
        let exists = run::run(
            &git,
            &repo_root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ],
            Duration::from_secs(10),
            4096,
        )?;
        if exists.code == Some(0) || path.exists() {
            continue;
        }
        let valid = run::run(
            &git,
            &repo_root,
            &["check-ref-format", "--branch", &branch],
            Duration::from_secs(10),
            4096,
        )?;
        if valid.code != Some(0) {
            return Err(GitError::Invalid(format!(
                "'{branch}' is not a valid branch name."
            )));
        }
        let command = format!(
            "git worktree add -b {branch} {} {}",
            path.display(),
            short(&base)
        );
        return Ok(CreatePlan {
            repo_root,
            agent_slug,
            branch,
            path,
            base,
            root_dir: root_dir.to_path_buf(),
            command,
            git,
        });
    }
    Err(GitError::Invalid(
        "Could not find a free branch name for the worktree.".into(),
    ))
}

/// Run `git worktree add`. The caller must already hold an approval for this plan.
pub fn create(plan: &CreatePlan, trusted: &dyn Fn(&Path) -> bool) -> Result<(), GitError> {
    // Checked again: the repository's configuration may have changed since the plan was made.
    if !matches!(open(&plan.repo_root, trusted)?, Opened::Ready { .. }) {
        return Err(GitError::Invalid(
            "The repository can no longer be used safely; nothing was created.".into(),
        ));
    }
    if plan.path.exists() {
        return Err(GitError::Invalid(
            "The worktree folder already exists; nothing was created.".into(),
        ));
    }
    let path = plan.path.to_string_lossy().into_owned();
    let o = run::run(
        &plan.git,
        &plan.repo_root,
        &["worktree", "add", "-b", &plan.branch, &path, &plan.base],
        CREATE_TIMEOUT,
        64 * 1024,
    )?;
    if o.code != Some(0) {
        return Err(GitError::Failed(first_line(&o.stderr)));
    }
    if !plan.path.is_dir() {
        return Err(GitError::Failed(
            "Git reported success but the worktree folder is missing.".into(),
        ));
    }
    Ok(())
}

/// The approval action for creating a worktree. Once only.
///
/// The fingerprint covers what matters for safety: the repository, the commit it is based on, the
/// agent, and the folder all worktrees live under. It does not cover the time part of the branch
/// name, which is finalised at creation, so the review shown earlier still matches.
pub fn create_action(plan: &CreatePlan, workspace_id: &str) -> Action {
    Action::new(
        ActionClass::CreateWorktree,
        workspace_id,
        &[
            &plan.repo_root.to_string_lossy(),
            &plan.base,
            &plan.agent_slug,
            &plan.root_dir.to_string_lossy(),
        ],
        format!(
            "Create a worktree for {} in {}",
            plan.agent_slug,
            plan.repo_root.display()
        ),
        plan.display(),
        vec![Scope::Once],
    )
}

// ---------------------------------------------------------------------------- records

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedWorktree {
    pub id: String,
    /// `None` once the workspace it was created for has been removed.
    pub workspace_id: Option<String>,
    pub repo_root: String,
    pub path: String,
    pub branch: String,
    pub base_commit: String,
    pub created_at: String,
}

pub struct WorktreeStore<'a> {
    db: &'a Db,
}

const COLS: &str = "id, workspace_id, repo_root, path, branch, base_commit, created_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ManagedWorktree> {
    Ok(ManagedWorktree {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        repo_root: r.get(2)?,
        path: r.get(3)?,
        branch: r.get(4)?,
        base_commit: r.get(5)?,
        created_at: r.get(6)?,
    })
}

impl<'a> WorktreeStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Record a worktree that was just created. Metadata only.
    pub fn insert(
        &self,
        workspace_id: &str,
        plan: &CreatePlan,
    ) -> Result<ManagedWorktree, GitError> {
        let id = new_id();
        self.db
            .c()
            .execute(
                &format!("INSERT INTO worktrees ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"),
                params![
                    id,
                    workspace_id,
                    plan.repo_root.to_string_lossy(),
                    plan.path.to_string_lossy(),
                    plan.branch,
                    plan.base,
                    iso8601_utc_now()
                ],
            )
            .map_err(|e| GitError::Storage(e.to_string()))?;
        self.get(&id)?
            .ok_or_else(|| GitError::Storage("not saved".into()))
    }

    pub fn get(&self, id: &str) -> Result<Option<ManagedWorktree>, GitError> {
        self.db
            .c()
            .query_row(
                &format!("SELECT {COLS} FROM worktrees WHERE id = ?1"),
                [id],
                row,
            )
            .optional()
            .map_err(|e| GitError::Storage(e.to_string()))
    }

    /// Newest first. `workspace_id`: that workspace's worktrees; `None`: every one.
    pub fn list(&self, workspace_id: Option<&str>) -> Result<Vec<ManagedWorktree>, GitError> {
        let c = self.db.c();
        let sql = format!(
            "SELECT {COLS} FROM worktrees WHERE (?1 IS NULL OR workspace_id = ?1) ORDER BY created_at DESC, rowid DESC"
        );
        let mut st = c
            .prepare(&sql)
            .map_err(|e| GitError::Storage(e.to_string()))?;
        let rows = st
            .query_map([workspace_id], row)
            .and_then(|r| r.collect::<Result<Vec<_>, _>>())
            .map_err(|e| GitError::Storage(e.to_string()))?;
        Ok(rows)
    }

    /// Worktrees whose workspace was removed, so they can be cleaned up from Settings.
    pub fn orphaned(&self) -> Result<Vec<ManagedWorktree>, GitError> {
        Ok(self
            .list(None)?
            .into_iter()
            .filter(|w| w.workspace_id.is_none())
            .collect())
    }

    /// Forget a record. Does not touch the folder or Git.
    pub fn delete(&self, id: &str) -> Result<(), GitError> {
        self.db
            .c()
            .execute("DELETE FROM worktrees WHERE id = ?1", [id])
            .map_err(|e| GitError::Storage(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------- removal

#[derive(Debug, Clone)]
pub struct RemovePlan {
    pub id: String,
    pub repo_root: PathBuf,
    pub path: PathBuf,
    pub branch: String,
    pub head: String,
    /// What a person would want to know before it goes, for example "clean".
    pub summary: String,
    state: String,
    git: PathBuf,
}

impl RemovePlan {
    pub fn display(&self) -> String {
        format!(
            "git worktree remove {}\n\nWorktree status: {}.\nOnly this folder is removed. The branch '{}' is kept with all of its commits.",
            self.path.display(),
            self.summary,
            self.branch
        )
    }
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Whether the folder of a recorded worktree is gone (the user deleted it). Then only the record
/// needs to go, which is Naurix's own data and does not need a Git approval.
pub fn folder_missing(rec: &ManagedWorktree) -> bool {
    !Path::new(&rec.path).exists()
}

/// Check that this recorded worktree can be removed right now and describe what that would do.
/// Refuses, with the reason, anything dirty, locked, in use, outside Naurix's worktree folder, or
/// not actually a worktree of that repository. Changes nothing.
pub fn plan_remove(
    rec: &ManagedWorktree,
    root_dir: &Path,
    in_use: bool,
    trusted: &dyn Fn(&Path) -> bool,
) -> Result<RemovePlan, GitError> {
    let bad = |m: &str| Err(GitError::Invalid(m.into()));
    let path = canonical(Path::new(&rec.path));
    if !path.starts_with(canonical(root_dir)) {
        return bad(
            "This folder is not inside Naurix's worktree folder, so Naurix will not remove it.",
        );
    }
    if !path.is_dir() {
        return bad("The worktree folder no longer exists.");
    }
    if in_use {
        return bad("An agent or terminal is still running in this worktree. Stop it first.");
    }
    let (git, repo_root) = match open(Path::new(&rec.repo_root), trusted)? {
        Opened::Ready { git, root, .. } => (git, root),
        _ => return bad("The repository can no longer be used safely; nothing will be removed."),
    };
    let registered = super::worktrees(&repo_root, trusted)?
        .into_iter()
        .find(|w| canonical(Path::new(&w.path)) == path);
    let Some(entry) = registered else {
        return bad("Git does not list this folder as a worktree of the repository.");
    };
    if entry.locked {
        return bad(
            "This worktree is locked in Git. Unlock it yourself if you really want it removed.",
        );
    }
    let StatusResult::Repo(s) = status(&path, trusted)? else {
        return bad("Could not read the worktree's status; nothing will be removed.");
    };
    let changed = s.counts.staged + s.counts.unstaged + s.counts.untracked + s.counts.conflicted;
    if changed > 0 {
        return bad(&format!(
            "This worktree has uncommitted work ({} staged, {} changed, {} untracked, {} in conflict). Commit or discard it first; Naurix never forces a removal.",
            s.counts.staged, s.counts.unstaged, s.counts.untracked, s.counts.conflicted
        ));
    }
    let head = s.head_oid.clone().unwrap_or_default();
    let state = format!("{head}|clean");
    Ok(RemovePlan {
        id: rec.id.clone(),
        repo_root,
        path,
        branch: rec.branch.clone(),
        head,
        summary: "clean, nothing uncommitted".into(),
        state,
        git,
    })
}

/// Run `git worktree remove` without `--force`. The caller must already hold an approval.
pub fn remove(plan: &RemovePlan, trusted: &dyn Fn(&Path) -> bool) -> Result<(), GitError> {
    if !matches!(open(&plan.repo_root, trusted)?, Opened::Ready { .. }) {
        return Err(GitError::Invalid(
            "The repository can no longer be used safely; nothing was removed.".into(),
        ));
    }
    let path = plan.path.to_string_lossy().into_owned();
    let o = run::run(
        &plan.git,
        &plan.repo_root,
        &["worktree", "remove", &path],
        CREATE_TIMEOUT,
        64 * 1024,
    )?;
    if o.code != Some(0) {
        return Err(GitError::Failed(first_line(&o.stderr)));
    }
    Ok(())
}

/// The approval action for removing a worktree. Once only. The fingerprint covers the worktree's
/// commit and state, so work added between the review and the removal asks again.
pub fn remove_action(plan: &RemovePlan, workspace_id: &str) -> Action {
    let h = Sha256::digest(plan.state.as_bytes());
    let state_hash: String = h.iter().take(8).map(|b| format!("{b:02x}")).collect();
    Action::new(
        ActionClass::RemoveWorktree,
        workspace_id,
        &[
            &plan.repo_root.to_string_lossy(),
            &plan.path.to_string_lossy(),
            &plan.branch,
            &plan.head,
            &state_hash,
        ],
        format!("Remove the worktree for {}", plan.branch),
        plan.display(),
        vec![Scope::Once],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_cannot_carry_separators_options_or_spaces() {
        assert_eq!(slug("Claude Code"), "claude-code");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("--exec=rm"), "exec-rm");
        assert_eq!(slug("a b\tc\nd"), "a-b-c-d");
        assert_eq!(slug(""), "agent");
        assert_eq!(slug("///"), "agent");
        assert_eq!(slug(&"x".repeat(200)).len(), 40);
        assert!(slug("ÄÖ🙂").is_ascii());
    }

    #[test]
    fn the_time_part_is_compact_and_tolerant() {
        assert_eq!(stamp("2026-11-23T11:05:09Z"), "20261123-1105");
        assert_eq!(stamp("garbage"), "now");
    }
}
