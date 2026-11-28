//! Tauri commands for workflows and runs (ADR 0030). The webview names workflow, run and step ids and
//! the user's own text; it can never name a program, an argument or a path. Starting a run needs the
//! reviewed once-approval; the engine in Core does the rest.

use crate::mcp::SharedSecrets;
use naurix_core::approval::{Action, ApprovalRequest};
use naurix_core::persistence::Db;
use naurix_core::pty::SessionId;
use naurix_core::session::SessionError;
use naurix_core::workflow::{
    Engine, LaunchProblem, Launcher, Run, StartVerdict, StepInput, Workflow, WorkflowStore,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

pub type SharedEngine = Arc<Engine>;

fn err(e: impl ToString) -> String {
    e.to_string()
}

/// The agent launch path, for steps. Same resolution, MCP environment and status hooks as a launch from
/// the UI; the session starts detached and is attached to a pane only if the user opens it.
pub struct ShellLauncher {
    pub db: Arc<Db>,
    pub terminals: crate::terminal::SharedTerminals,
    pub secrets: SharedSecrets,
    pub data_dir: PathBuf,
}

impl ShellLauncher {
    fn problem(not_found: bool) -> LaunchProblem {
        if not_found {
            LaunchProblem::NotFound
        } else {
            LaunchProblem::Failed
        }
    }
}

impl Launcher for ShellLauncher {
    fn action(&self, workspace_id: &str, agent_id: &str) -> Result<Action, LaunchProblem> {
        crate::agent::resolve(&self.db, workspace_id, agent_id)
            .map(|(_, _, a)| a)
            .map_err(|(nf, _)| Self::problem(nf))
    }

    fn launch(
        &self,
        workspace_id: &str,
        agent_id: &str,
        approval_id: &str,
    ) -> Result<SessionId, LaunchProblem> {
        let (def, plan, _) = crate::agent::resolve(&self.db, workspace_id, agent_id)
            .map_err(|(nf, _)| Self::problem(nf))?;
        if self.terminals.ensure_agent_slot().is_err() {
            return Err(LaunchProblem::Busy);
        }
        let (launch, spool) = crate::agent::build_launch(
            &self.db,
            &self.secrets,
            &self.data_dir,
            &def,
            &plan,
            approval_id.to_string(),
            None,
            None,
        )
        .map_err(|_| LaunchProblem::Failed)?;
        let cleanup = |p: &Option<PathBuf>| {
            if let Some(p) = p {
                let _ = std::fs::remove_file(p);
            }
        };
        match self
            .terminals
            .start(workspace_id, launch, 24, 80, |_| false)
        {
            Ok(id) => {
                if let Some(sp) = &spool {
                    if self.terminals.watch_status(id, sp.clone()).is_err() {
                        cleanup(&spool);
                    }
                }
                Ok(id)
            }
            Err(SessionError::AgentLimit { .. }) => {
                cleanup(&spool);
                Err(LaunchProblem::Busy)
            }
            Err(_) => {
                cleanup(&spool);
                Err(LaunchProblem::Failed)
            }
        }
    }

    fn success_exit_codes(&self, _workspace_id: &str, agent_id: &str) -> Vec<u32> {
        naurix_core::agents::AgentStore::new(&self.db)
            .get(agent_id)
            .map(|d| d.success_codes())
            .unwrap_or_else(|_| vec![0])
    }
}

/// Build the engine for a data folder and start following session exits. Runs left over from a previous
/// start are marked interrupted first.
pub fn start_engine(
    db: Arc<Db>,
    gate: Arc<naurix_core::approval::ApprovalGate>,
    terminals: crate::terminal::SharedTerminals,
    secrets: SharedSecrets,
    data_dir: PathBuf,
) -> SharedEngine {
    let launcher = ShellLauncher {
        db: db.clone(),
        terminals: terminals.clone(),
        secrets,
        data_dir,
    };
    let engine = Engine::new(db, gate, terminals, Box::new(launcher));
    engine.mark_interrupted();
    engine.spawn_listener();
    engine
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepDto {
    name: String,
    agent_id: String,
    depends_on: Vec<usize>,
}

/// A run plus the live session of each running step (the id the UI attaches a pane to).
#[derive(Serialize)]
pub struct RunDto {
    #[serde(flatten)]
    run: Run,
    sessions: HashMap<String, u64>,
}

fn run_dto(engine: &Engine, run: Run) -> RunDto {
    let sessions = run
        .steps
        .iter()
        .filter_map(|s| engine.session_of(&s.id).map(|sid| (s.id.clone(), sid.0)))
        .collect();
    RunDto { run, sessions }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunStartDto {
    Required { request: Box<ApprovalRequest> },
    Started { run: Box<RunDto> },
}

#[tauri::command]
pub async fn workflow_list(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
) -> Result<Vec<Workflow>, String> {
    WorkflowStore::new(&db).list(&workspace_id).map_err(err)
}

#[tauri::command]
pub async fn workflow_save(
    db: State<'_, Arc<Db>>,
    workspace_id: String,
    id: Option<String>,
    name: String,
    steps: Vec<StepDto>,
) -> Result<Workflow, String> {
    let steps: Vec<StepInput> = steps
        .into_iter()
        .map(|s| StepInput {
            name: s.name,
            agent_id: s.agent_id,
            depends_on: s.depends_on,
        })
        .collect();
    WorkflowStore::new(&db)
        .save(&workspace_id, id.as_deref(), &name, &steps)
        .map_err(err)
}

#[tauri::command]
pub async fn workflow_delete(db: State<'_, Arc<Db>>, id: String) -> Result<(), String> {
    WorkflowStore::new(&db).delete(&id).map_err(err)
}

/// Start a run. Without the reviewed approval this returns it as a request and starts nothing.
#[tauri::command]
pub async fn workflow_run(
    engine: State<'_, SharedEngine>,
    workflow_id: String,
) -> Result<RunStartDto, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(match e.start(&workflow_id).map_err(err)? {
            StartVerdict::Required(request) => RunStartDto::Required { request },
            StartVerdict::Started(run) => RunStartDto::Started {
                run: Box::new(run_dto(&e, *run)),
            },
        })
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn run_list(
    engine: State<'_, SharedEngine>,
    workspace_id: String,
) -> Result<Vec<RunDto>, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let runs = e.list_runs(&workspace_id, 20).map_err(err)?;
        Ok(runs.into_iter().map(|r| run_dto(&e, r)).collect())
    })
    .await
    .map_err(err)?
}

macro_rules! run_action {
    ($name:ident, $method:ident) => {
        #[tauri::command]
        pub async fn $name(
            engine: State<'_, SharedEngine>,
            run_id: String,
            step_id: String,
        ) -> Result<RunDto, String> {
            let e = engine.inner().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let run = e.$method(&run_id, &step_id).map_err(err)?;
                Ok(run_dto(&e, run))
            })
            .await
            .map_err(err)?
        }
    };
}
run_action!(run_mark_done, mark_done);
run_action!(run_skip, skip);

#[tauri::command]
pub async fn run_cancel(engine: State<'_, SharedEngine>, run_id: String) -> Result<RunDto, String> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let run = e.cancel(&run_id).map_err(err)?;
        Ok(run_dto(&e, run))
    })
    .await
    .map_err(err)?
}
