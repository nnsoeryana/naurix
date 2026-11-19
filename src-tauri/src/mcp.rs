//! Tauri commands for the MCP manager. Commands validate input and delegate to `naurix_core::mcp`.
//! Secret values flow in one direction only: from the user's keyboard into the Keychain. No command
//! ever returns one.

use crate::agent::dirs;
use naurix_core::agents::{adapter, Feature, Level, PRESETS};
use naurix_core::mcp::catalog::{CatalogEntry, CATALOG};
use naurix_core::mcp::model::{McpInput, McpServer, Pair, Transport};
use naurix_core::mcp::probe::{probe, ProbeResult};
use naurix_core::mcp::sync::{self, SyncPlan, SyncRun};
use naurix_core::mcp::{McpStore, SecretStore};
use naurix_core::persistence::Db;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

pub type SharedSecrets = Arc<dyn SecretStore>;

/// Where Naurix keeps its own files (used for sync backups).
#[derive(Clone)]
pub struct DataDir(pub PathBuf);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretDto {
    name: String,
    has_value: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerDto {
    id: String,
    name: String,
    transport: Transport,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    env: Vec<Pair>,
    headers: Vec<Pair>,
    secrets: Vec<SecretDto>,
    enabled: bool,
    approved: bool,
    /// What the user approves: the review dialog sends this back so approval is bound to what was shown.
    fingerprint: String,
    approved_at: Option<String>,
    last_status: Option<String>,
    last_error: Option<String>,
    last_tool_count: Option<i64>,
    last_tested_at: Option<String>,
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn dto(st: &McpStore<'_>, s: McpServer) -> Result<ServerDto, String> {
    let secrets = st
        .secret_status(&s)
        .map_err(err)?
        .into_iter()
        .map(|(name, has_value)| SecretDto { name, has_value })
        .collect();
    Ok(ServerDto {
        approved: s.is_approved(),
        fingerprint: s.fingerprint(),
        id: s.id,
        name: s.name,
        transport: s.transport,
        command: s.command,
        args: s.args,
        url: s.url,
        env: s.env,
        headers: s.headers,
        secrets,
        enabled: s.enabled,
        approved_at: s.approved_at,
        last_status: s.last_status,
        last_error: s.last_error,
        last_tool_count: s.last_tool_count,
        last_tested_at: s.last_tested_at,
    })
}

fn list(db: &Db, sec: &dyn SecretStore) -> Result<Vec<ServerDto>, String> {
    let st = McpStore::new(db, sec);
    st.list()
        .map_err(err)?
        .into_iter()
        .map(|s| dto(&st, s))
        .collect()
}

fn workspace_root(db: &Db, workspace_id: &str) -> Result<PathBuf, String> {
    let w = db.workspace(workspace_id).map_err(err)?;
    let p = PathBuf::from(w.root_path);
    if p.is_dir() {
        Ok(p)
    } else {
        Err(format!("The workspace folder is missing: {}", p.display()))
    }
}

fn home() -> PathBuf {
    naurix_core::platform::home_dir_or_empty()
}

#[tauri::command]
pub async fn mcp_list(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
) -> Result<Vec<ServerDto>, String> {
    list(&db, &**sec)
}

#[tauri::command]
pub async fn mcp_catalog() -> Vec<&'static CatalogEntry> {
    CATALOG.iter().collect()
}

/// An editor-ready definition for a catalog entry (nothing is saved or run).
#[tauri::command]
pub async fn mcp_catalog_input(
    db: State<'_, Arc<Db>>,
    entry_id: String,
    workspace_id: Option<String>,
) -> Result<McpInput, String> {
    let e = CATALOG
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or("unknown catalog entry")?;
    let root = match workspace_id {
        Some(w) => workspace_root(&db, &w)?.to_string_lossy().into_owned(),
        None => "/path/to/project".to_string(),
    };
    Ok(e.to_input(&root))
}

#[tauri::command]
pub async fn mcp_save(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    input: McpInput,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec).save(input).map_err(err)?;
    list(&db, &**sec)
}

#[tauri::command]
pub async fn mcp_delete(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec).delete(&id).map_err(err)?;
    list(&db, &**sec)
}

#[tauri::command]
pub async fn mcp_set_enabled(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
    enabled: bool,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec)
        .set_enabled(&id, enabled)
        .map_err(err)?;
    list(&db, &**sec)
}

/// Write-only: stores the value in the Keychain and returns only whether a value exists.
#[tauri::command]
pub async fn mcp_secret_set(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
    name: String,
    value: String,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec)
        .set_secret(&id, &name, &value)
        .map_err(err)?;
    list(&db, &**sec)
}

#[tauri::command]
pub async fn mcp_secret_clear(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
    name: String,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec)
        .clear_secret(&id, &name)
        .map_err(err)?;
    list(&db, &**sec)
}

#[tauri::command]
pub async fn mcp_approve(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
    fingerprint: String,
) -> Result<Vec<ServerDto>, String> {
    McpStore::new(&db, &**sec)
        .approve(&id, &fingerprint)
        .map_err(err)?;
    list(&db, &**sec)
}

/// Run the handshake against an approved local server (executes it). Runs off the async worker.
#[tauri::command]
pub async fn mcp_test(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    id: String,
) -> Result<ProbeResult, String> {
    let (db, sec) = (db.inner().clone(), sec.inner().clone());
    tauri::async_runtime::spawn_blocking(move || {
        let st = McpStore::new(&db, &*sec);
        let server = st.get(&id).map_err(err)?;
        let result = probe(&st, &server, &dirs(), Duration::from_secs(15)).map_err(err)?;
        if result.status != "not_testable" {
            st.record_probe(
                &id,
                &result.status,
                result.error.as_deref(),
                result.ok.then_some(result.tools.len() as i64),
            )
            .map_err(err)?;
        }
        Ok(result)
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAgentDto {
    agent_id: &'static str,
    name: &'static str,
    supported: bool,
    /// Why not (or what the limit is): from the verified adapter matrix.
    note: String,
}

/// Which agents MCP servers can be synced to, straight from the adapter verification levels.
#[tauri::command]
pub async fn mcp_sync_agents() -> Vec<SyncAgentDto> {
    PRESETS
        .iter()
        .filter_map(|(id, name, _)| {
            let a = adapter(id)?;
            let (supported, note) = match a.level(Feature::McpConfig) {
                Level::Verified { .. } => (true, a.capabilities.iter().find(|c| c.feature == Feature::McpConfig).map(|c| c.detail.to_string()).unwrap_or_default()),
                Level::DocumentedAccepted { .. } => (true, "Accepted by the maintainer without live verification.".to_string()),
                Level::DocumentedOnly { .. } => (false, "Documented by the vendor but not verified against the real CLI yet, so Naurix will not write to it.".to_string()),
                Level::Unsupported { reason } => (false, reason.to_string()),
            };
            Some(SyncAgentDto { agent_id: id, name, supported, note })
        })
        .collect()
}

#[tauri::command]
pub async fn mcp_sync_plan(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    workspace_id: String,
    agent_id: String,
) -> Result<SyncPlan, String> {
    let root = workspace_root(&db, &workspace_id)?;
    sync::plan(&McpStore::new(&db, &**sec), &agent_id, &root, &home()).map_err(err)
}

#[tauri::command]
pub async fn mcp_sync_apply(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    dir: State<'_, DataDir>,
    workspace_id: String,
    agent_id: String,
    plan_hash: String,
) -> Result<SyncRun, String> {
    let root = workspace_root(&db, &workspace_id)?;
    sync::apply(
        &McpStore::new(&db, &**sec),
        &agent_id,
        &root,
        &home(),
        &plan_hash,
        &dir.0.join("mcp-backups"),
    )
    .map_err(err)
}

#[tauri::command]
pub async fn mcp_sync_runs(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    agent_id: String,
) -> Result<Vec<SyncRun>, String> {
    sync::runs(&McpStore::new(&db, &**sec), &agent_id, 10).map_err(err)
}

#[tauri::command]
pub async fn mcp_sync_rollback(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    run_id: String,
) -> Result<SyncRun, String> {
    sync::rollback(&McpStore::new(&db, &**sec), &run_id).map_err(err)
}
