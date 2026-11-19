//! Tauri commands for the Settings screen: typed settings, the Security overview, and an on-demand
//! memory snapshot. Nothing here leaves the machine, and nothing is collected in the background.

use crate::mcp::{DataDir, SharedSecrets};
use crate::terminal::SharedTerminals;
use naurix_core::git::TrustedRepo;
use naurix_core::mcp::McpStore;
use naurix_core::persistence::Db;
use naurix_core::settings::{self, Settings};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn settings_get(db: State<'_, Arc<Db>>) -> Result<Settings, String> {
    settings::get(&db).map_err(err)
}

/// Validate and store the complete settings document; returns the normalized result.
#[tauri::command]
pub async fn settings_update(
    db: State<'_, Arc<Db>>,
    settings: Settings,
) -> Result<Settings, String> {
    settings::update(&db, &settings).map_err(err)
}

#[tauri::command]
pub async fn settings_reset(db: State<'_, Arc<Db>>) -> Result<Settings, String> {
    settings::reset(&db).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityInfo {
    version: &'static str,
    arch: &'static str,
    data_dir: String,
    keychain_service: &'static str,
    mcp_servers: usize,
    mcp_approved: usize,
    skills: usize,
    trusted_repos: Vec<TrustedRepo>,
}

#[tauri::command]
pub async fn security_info(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
    dir: State<'_, DataDir>,
) -> Result<SecurityInfo, String> {
    let servers = McpStore::new(&db, &**sec).list().map_err(err)?;
    let info = naurix_core::app_info();
    Ok(SecurityInfo {
        version: info.version,
        arch: info.arch,
        data_dir: dir.0.to_string_lossy().into_owned(),
        keychain_service: naurix_core::mcp::secrets::KEYCHAIN_SERVICE,
        mcp_approved: servers.iter().filter(|s| s.is_approved()).count(),
        mcp_servers: servers.len(),
        skills: naurix_core::skills::SkillStore::new(&db, &dir.0)
            .list()
            .map_err(err)?
            .len(),
        trusted_repos: naurix_core::git::trusted_repos(&db).map_err(err)?,
    })
}

#[tauri::command]
pub async fn security_untrust_repo(db: State<'_, Arc<Db>>, key: String) -> Result<(), String> {
    naurix_core::git::untrust(&db, &key).map_err(err)
}

/// Withdraw every MCP approval; servers stay defined but inert until reviewed again.
#[tauri::command]
pub async fn security_revoke_mcp_approvals(
    db: State<'_, Arc<Db>>,
    sec: State<'_, SharedSecrets>,
) -> Result<usize, String> {
    McpStore::new(&db, &**sec)
        .revoke_all_approvals()
        .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSnapshot {
    pid: u32,
    /// Resident memory of the Naurix process itself. WebKit's helper processes are separate and are
    /// not included; `bench/measure.mjs` measures the whole app.
    rss_mb: Option<f64>,
    sessions: usize,
}

/// One on-demand reading (no background sampling).
#[tauri::command]
pub async fn perf_snapshot(terminals: State<'_, SharedTerminals>) -> Result<PerfSnapshot, String> {
    let pid = std::process::id();
    let out = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .map_err(err)?;
    let rss_mb = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .map(|kb| (kb / 1024.0 * 10.0).round() / 10.0);
    Ok(PerfSnapshot {
        pid,
        rss_mb,
        sessions: terminals.session_count(),
    })
}
