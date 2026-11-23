//! Tauri commands for project memory (ADR 0031). The webview names a workspace id, a note id and the
//! user's own text. It never names a file: the export opens the OS save dialog from here, so the
//! destination comes from the user's choice and not from the page. Note text is never logged.

use naurix_core::memory::{self, ExportFormat, MemoryStore, Note, Status};
use naurix_core::persistence::Db;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn memory_status(db: State<'_, Arc<Db>>, workspace_id: String) -> Result<Status, String> {
    MemoryStore::new(&db).status(&workspace_id).map_err(err)
}

#[tauri::command]
pub async fn memory_set_enabled(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    enabled: bool,
) -> Result<Status, String> {
    let m = MemoryStore::new(&db);
    m.set_enabled(&workspace_id, enabled).map_err(err)?;
    m.status(&workspace_id).map_err(err)
}

#[tauri::command]
pub async fn memory_list(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    query: Option<String>,
) -> Result<Vec<Note>, String> {
    MemoryStore::new(&db)
        .list(&workspace_id, query.as_deref())
        .map_err(err)
}

#[tauri::command]
pub async fn memory_save(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    id: Option<String>,
    title: String,
    body: String,
) -> Result<Note, String> {
    MemoryStore::new(&db)
        .save(&workspace_id, id.as_deref(), &title, &body)
        .map_err(err)
}

#[tauri::command]
pub async fn memory_delete(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    id: String,
) -> Result<(), String> {
    MemoryStore::new(&db)
        .delete(&workspace_id, &id)
        .map_err(err)
}

#[tauri::command]
pub async fn memory_delete_all(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<usize, String> {
    MemoryStore::new(&db).delete_all(&workspace_id).map_err(err)
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportResult {
    /// The user closed the save dialog or declined the repository warning. Nothing was written.
    Cancelled,
    Written {
        path: String,
        in_repository: bool,
    },
}

/// Export a workspace's notes to a file the user picks in the OS save dialog. When the chosen place is
/// inside a Git repository the user must confirm a second time. Nothing is ever added or committed.
#[tauri::command]
pub async fn memory_export(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    format: String,
) -> Result<ExportResult, String> {
    let (fmt, ext, label) = match format.as_str() {
        "markdown" => (ExportFormat::Markdown, "md", "Markdown"),
        "json" => (ExportFormat::Json, "json", "JSON"),
        _ => return Err("Unknown export format.".into()),
    };
    let text = MemoryStore::new(&db)
        .export(&workspace_id, fmt)
        .map_err(err)?;
    let app2 = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let path = app2
            .dialog()
            .file()
            .set_title("Export memory")
            .set_file_name(format!("naurix-memory.{ext}"))
            .add_filter(label, &[ext])
            .blocking_save_file()?;
        let path = path.into_path().ok()?;
        let in_repo = memory::inside_git_repo(&path);
        if in_repo {
            let ok = app2
                .dialog()
                .message("This location is inside a Git repository. Memory notes are your own text and could be committed or pushed by mistake. Write the file here anyway?")
                .title("Export inside a Git repository?")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom("Write here".into(), "Cancel".into()))
                .blocking_show();
            if !ok {
                return None;
            }
        }
        Some((path, in_repo))
    })
    .await
    .map_err(err)?;
    let Some((path, in_repository)) = picked else {
        return Ok(ExportResult::Cancelled);
    };
    std::fs::write(&path, text).map_err(|e| format!("Could not write the file: {e}"))?;
    Ok(ExportResult::Written {
        path: path.to_string_lossy().into_owned(),
        in_repository,
    })
}
