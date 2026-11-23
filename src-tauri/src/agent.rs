//! Tauri commands for AI agents. The frontend names an agent id; the backend resolves the
//! executable, arguments and environment itself, so the webview can never ask for an arbitrary
//! program to be run.

use crate::approval::SharedGate;
use crate::mcp::{DataDir, SharedSecrets};
use crate::terminal::{start_session, ExitPayload, SharedTerminals};
use naurix_core::agents::{
    adapter, launch_action, plan_launch, search_dirs, AgentCapability, AgentDef, AgentKind,
    AgentStore, CapabilityInfo, CustomAgent, Feature,
};
use naurix_core::approval::{ApprovalGate, ApprovalRequest, Verdict};
use naurix_core::git::worktree::{self, CreatePlan, WorktreeStore};
use naurix_core::persistence::iso8601_utc_now;
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::session::SessionLaunch;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvDto {
    name: String,
    value: String,
}

/// What Naurix knows about an agent and how well it is verified (see docs/AGENT_ADAPTERS.md).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDto {
    capabilities: &'static [CapabilityInfo],
    notice: Option<&'static str>,
    verified_with: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDto {
    id: String,
    name: String,
    kind: AgentKind,
    executable: String,
    args: Vec<String>,
    env: Vec<EnvDto>,
    /// Where the executable was found; null when it is not installed / not found.
    resolved_path: Option<String>,
    path_override: Option<String>,
    /// The user approved this exact executable for every workspace (presets only).
    approved: bool,
    /// The command line shown in the review dialog (null when not found).
    command: Option<String>,
    capabilities: Vec<AgentCapability>,
    adapter: Option<AdapterDto>,
}

pub(crate) fn dirs() -> Vec<PathBuf> {
    let home = naurix_core::platform::home_dir_or_empty();
    search_dirs(std::env::var_os("PATH").as_deref(), &home)
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn describe(
    store: &AgentStore<'_>,
    gate: &ApprovalGate,
    def: &AgentDef,
    dirs: &[PathBuf],
) -> Result<AgentDto, String> {
    let over = store.path_override(&def.id).map_err(err)?;
    let plan = plan_launch(def, over.as_deref(), dirs).ok();
    let approved = match (&plan, def.kind) {
        (Some(p), AgentKind::Preset) => gate
            .has_app_grant(&launch_action(def, &p.program, ""))
            .map_err(err)?,
        _ => false,
    };
    Ok(AgentDto {
        id: def.id.clone(),
        name: def.name.clone(),
        kind: def.kind,
        executable: def.executable.clone(),
        args: def.args.clone(),
        env: def
            .env
            .iter()
            .map(|(n, v)| EnvDto {
                name: n.clone(),
                value: v.clone(),
            })
            .collect(),
        resolved_path: plan
            .as_ref()
            .map(|p| p.program.to_string_lossy().into_owned()),
        path_override: over,
        approved,
        command: plan.map(|p| p.display),
        capabilities: def.capabilities().to_vec(),
        adapter: adapter(&def.id).map(|a| AdapterDto {
            capabilities: a.capabilities,
            notice: a.notice,
            verified_with: a.verified_with,
        }),
    })
}

fn list(db: &Db, gate: &ApprovalGate) -> Result<Vec<AgentDto>, String> {
    let store = AgentStore::new(db);
    let d = dirs();
    store
        .all()
        .map_err(err)?
        .iter()
        .map(|a| describe(&store, gate, a, &d))
        .collect()
}

#[tauri::command]
pub async fn agent_list(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
) -> Result<Vec<AgentDto>, String> {
    list(&db, &gate)
}

#[tauri::command]
pub async fn agent_set_path(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    id: String,
    path: Option<String>,
) -> Result<Vec<AgentDto>, String> {
    AgentStore::new(&db)
        .set_path_override(&id, path.as_deref())
        .map_err(err)?;
    list(&db, &gate)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInput {
    name: String,
    value: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInput {
    id: Option<String>,
    name: String,
    executable: String,
    args: Vec<String>,
    env: Vec<EnvInput>,
}

#[tauri::command]
pub async fn agent_custom_save(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    agent: CustomInput,
) -> Result<Vec<AgentDto>, String> {
    let c = CustomAgent {
        id: agent.id.unwrap_or_default(),
        name: agent.name,
        executable: agent.executable,
        args: agent.args,
        env: agent
            .env
            .into_iter()
            .map(|e| naurix_core::agents::EnvVar {
                name: e.name,
                value: e.value,
            })
            .collect(),
    };
    AgentStore::new(&db).save_custom(c).map_err(err)?;
    list(&db, &gate)
}

#[tauri::command]
pub async fn agent_custom_delete(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    id: String,
) -> Result<Vec<AgentDto>, String> {
    AgentStore::new(&db).delete_custom(&id).map_err(err)?;
    list(&db, &gate)
}

/// Result of asking whether an agent may start now, without spending anything.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthorizeDto {
    Allowed,
    /// Everything that still needs a decision, each with a review the backend built. With a worktree
    /// that is up to two: the agent launch, and the worktree creation (once only).
    Required {
        requests: Vec<ApprovalRequest>,
    },
}

/// A launch failure the UI can act on: either approval is needed, or a plain message.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchError {
    ApprovalRequired { requests: Vec<ApprovalRequest> },
    Error { message: String },
}

impl From<String> for LaunchError {
    fn from(message: String) -> Self {
        Self::Error { message }
    }
}

/// The launch action for this agent in this workspace, built entirely from backend data.
fn action_for(
    db: &Db,
    workspace_id: &str,
    agent_id: &str,
) -> Result<
    (
        AgentDef,
        naurix_core::agents::LaunchPlan,
        naurix_core::approval::Action,
    ),
    String,
> {
    let (def, plan) = AgentStore::new(db)
        .prepare_launch(agent_id, &dirs())
        .map_err(err)?;
    let ws = db.workspace(workspace_id).map_err(err)?;
    if !std::path::Path::new(&ws.root_path).is_dir() {
        return Err(format!("The workspace folder is missing: {}", ws.root_path));
    }
    let action = launch_action(&def, &plan.program, workspace_id);
    Ok((def, plan, action))
}

/// Decide where a worktree for this launch would go (changes nothing) and the approval it needs.
fn worktree_plan(
    db: &Arc<Db>,
    dir: &DataDir,
    workspace_id: &str,
    agent_id: &str,
) -> Result<(CreatePlan, naurix_core::approval::Action), String> {
    let ws = db.workspace(workspace_id).map_err(err)?;
    let trusted = |root: &std::path::Path| naurix_core::git::is_trusted(db, root);
    let plan = worktree::plan_create(
        std::path::Path::new(&ws.root_path),
        &trusted,
        &crate::worktree::root_dir(dir),
        workspace_id,
        agent_id,
        &iso8601_utc_now(),
    )
    .map_err(err)?;
    let action = worktree::create_action(&plan, workspace_id);
    Ok((plan, action))
}

fn required(v: Verdict) -> Option<ApprovalRequest> {
    match v {
        Verdict::Required(r) => Some(r),
        Verdict::Allowed { .. } => None,
    }
}

/// Ask whether this agent may start in this workspace, optionally in its own worktree. Spends nothing.
/// When approval is needed the answer carries the requests, each with a review built here.
#[tauri::command]
pub async fn agent_authorize(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    dir: State<'_, DataDir>,
    workspace_id: String,
    agent_id: String,
    worktree: Option<bool>,
) -> Result<AuthorizeDto, String> {
    let (_, _, action) = action_for(&db, &workspace_id, &agent_id)?;
    let mut requests: Vec<ApprovalRequest> = required(gate.preview(&action).map_err(err)?)
        .into_iter()
        .collect();
    if worktree == Some(true) {
        let (db, dir, gate) = (
            db.inner().clone(),
            DataDir(dir.0.clone()),
            gate.inner().clone(),
        );
        let wt = tauri::async_runtime::spawn_blocking(move || {
            let (_, a) = worktree_plan(&db, &dir, &workspace_id, &agent_id)?;
            Ok::<_, String>(required(gate.preview(&a).map_err(err)?))
        })
        .await
        .map_err(err)??;
        requests.extend(wt);
    }
    Ok(if requests.is_empty() {
        AuthorizeDto::Allowed
    } else {
        AuthorizeDto::Required { requests }
    })
}

/// Launch an agent in a PTY inside a workspace, optionally in a fresh worktree. The approval gate
/// decides: without a matching grant for every action involved this fails with `ApprovalRequired` and
/// starts nothing (and creates nothing).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn agent_launch(
    db: State<'_, Arc<Db>>,
    secrets: State<'_, SharedSecrets>,
    terminals: State<'_, SharedTerminals>,
    gate: State<'_, SharedGate>,
    dir: State<'_, DataDir>,
    workspace_id: String,
    agent_id: String,
    worktree: Option<bool>,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, LaunchError> {
    let (def, plan, action) = action_for(&db, &workspace_id, &agent_id)?;
    // Refuse before spending a one-time approval. `start` checks again atomically.
    terminals.ensure_agent_slot().map_err(err)?;

    // Look at every approval first, without spending any, so a missing one never wastes another.
    let wt = if worktree == Some(true) {
        let (db, dir, ws, ag) = (
            db.inner().clone(),
            DataDir(dir.0.clone()),
            workspace_id.clone(),
            agent_id.clone(),
        );
        Some(
            tauri::async_runtime::spawn_blocking(move || worktree_plan(&db, &dir, &ws, &ag))
                .await
                .map_err(err)??,
        )
    } else {
        None
    };
    let mut missing: Vec<ApprovalRequest> = required(gate.preview(&action).map_err(err)?)
        .into_iter()
        .collect();
    if let Some((_, a)) = &wt {
        missing.extend(required(gate.preview(a).map_err(err)?));
    }
    if !missing.is_empty() {
        return Err(LaunchError::ApprovalRequired { requests: missing });
    }

    let approval_id = match gate.authorize(&action).map_err(err)? {
        Verdict::Allowed { approval_id } => approval_id,
        Verdict::Required(r) => return Err(LaunchError::ApprovalRequired { requests: vec![r] }),
    };
    // Create the worktree last, right before the session, so nothing irreversible happens for a launch
    // that was going to be refused.
    let (cwd, worktree_id) = match wt {
        None => (None, None),
        Some((plan, a)) => {
            if let Verdict::Required(r) = gate.authorize(&a).map_err(err)? {
                return Err(LaunchError::ApprovalRequired { requests: vec![r] });
            }
            let db2 = db.inner().clone();
            let ws = workspace_id.clone();
            let rec = tauri::async_runtime::spawn_blocking(move || {
                let trusted = |root: &std::path::Path| naurix_core::git::is_trusted(&db2, root);
                worktree::create(&plan, &trusted).map_err(err)?;
                WorktreeStore::new(&db2).insert(&ws, &plan).map_err(err)
            })
            .await
            .map_err(err)??;
            (Some(std::path::PathBuf::from(&rec.path)), Some(rec.id))
        }
    };
    // MCP secrets reach the agent as environment variables (its config file only holds references),
    // and only for agents whose MCP config handling was verified (docs/AGENT_ADAPTERS.md).
    let mut env = plan.env.clone();
    if adapter(&def.id).is_some_and(|a| a.acts_on(Feature::McpConfig)) {
        env.extend(
            naurix_core::mcp::McpStore::new(&db, &**secrets)
                .launch_environment()
                .map_err(err)?,
        );
    }
    let launch = SessionLaunch {
        program: plan.program.to_string_lossy().into_owned(),
        args: plan.args.clone(),
        env,
        kind: SessionKind::Agent,
        title: def.name.clone(),
        adapter_id: Some(def.id.clone()),
        approval_id: Some(approval_id),
        worktree_id: worktree_id.clone(),
        cwd: cwd.clone(),
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
    .map_err(|e| match cwd {
        // The worktree was created but the agent could not start. It is not removed automatically
        // (removal is a Git change that needs its own approval), so say where it is.
        Some(p) => LaunchError::Error {
            message: format!(
                "{e} The worktree was already created at {} and is listed in the Git view; remove it there if you do not need it.",
                p.display()
            ),
        },
        None => e.into(),
    })
}

/// Agents already running in this workspace's own folder. See `worktree.rs`.
/// The agent's reported version. Only for a preset whose exact executable the user approved for every
/// workspace, because this runs `<executable> --version`.
#[tauri::command]
pub async fn agent_version(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    id: String,
) -> Result<Option<String>, String> {
    let (db, gate) = (db.inner().clone(), gate.inner().clone());
    tauri::async_runtime::spawn_blocking(move || {
        AgentStore::new(&db)
            .version_if_approved(&id, &dirs(), &gate)
            .map_err(err)
    })
    .await
    .map_err(err)?
}
