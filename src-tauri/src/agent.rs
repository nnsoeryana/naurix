//! Tauri commands for AI agents. The frontend names an agent id; the backend resolves the
//! executable, arguments and environment itself, so the webview can never ask for an arbitrary
//! program to be run.

use crate::approval::SharedGate;
use crate::mcp::SharedSecrets;
use crate::terminal::{start_session, ExitPayload, SharedTerminals};
use naurix_core::agents::{
    adapter, launch_action, plan_launch, search_dirs, AgentCapability, AgentDef, AgentKind,
    AgentStore, CapabilityInfo, CustomAgent, Feature,
};
use naurix_core::approval::{ApprovalGate, ApprovalRequest, Verdict};
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
    Required { request: ApprovalRequest },
}

/// A launch failure the UI can act on: either an approval is needed, or a plain message.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchError {
    ApprovalRequired { request: Box<ApprovalRequest> },
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

/// Ask whether this agent may start in this workspace. Does not spend a "once" grant. When approval
/// is needed the answer carries a request whose display text was built here, to show to the user.
#[tauri::command]
pub async fn agent_authorize(
    db: State<'_, Arc<Db>>,
    gate: State<'_, SharedGate>,
    workspace_id: String,
    agent_id: String,
) -> Result<AuthorizeDto, String> {
    let (_, _, action) = action_for(&db, &workspace_id, &agent_id)?;
    Ok(match gate.preview(&action).map_err(err)? {
        Verdict::Allowed { .. } => AuthorizeDto::Allowed,
        Verdict::Required(request) => AuthorizeDto::Required { request },
    })
}

/// Launch an agent in a PTY inside a workspace. The approval gate decides: with no matching grant
/// this fails with `ApprovalRequired` and starts nothing.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn agent_launch(
    db: State<'_, Arc<Db>>,
    secrets: State<'_, SharedSecrets>,
    terminals: State<'_, SharedTerminals>,
    gate: State<'_, SharedGate>,
    workspace_id: String,
    agent_id: String,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, LaunchError> {
    let (def, plan, action) = action_for(&db, &workspace_id, &agent_id)?;
    let approval_id = match gate.authorize(&action).map_err(err)? {
        Verdict::Allowed { approval_id } => approval_id,
        Verdict::Required(request) => {
            return Err(LaunchError::ApprovalRequired {
                request: Box::new(request),
            })
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
        cwd: None,
    };
    Ok(start_session(
        terminals.inner(),
        &workspace_id,
        launch,
        rows,
        cols,
        on_output,
        on_exit,
    )?)
}

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
