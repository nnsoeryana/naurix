//! Tauri commands for terminal sessions. Commands validate input and delegate to
//! `naurix-core`; they contain no PTY logic.
//!
//! Security: the frontend cannot choose what to execute or where. It names a workspace; the
//! backend resolves that to a directory it validated itself and starts only the user's login
//! shell. Agent launches (M5) will go through named presets, not raw programs.

use naurix_core::persistence::SessionKind;
use naurix_core::pty::{Outbound, SessionId};
use naurix_core::session::{SessionLaunch, SessionManager};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

pub(crate) const TERMINATE_GRACE: Duration = Duration::from_secs(3);

/// Session ownership lives in `naurix-core` (ADR 0023); this layer only adapts it to Tauri.
pub type SharedTerminals = Arc<SessionManager>;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExitPayload {
    code: Option<u32>,
    signal: Option<String>,
}

/// Adapt the webview channels to a Core output consumer.
fn output_sink(
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> impl FnMut(Outbound) -> bool + Send + 'static {
    move |out| match out {
        Outbound::Data(d) => on_output.send(InvokeResponseBody::Raw(d)).is_ok(),
        Outbound::Exited(i) => on_exit
            .send(ExitPayload {
                code: i.code,
                signal: i.signal,
            })
            .is_ok(),
    }
}

/// Start a PTY session in a workspace; its output and exit go to the webview channels.
pub(crate) fn start_session(
    terminals: &Arc<SessionManager>,
    workspace_id: &str,
    launch: SessionLaunch,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    terminals
        .start(
            workspace_id,
            launch,
            rows,
            cols,
            output_sink(on_output, on_exit),
        )
        .map(|id| id.0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn terminal_spawn(
    terminals: State<'_, SharedTerminals>,
    workspace_id: String,
    title: Option<String>,
    at_repo_root: Option<bool>,
    worktree_id: Option<String>,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    // "Open repository in terminal": the backend works out the repository root itself; the webview
    // can never name a directory.
    let cwd = if let Some(id) = worktree_id.as_deref() {
        // A terminal in a worktree Naurix created: the folder comes from its record, never from the webview.
        Some(crate::worktree::path_for(terminals.db(), id)?)
    } else if at_repo_root == Some(true) {
        let ws = terminals
            .db()
            .workspace(&workspace_id)
            .map_err(|e| e.to_string())?;
        let root = naurix_core::git::repo_root(std::path::Path::new(&ws.root_path))
            .map_err(|e| e.to_string())?
            .ok_or("This folder is not a Git repository.")?;
        Some(root)
    } else {
        None
    };
    // Shell program and flags come from Settings (validated in core); $SHELL is the default.
    let settings = naurix_core::settings::get(terminals.db()).map_err(|e| e.to_string())?;
    let (program, args) =
        naurix_core::settings::shell_command(&settings.terminal, shell_from_env().as_deref());
    let launch = SessionLaunch {
        program,
        args,
        env: vec![],
        kind: SessionKind::Terminal,
        title: title.unwrap_or_else(|| "Terminal".into()),
        adapter_id: None,
        approval_id: None,
        worktree_id,
        cwd,
    };
    start_session(
        terminals.inner(),
        &workspace_id,
        launch,
        rows,
        cols,
        on_output,
        on_exit,
    )
}

#[tauri::command]
pub async fn terminal_write(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    data: String,
) -> Result<(), String> {
    terminals
        .write(SessionId(id), data.as_bytes())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn terminal_write_bytes(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    data: Vec<u8>,
) -> Result<(), String> {
    terminals
        .write(SessionId(id), &data)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn terminal_resize(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    terminals
        .resize(SessionId(id), rows, cols)
        .map_err(|e| e.to_string())
}

/// The webview finished processing `bytes` of output (xterm.js write callback).
#[tauri::command]
pub async fn terminal_ack(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    bytes: usize,
) -> Result<(), String> {
    terminals
        .ack(SessionId(id), bytes)
        .map_err(|e| e.to_string())
}

/// Whether a command is running in the foreground (to confirm before closing). Unknown => false.
#[tauri::command]
pub async fn terminal_has_foreground_job(
    terminals: State<'_, SharedTerminals>,
    id: u64,
) -> Result<bool, String> {
    Ok(terminals.has_foreground_job(SessionId(id)).unwrap_or(false))
}

/// A session that is still known to Core: running, or exited but not yet closed (ADR 0026). Metadata only.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    id: u64,
    workspace_id: String,
    kind: &'static str,
    title: String,
    adapter_id: Option<String>,
    started_at: String,
    /// The branch of the Naurix-created worktree this session runs in, if any.
    worktree_branch: Option<String>,
    running: bool,
    code: Option<u32>,
    signal: Option<String>,
    /// The agent's last reported status (ADR 0029). Absent means "no signal", never "idle".
    status: Option<StatusDto>,
}

#[derive(Serialize)]
pub struct StatusDto {
    kind: &'static str,
    at: String,
}

/// Sessions the webview can attach to (after a reload it has forgotten them).
#[tauri::command]
pub async fn session_list(
    terminals: State<'_, SharedTerminals>,
) -> Result<Vec<SessionDto>, String> {
    let store = naurix_core::git::worktree::WorktreeStore::new(terminals.db());
    Ok(terminals
        .list()
        .into_iter()
        .map(|i| SessionDto {
            worktree_branch: i
                .worktree_id
                .as_deref()
                .and_then(|w| store.get(w).ok().flatten())
                .map(|w| w.branch),
            id: i.id.0,
            workspace_id: i.workspace_id,
            kind: match i.kind {
                SessionKind::Terminal => "terminal",
                SessionKind::Agent => "agent",
            },
            title: i.title,
            adapter_id: i.adapter_id,
            started_at: i.started_at,
            running: i.exit.is_none(),
            status: i.status.map(|s| StatusDto {
                kind: s.kind.word(),
                at: s.at,
            }),
            code: i.exit.as_ref().and_then(|e| e.code),
            signal: i.exit.and_then(|e| e.signal),
        })
        .collect())
}

/// Make this webview the consumer of an existing session: the PTY is resized to the new view, the
/// buffered tail is replayed, then live output follows. Returns the consumer's generation for `session_detach`.
#[tauri::command]
pub async fn session_attach(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    let id = SessionId(id);
    // The new view has its own size. An exited session cannot be resized; that is fine.
    let _ = terminals.resize(id, rows, cols);
    terminals
        .attach(id, output_sink(on_output, on_exit))
        .map_err(|e| e.to_string())
}

/// The view that attached with `generation` is going away. Ignored if a newer view has since attached.
/// The process keeps running.
#[tauri::command]
pub async fn session_detach(
    terminals: State<'_, SharedTerminals>,
    id: u64,
    generation: u64,
) -> Result<(), String> {
    terminals
        .detach_if(SessionId(id), generation)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The user closed this session: stop it if it is running and forget it. This is the only thing that
/// ends a session on purpose; an unmounting view only detaches.
#[tauri::command]
pub async fn session_close(terminals: State<'_, SharedTerminals>, id: u64) -> Result<(), String> {
    let hub = terminals.inner().clone();
    // close() blocks for up to grace + 2 s; keep it off the async worker.
    tauri::async_runtime::spawn_blocking(move || {
        hub.close(SessionId(id)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Smoke and benchmark modes create workspaces and sessions, so they only run against a separate data
/// folder (`NAURIX_DATA_DIR`). They can never touch your real workspaces, even if the variables leak
/// into a normal run.
fn sandboxed() -> bool {
    std::env::var_os("NAURIX_DATA_DIR").is_some()
}

/// Debug-only smoke-test hook (dev builds): configuration from the environment plus a way to
/// write a report file, so the UI can be verified end to end without screenshots.
/// `NAURIX_SMOKE_OUT` = report path, `NAURIX_SMOKE_WORKSPACE` = folder to open as a workspace.
/// Absent from release builds.
#[derive(Serialize)]
pub struct SmokeConfig {
    out: String,
    workspace: Option<String>,
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_smoke(text: Option<String>) -> Option<SmokeConfig> {
    if !sandboxed() {
        return None;
    }
    let out = std::env::var("NAURIX_SMOKE_OUT").ok()?;
    if let Some(t) = text {
        let _ = std::fs::write(&out, t);
    }
    Some(SmokeConfig {
        out,
        workspace: std::env::var("NAURIX_SMOKE_WORKSPACE").ok(),
    })
}

#[cfg(not(debug_assertions))]
#[tauri::command]
pub fn debug_smoke(_text: Option<String>) -> Option<SmokeConfig> {
    None
}

/// Benchmark mode (all builds, requires `NAURIX_DATA_DIR`): `NAURIX_BENCH_WORKSPACE` + `NAURIX_BENCH_TERMINALS` (1-16) make the app
/// open that many terminals on start so memory can be measured (docs/PERFORMANCE_AND_COMPATIBILITY.md).
/// `NAURIX_BENCH_COMMAND` is typed into each terminal, exactly as if the user typed it.
/// Reads only these environment variables; nothing is sent anywhere.
#[derive(Serialize)]
pub struct BenchConfig {
    workspace: String,
    terminals: u8,
    command: Option<String>,
    /// JSON of settings to apply before opening terminals (e.g. `{"terminal":{"renderer":"webgl"}}`).
    settings: Option<String>,
}

#[tauri::command]
pub fn bench_config() -> Option<BenchConfig> {
    if !sandboxed() {
        return None;
    }
    let workspace = std::env::var("NAURIX_BENCH_WORKSPACE").ok()?;
    let terminals = std::env::var("NAURIX_BENCH_TERMINALS")
        .ok()?
        .parse::<u8>()
        .ok()
        .filter(|n| (1..=16).contains(n))?;
    Some(BenchConfig {
        workspace,
        terminals,
        command: std::env::var("NAURIX_BENCH_COMMAND").ok(),
        settings: std::env::var("NAURIX_BENCH_SETTINGS").ok(),
    })
}

/// Debug-only: `NAURIX_SMOKE_VISUAL=home|terminal` makes the smoke test open the workspace (and, for
/// `terminal`, one tab) and stop, so the window can be inspected.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_smoke_visual() -> Option<String> {
    if !sandboxed() {
        return None;
    }
    std::env::var("NAURIX_SMOKE_VISUAL").ok()
}

#[cfg(not(debug_assertions))]
#[tauri::command]
pub fn debug_smoke_visual() -> Option<String> {
    None
}

/// The user's configured shell from the environment: `$SHELL` on Unix, `%COMSPEC%` on Windows.
fn shell_from_env() -> Option<String> {
    std::env::var(if cfg!(windows) { "COMSPEC" } else { "SHELL" }).ok()
}
