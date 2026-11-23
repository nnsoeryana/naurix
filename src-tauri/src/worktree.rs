//! Tauri commands for agent worktrees (ADR 0028). The webview names a worktree by id and answers an
//! approval request by id and scope; it never supplies a path, a branch or a command, and a Git
//! mutation only runs after the gate has allowed exactly what the backend built.

use crate::approval::SharedGate;
use crate::mcp::DataDir;
use crate::terminal::SharedTerminals;
use naurix_core::approval::{ApprovalRequest, Verdict};
use naurix_core::git::worktree::{self, ManagedWorktree, WorktreeStore};
use naurix_core::git::{self, GitError};
use naurix_core::persistence::{iso8601_utc_now, Db};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

/// Where Naurix keeps the worktrees it creates.
pub(crate) fn root_dir(dir: &DataDir) -> PathBuf {
    dir.0.join("worktrees")
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn trusted(db: &Db) -> impl Fn(&Path) -> bool + '_ {
    move |root: &Path| git::is_trusted(db, root)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeDto {
    id: String,
    workspace_id: Option<String>,
    repo_root: String,
    path: String,
    branch: String,
    base_commit: String,
    created_at: String,
    /// The folder is still there.
    exists: bool,
    /// A session is running in it right now.
    in_use: bool,
}

fn in_use(terminals: &SharedTerminals, id: &str) -> bool {
    terminals
        .list()
        .iter()
        .any(|s| s.worktree_id.as_deref() == Some(id) && s.running())
}

fn dto(terminals: &SharedTerminals, w: ManagedWorktree) -> WorktreeDto {
    WorktreeDto {
        exists: !worktree::folder_missing(&w),
        in_use: in_use(terminals, &w.id),
        id: w.id,
        workspace_id: w.workspace_id,
        repo_root: w.repo_root,
        path: w.path,
        branch: w.branch,
        base_commit: w.base_commit,
        created_at: w.created_at,
    }
}

/// Worktrees Naurix created: one workspace's, or (no workspace) every one.
#[tauri::command]
pub async fn worktree_list(
    db: State<'_, Arc<Db>>,
    terminals: State<'_, SharedTerminals>,
    workspace_id: Option<String>,
) -> Result<Vec<WorktreeDto>, String> {
    let all = WorktreeStore::new(&db)
        .list(workspace_id.as_deref())
        .map_err(err)?;
    Ok(all.into_iter().map(|w| dto(&terminals, w)).collect())
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SupportDto {
    Ok,
    /// Why a worktree cannot be offered here.
    Unavailable {
        reason: String,
    },
}

/// Whether "run in a separate worktree" can be offered for this workspace. Changes nothing.
#[tauri::command]
pub async fn worktree_support(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    workspace_id: String,
) -> Result<SupportDto, String> {
    let ws = db.workspace(&workspace_id).map_err(err)?;
    let (db, root) = (db.inner().clone(), root_dir(&dir));
    tauri::async_runtime::spawn_blocking(move || {
        let t = trusted(&db);
        Ok(
            match worktree::plan_create(
                Path::new(&ws.root_path),
                &t,
                &root,
                &workspace_id,
                "check",
                &iso8601_utc_now(),
            ) {
                Ok(_) => SupportDto::Ok,
                Err(e @ (GitError::Invalid(_) | GitError::GitMissing)) => SupportDto::Unavailable {
                    reason: e.to_string(),
                },
                Err(e) => SupportDto::Unavailable {
                    reason: e.to_string(),
                },
            },
        )
    })
    .await
    .map_err(err)?
}

/// Agents already running in this workspace's own folder (not in a worktree). A reason to suggest a
/// worktree; it never forces one.
#[tauri::command]
pub async fn agents_in_workspace_folder(
    terminals: State<'_, SharedTerminals>,
    workspace_id: String,
) -> Result<usize, String> {
    Ok(terminals
        .list()
        .iter()
        .filter(|s| {
            s.workspace_id == workspace_id
                && s.kind == naurix_core::persistence::SessionKind::Agent
                && s.worktree_id.is_none()
                && s.running()
        })
        .count())
}

/// The worktree a session runs in, if any.
#[tauri::command]
pub async fn session_worktree(
    db: State<'_, Arc<Db>>,
    terminals: State<'_, SharedTerminals>,
    id: u64,
) -> Result<Option<WorktreeDto>, String> {
    let Some(wid) = terminals
        .list()
        .into_iter()
        .find(|s| s.id.0 == id)
        .and_then(|s| s.worktree_id)
    else {
        return Ok(None);
    };
    Ok(WorktreeStore::new(&db)
        .get(&wid)
        .map_err(err)?
        .map(|w| dto(&terminals, w)))
}

fn record(db: &Db, id: &str) -> Result<ManagedWorktree, String> {
    WorktreeStore::new(db)
        .get(id)
        .map_err(err)?
        .ok_or_else(|| "That worktree is not known to Naurix.".to_string())
}

fn remove_plan(
    db: &Db,
    terminals: &SharedTerminals,
    dir: &DataDir,
    id: &str,
) -> Result<(ManagedWorktree, worktree::RemovePlan), String> {
    let rec = record(db, id)?;
    let t = trusted(db);
    let plan =
        worktree::plan_remove(&rec, &root_dir(dir), in_use(terminals, id), &t).map_err(err)?;
    Ok((rec, plan))
}

/// Check that this worktree can be removed and return the review to show. Refuses, with the reason,
/// for anything dirty, locked, in use or not Naurix's own. Changes nothing.
#[tauri::command]
pub async fn worktree_review_remove(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    terminals: State<'_, SharedTerminals>,
    dir: State<'_, DataDir>,
    id: String,
) -> Result<ApprovalRequest, String> {
    let (db, terminals, dir, gate) = (
        db.inner().clone(),
        terminals.inner().clone(),
        DataDir(dir.0.clone()),
        gate.inner().clone(),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let (rec, plan) = remove_plan(&db, &terminals, &dir, &id)?;
        let action = worktree::remove_action(&plan, rec.workspace_id.as_deref().unwrap_or(""));
        match gate.preview(&action).map_err(err)? {
            Verdict::Required(r) => Ok(r),
            // A once grant cannot pre-exist for this exact state, so there is nothing to show.
            Verdict::Allowed { .. } => Err("This removal is already approved; try again.".into()),
        }
    })
    .await
    .map_err(err)?
}

/// Remove the worktree, only if the approval for exactly what was reviewed exists. Git is asked to
/// remove it without `--force`, so anything uncommitted stops it. The branch is kept.
#[tauri::command]
pub async fn worktree_remove(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    terminals: State<'_, SharedTerminals>,
    dir: State<'_, DataDir>,
    id: String,
) -> Result<(), String> {
    let (db, terminals, dir, gate) = (
        db.inner().clone(),
        terminals.inner().clone(),
        DataDir(dir.0.clone()),
        gate.inner().clone(),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let (rec, plan) = remove_plan(&db, &terminals, &dir, &id)?;
        let action = worktree::remove_action(&plan, rec.workspace_id.as_deref().unwrap_or(""));
        match gate.authorize(&action).map_err(err)? {
            Verdict::Allowed { .. } => {}
            Verdict::Required(_) => {
                return Err("This removal needs your approval again; nothing was removed.".into())
            }
        }
        let t = trusted(&db);
        worktree::remove(&plan, &t).map_err(err)?;
        WorktreeStore::new(&db).delete(&id).map_err(err)
    })
    .await
    .map_err(err)?
}

/// Forget a worktree whose folder is already gone. Only Naurix's own record is deleted, so no Git
/// approval is involved; Git's own list is left for the user.
#[tauri::command]
pub async fn worktree_forget(db: State<'_, Arc<Db>>, id: String) -> Result<(), String> {
    let rec = record(&db, &id)?;
    if !worktree::folder_missing(&rec) {
        return Err("The worktree folder still exists; remove it instead.".into());
    }
    WorktreeStore::new(&db).delete(&id).map_err(err)
}

/// The folder a terminal for this worktree should start in. Resolved here from the record, never from
/// anything the webview sends.
pub(crate) fn path_for(db: &Db, id: &str) -> Result<PathBuf, String> {
    let rec = record(db, id)?;
    let p = PathBuf::from(&rec.path);
    if p.is_dir() {
        Ok(p)
    } else {
        Err("The worktree folder no longer exists.".into())
    }
}
