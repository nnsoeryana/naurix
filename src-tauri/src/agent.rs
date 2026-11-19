//! Tauri commands for AI agents. The frontend names an agent id; the backend resolves the
//! executable, arguments and environment itself, so the webview can never ask for an arbitrary
//! program to be run.

use crate::mcp::SharedSecrets;
use crate::terminal::{start_session, ExitPayload, Launch, SharedTerminals};
use naurix_core::agents::{
    adapter, plan_launch, search_dirs, AgentCapability, AgentDef, AgentKind, AgentStore,
    CapabilityInfo, CustomAgent, Feature,
};
use naurix_core::persistence::{Db, SessionKind};
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
    /// Already reviewed for this exact executable (presets only).
    approved: bool,
    /// The command line shown in the review dialog (null when not found).
    command: Option<String>,
    capabilities: Vec<AgentCapability>,
    adapter: Option<AdapterDto>,
}

pub(crate) fn dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    search_dirs(std::env::var_os("PATH").as_deref(), &home)
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn describe(store: &AgentStore<'_>, def: &AgentDef, dirs: &[PathBuf]) -> Result<AgentDto, String> {
    let over = store.path_override(&def.id).map_err(err)?;
    let plan = plan_launch(def, over.as_deref(), dirs).ok();
    let approved = match (&plan, def.kind) {
        (Some(p), AgentKind::Preset) => {
            store.approved_program(&def.id).map_err(err)?.as_deref()
                == Some(&*p.program.to_string_lossy())
        }
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

fn list(db: &Db) -> Result<Vec<AgentDto>, String> {
    let store = AgentStore::new(db);
    let d = dirs();
    store
        .all()
        .map_err(err)?
        .iter()
        .map(|a| describe(&store, a, &d))
        .collect()
}

#[tauri::command]
pub async fn agent_list(db: State<'_, Arc<Db>>) -> Result<Vec<AgentDto>, String> {
    list(&db)
}

#[tauri::command]
pub async fn agent_set_path(
    db: State<'_, Arc<Db>>,
    id: String,
    path: Option<String>,
) -> Result<Vec<AgentDto>, String> {
    AgentStore::new(&db)
        .set_path_override(&id, path.as_deref())
        .map_err(err)?;
    list(&db)
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
    list(&db)
}

#[tauri::command]
pub async fn agent_custom_delete(
    db: State<'_, Arc<Db>>,
    id: String,
) -> Result<Vec<AgentDto>, String> {
    AgentStore::new(&db).delete_custom(&id).map_err(err)?;
    list(&db)
}

/// Launch an agent in a PTY inside a workspace. `reviewed` must be true unless this preset was
/// already approved for this exact executable (enforced in the core crate).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn agent_launch(
    db: State<'_, Arc<Db>>,
    secrets: State<'_, SharedSecrets>,
    terminals: State<'_, SharedTerminals>,
    workspace_id: String,
    agent_id: String,
    reviewed: bool,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
    on_exit: Channel<ExitPayload>,
) -> Result<u64, String> {
    let store = AgentStore::new(&db);
    let (def, plan) = store
        .prepare_launch(&agent_id, &dirs(), reviewed)
        .map_err(err)?;
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
    let launch = Launch {
        spec_program: plan.program.to_string_lossy().into_owned(),
        args: plan.args.clone(),
        env,
        kind: SessionKind::Agent,
        title: def.name.clone(),
        adapter_id: Some(def.id.clone()),
        cwd: None,
    };
    let id = start_session(
        terminals.inner(),
        &workspace_id,
        launch,
        rows,
        cols,
        on_output,
        on_exit,
    )?;
    if def.kind == AgentKind::Preset {
        // Remember the review for this exact executable (not for other paths).
        let _ = store.approve(&def.id, &plan.program);
    }
    Ok(id)
}

/// The agent's reported version. Only for an executable the user has already approved, because this
/// runs `<executable> --version`.
#[tauri::command]
pub async fn agent_version(db: State<'_, Arc<Db>>, id: String) -> Result<Option<String>, String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        AgentStore::new(&db)
            .version_of_approved(&id, &dirs())
            .map_err(err)
    })
    .await
    .map_err(err)?
}
