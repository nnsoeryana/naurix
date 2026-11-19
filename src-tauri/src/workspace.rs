//! Tauri commands for workspaces and session history. Paths are validated in the core crate;
//! the frontend only passes what the user picked in the native folder dialog.

use crate::terminal::SharedTerminals;
use naurix_core::persistence::{Db, SessionRecord, Workspace};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    id: String,
    name: String,
    root_path: String,
    last_opened_at: Option<String>,
    /// False when the folder no longer exists (DATA-005).
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    id: String,
    kind: String,
    title: String,
    lifecycle: String,
    started_at: Option<String>,
    ended_at: Option<String>,
}

fn dto(w: Workspace) -> WorkspaceDto {
    let available = std::path::Path::new(&w.root_path).is_dir();
    WorkspaceDto {
        id: w.id,
        name: w.name,
        root_path: w.root_path,
        last_opened_at: w.last_opened_at,
        available,
    }
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn workspace_list(db: State<'_, Arc<Db>>) -> Result<Vec<WorkspaceDto>, String> {
    Ok(db
        .list_workspaces()
        .map_err(err)?
        .into_iter()
        .map(dto)
        .collect())
}

#[tauri::command]
pub async fn workspace_create(
    db: State<'_, Arc<Db>>,
    path: String,
) -> Result<WorkspaceDto, String> {
    db.create_workspace(&path).map(dto).map_err(err)
}

#[tauri::command]
pub async fn workspace_rename(
    db: State<'_, Arc<Db>>,
    id: String,
    name: String,
) -> Result<WorkspaceDto, String> {
    db.rename_workspace(&id, &name).map(dto).map_err(err)
}

#[tauri::command]
pub async fn workspace_open(db: State<'_, Arc<Db>>, id: String) -> Result<(), String> {
    db.touch_workspace(&id).map_err(err)
}

/// Remove a workspace from the app. Stops its sessions; never deletes project files.
#[tauri::command]
pub async fn workspace_remove(
    db: State<'_, Arc<Db>>,
    terminals: State<'_, SharedTerminals>,
    id: String,
) -> Result<(), String> {
    let (hub, wid) = (terminals.inner().clone(), id.clone());
    tauri::async_runtime::spawn_blocking(move || hub.terminate_workspace(&wid))
        .await
        .map_err(err)?;
    db.remove_workspace(&id).map_err(err)
}

/// Previous sessions for a workspace (metadata only).
#[tauri::command]
pub async fn session_recent(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<Vec<SessionDto>, String> {
    let rows: Vec<SessionRecord> = db.recent_sessions(&workspace_id, 10).map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|s| SessionDto {
            id: s.id,
            kind: s.kind,
            title: s.title,
            lifecycle: s.lifecycle,
            started_at: s.started_at,
            ended_at: s.ended_at,
        })
        .collect())
}
