//! Tauri commands for terminal sessions. Commands validate input and delegate to
//! `naurix-core`; they contain no PTY logic.
//!
//! Security: the frontend cannot choose what to execute or where. It names a workspace; the
//! backend resolves that to a directory it validated itself and starts only the user's login
//! shell. Agent launches (M5) will go through named presets, not raw programs.

use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::{forward, Flow, Outbound, SessionId, SpawnSpec, Supervisor};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

const TERMINATE_GRACE: Duration = Duration::from_secs(3);

struct Entry {
    flow: Arc<Flow>,
    workspace_id: String,
}

pub struct Terminals {
    sup: Supervisor,
    pub(crate) db: Arc<Db>,
    entries: Mutex<HashMap<SessionId, Entry>>,
}

pub type SharedTerminals = Arc<Terminals>;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExitPayload {
    code: Option<u32>,
    signal: Option<String>,
}

impl Terminals {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            sup: Supervisor::new(),
            db,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Terminate every session. Called when the page (re)loads, since the JS side that owned the
    /// sessions is gone, and on app exit.
    pub fn shutdown(&self) {
        for (_, e) in self.entries.lock().unwrap().drain() {
            e.flow.close();
        }
        self.sup.shutdown_all(TERMINATE_GRACE);
    }

    /// Terminate all sessions of one workspace (used when the workspace is removed).
    pub fn terminate_workspace(&self, workspace_id: &str) {
        let ids: Vec<SessionId> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.workspace_id == workspace_id)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let _ = self.sup.terminate(id, TERMINATE_GRACE);
        }
    }

    pub fn session_count(&self) -> usize {
        self.sup.list().len()
    }

    fn flow(&self, id: SessionId) -> Result<Arc<Flow>, String> {
        self.entries
            .lock()
            .unwrap()
            .get(&id)
            .map(|e| e.flow.clone())
            .ok_or_else(|| "session not found".to_string())
    }
}

/// What to start in a new session. Built only by the backend from validated data.
pub(crate) struct Launch {
    pub spec_program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub kind: SessionKind,
    pub title: String,
    pub adapter_id: Option<String>,
    /// Start somewhere other than the workspace folder. Only ever set by the backend (never from the webview).
    pub cwd: Option<PathBuf>,
}

/// Start a PTY session in a workspace, record its metadata, and forward its output.
pub(crate) fn start_session(
    terminals: &Arc<Terminals>,
    workspace_id: &str,
    launch: Launch,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    let ws = terminals
        .db
        .workspace(workspace_id)
        .map_err(|e| e.to_string())?;
    let cwd = launch.cwd.unwrap_or_else(|| PathBuf::from(&ws.root_path));
    if !cwd.is_dir() {
        return Err(format!("The workspace folder is missing: {}", ws.root_path));
    }
    let spec = SpawnSpec {
        program: launch.spec_program,
        args: launch.args,
        cwd,
        env: launch.env,
        rows,
        cols,
    };
    let (id, rx) = terminals.sup.spawn(&spec).map_err(|e| e.to_string())?;
    let db_id = terminals
        .db
        .insert_session(
            &ws.id,
            launch.kind,
            &launch.title,
            launch.adapter_id.as_deref(),
        )
        .map_err(|e| e.to_string());
    let flow = Arc::new(Flow::new());
    terminals.entries.lock().unwrap().insert(
        id,
        Entry {
            flow: flow.clone(),
            workspace_id: ws.id.clone(),
        },
    );

    let hub = terminals.clone();
    std::thread::Builder::new()
        .name(format!("forward-{id}"))
        .spawn(move || {
            forward(&rx, &flow, |out| match out {
                Outbound::Data(d) => on_output.send(InvokeResponseBody::Raw(d)).is_ok(),
                Outbound::Exited(i) => on_exit
                    .send(ExitPayload {
                        code: i.code,
                        signal: i.signal,
                    })
                    .is_ok(),
            });
            flow.close();
            hub.entries.lock().unwrap().remove(&id);
            if let Ok(db_id) = db_id {
                let _ = hub.db.finish_session(&db_id, "exited");
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(id.0)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn terminal_spawn(
    terminals: State<'_, SharedTerminals>,
    workspace_id: String,
    title: Option<String>,
    at_repo_root: Option<bool>,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    // "Open repository in terminal": the backend works out the repository root itself; the webview
    // can never name a directory.
    let cwd = if at_repo_root == Some(true) {
        let ws = terminals
            .db
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
    let settings = naurix_core::settings::get(&terminals.db).map_err(|e| e.to_string())?;
    let (program, args) = naurix_core::settings::shell_command(
        &settings.terminal,
        std::env::var("SHELL").ok().as_deref(),
    );
    let launch = Launch {
        spec_program: program,
        args,
        env: vec![],
        kind: SessionKind::Terminal,
        title: title.unwrap_or_else(|| "Terminal".into()),
        adapter_id: None,
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
        .sup
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
        .sup
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
        .sup
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
    terminals.flow(SessionId(id))?.ack(bytes);
    Ok(())
}

/// Whether a command is running in the foreground (to confirm before closing). Unknown => false.
#[tauri::command]
pub async fn terminal_has_foreground_job(
    terminals: State<'_, SharedTerminals>,
    id: u64,
) -> Result<bool, String> {
    Ok(terminals
        .sup
        .has_foreground_job(SessionId(id))
        .unwrap_or(false))
}

#[tauri::command]
pub async fn terminal_terminate(
    terminals: State<'_, SharedTerminals>,
    id: u64,
) -> Result<(), String> {
    let hub = terminals.inner().clone();
    // terminate() blocks for up to grace + 2 s; keep it off the async worker.
    tauri::async_runtime::spawn_blocking(move || {
        hub.sup
            .terminate(SessionId(id), TERMINATE_GRACE)
            .map(|_| ())
            .map_err(|e| e.to_string())
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
