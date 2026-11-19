//! Tauri commands for read-only Git visibility. They run `git` through `naurix_core::git`, which
//! neutralises repository-defined commands and refuses repositories that define filters or includes
//! until the user trusts them. Nothing here changes a repository.

use naurix_core::git::{self, Area, DiffResult, StatusResult, Worktree};
use naurix_core::persistence::Db;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn workspace_dir(db: &Db, id: &str) -> Result<PathBuf, String> {
    let w = db.workspace(id).map_err(err)?;
    let p = PathBuf::from(w.root_path);
    if p.is_dir() {
        Ok(p)
    } else {
        Err(format!("The workspace folder is missing: {}", p.display()))
    }
}

/// Run blocking git work off the async worker.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

#[tauri::command]
pub async fn git_status(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<StatusResult, String> {
    let db = db.inner().clone();
    blocking(move || {
        let dir = workspace_dir(&db, &workspace_id)?;
        git::status(&dir, &|root: &Path| git::is_trusted(&db, root)).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn git_diff(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    path: String,
    orig_path: Option<String>,
    area: Area,
) -> Result<DiffResult, String> {
    let db = db.inner().clone();
    blocking(move || {
        let dir = workspace_dir(&db, &workspace_id)?;
        git::diff(&dir, &path, orig_path.as_deref(), area, &|root: &Path| {
            git::is_trusted(&db, root)
        })
        .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn git_worktrees(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<Vec<Worktree>, String> {
    let db = db.inner().clone();
    blocking(move || {
        let dir = workspace_dir(&db, &workspace_id)?;
        git::worktrees(&dir, &|root: &Path| git::is_trusted(&db, root)).map_err(err)
    })
    .await
}

/// The user chose to trust this workspace's repository to run the commands its own config defines.
#[tauri::command]
pub async fn git_trust(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<StatusResult, String> {
    let db = db.inner().clone();
    blocking(move || {
        let dir = workspace_dir(&db, &workspace_id)?;
        let root = git::repo_root(&dir)
            .map_err(err)?
            .ok_or("This folder is not a Git repository.")?;
        git::trust(&db, &root).map_err(err)?;
        git::status(&dir, &|r: &Path| git::is_trusted(&db, r)).map_err(err)
    })
    .await
}
