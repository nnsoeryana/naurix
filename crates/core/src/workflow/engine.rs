//! Runs workflows (ADR 0030). The engine owns no processes: it asks a [`Launcher`] (the shell's agent
//! launch path) to start sessions, reacts to the exit of those sessions, and moves each step to a
//! state from the process alone. It never reads output, never sends input and never guesses.

use super::store::{RunStore, WorkflowStore};
use super::*;
use crate::approval::{Action, ActionClass, ApprovalGate, ApprovalRequest, Scope, Verdict};
use crate::events::Event;
use crate::persistence::Db;
use crate::pty::SessionId;
use crate::session::SessionManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Why a step could not be launched. Mapped to a fixed note on the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchProblem {
    /// No such agent, or its executable is not installed.
    NotFound,
    /// No agent slot is free right now. The step stays pending and is retried when a session exits.
    Busy,
    /// Anything else (the workspace folder is gone, the session failed to start).
    Failed,
}

/// The shell's agent launch path, as the engine needs it.
pub trait Launcher: Send + Sync {
    /// The approval action a launch of `agent_id` in this workspace needs. Pure: starts and creates nothing.
    fn action(&self, workspace_id: &str, agent_id: &str) -> Result<Action, LaunchProblem>;
    /// Start `agent_id` detached (no output consumer) and return its session. `approval_id` is the grant
    /// that allowed it, recorded on the session row.
    fn launch(
        &self,
        workspace_id: &str,
        agent_id: &str,
        approval_id: &str,
    ) -> Result<SessionId, LaunchProblem>;
    /// Exit codes that mean this agent's step succeeded (ADR 0032 §7). They are part of the launch
    /// fingerprint the run approval covers, so they cannot change unreviewed.
    fn success_exit_codes(&self, _workspace_id: &str, _agent_id: &str) -> Vec<u32> {
        vec![0]
    }
}

/// The answer to "start this workflow".
#[derive(Debug)]
pub enum StartVerdict {
    /// The run approval is needed. Decide it, then start again.
    Required(Box<ApprovalRequest>),
    Started(Box<Run>),
}

/// A step whose session is running.
struct LiveStep {
    run_id: String,
    step_id: String,
    /// Exit codes that count as success (ADR 0032 §7).
    success: Vec<u32>,
}

pub struct Engine {
    db: Arc<Db>,
    gate: Arc<ApprovalGate>,
    sessions: Arc<SessionManager>,
    launcher: Box<dyn Launcher>,
    /// Serialises every change to run state, so a step that exits while another is being started is
    /// handled after the start finished.
    lock: Mutex<()>,
    /// Running steps by session.
    live: Mutex<HashMap<SessionId, LiveStep>>,
}

impl Engine {
    pub fn new(
        db: Arc<Db>,
        gate: Arc<ApprovalGate>,
        sessions: Arc<SessionManager>,
        launcher: Box<dyn Launcher>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            gate,
            sessions,
            launcher,
            lock: Mutex::new(()),
            live: Mutex::new(HashMap::new()),
        })
    }

    /// Follow session exits for the lifetime of the session manager. Call once, after `new`.
    pub fn spawn_listener(self: &Arc<Self>) {
        let rx = self.sessions.events().subscribe();
        let me = self.clone();
        let _ = std::thread::Builder::new()
            .name("workflow-engine".into())
            .spawn(move || {
                for e in rx {
                    if let Event::SessionExited {
                        session,
                        code,
                        signal,
                        ..
                    } = e
                    {
                        me.on_exit(session, code, signal.is_some());
                        // Any session ending can free an agent slot, not only a workflow's own, so
                        // steps that were waiting for a slot get another chance.
                        me.retry_waiting();
                    }
                }
            });
    }

    fn publish(&self, run: &Run) {
        self.sessions.events().publish(Event::RunChanged {
            run_id: run.id.clone(),
            workspace_id: run.workspace_id.clone(),
        });
    }

    /// Every step's launch action, or the first step that cannot be launched.
    fn step_actions(&self, wf: &Workflow) -> Result<Vec<Action>, WorkflowError> {
        wf.steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                self.launcher
                    .action(&wf.workspace_id, &s.agent_id)
                    .map_err(|p| {
                        WorkflowError::Invalid(match p {
                            LaunchProblem::NotFound => format!(
                                "Step {} ({}): that agent is not installed or no longer exists.",
                                i + 1,
                                s.name
                            ),
                            LaunchProblem::Busy | LaunchProblem::Failed => {
                                format!("Step {} ({}) cannot be started right now.", i + 1, s.name)
                            }
                        })
                    })
            })
            .collect()
    }

    /// The once-only approval for running `wf`: lists every step's exact command and the dependencies.
    pub fn run_action(&self, wf: &Workflow) -> Result<(Action, Vec<Action>), WorkflowError> {
        let steps = self.step_actions(wf)?;
        let index = |id: &str| {
            wf.steps
                .iter()
                .position(|s| s.id == id)
                .unwrap_or(usize::MAX)
        };
        let mut parts: Vec<String> = vec![wf.steps.len().to_string()];
        let mut display = format!(
            "Run the workflow “{}” ({} steps).\n",
            wf.name,
            wf.steps.len()
        );
        for (i, (s, a)) in wf.steps.iter().zip(&steps).enumerate() {
            let deps: Vec<String> = s
                .depends_on
                .iter()
                .map(|d| (index(d) + 1).to_string())
                .collect();
            parts.push(a.fingerprint.clone());
            parts.push(deps.join(","));
            display.push_str(&format!(
                "\nStep {}: {}{}\n{}\n",
                i + 1,
                s.name,
                if deps.is_empty() {
                    String::new()
                } else {
                    format!(" (after step {})", deps.join(", "))
                },
                a.display
            ));
        }
        display.push_str(
            "\nApproving this lets these steps start by themselves as the steps before them finish, only for this run. \
             A step that needs something not shown here fails instead of asking. A step is done when its program exits \
             (0 means success, unless the step lists other success exit codes) or when you press Mark done. Nothing here sends input to an agent.",
        );
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        let action = Action::new(
            ActionClass::RunWorkflow,
            &wf.workspace_id,
            &refs,
            format!("Workflow “{}”", wf.name),
            display,
            vec![Scope::Once],
        );
        Ok((action, steps))
    }

    /// Start a run, if the reviewed approval is there. Otherwise return the request to decide.
    pub fn start(&self, workflow_id: &str) -> Result<StartVerdict, WorkflowError> {
        let _g = self.lock.lock().unwrap();
        let wf = WorkflowStore::new(&self.db).get(workflow_id)?;
        if RunStore::new(&self.db).has_active_run(workflow_id)? {
            return Err(WorkflowError::Active);
        }
        let (action, steps) = self.run_action(&wf)?;
        match self
            .gate
            .authorize(&action)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?
        {
            Verdict::Required(r) => Ok(StartVerdict::Required(Box::new(r))),
            Verdict::Allowed { .. } => {
                let run = RunStore::new(&self.db).create(&wf)?;
                let refs: Vec<&Action> = steps.iter().collect();
                self.gate
                    .grant_run(&run.id, &refs)
                    .map_err(|e| WorkflowError::Storage(e.to_string()))?;
                self.advance_locked(&run.id)?;
                let run = RunStore::new(&self.db).get(&run.id)?;
                self.publish(&run);
                Ok(StartVerdict::Started(Box::new(run)))
            }
        }
    }

    /// Start every step that is ready and fits under the agent limit, then settle the run if it is over.
    fn advance_locked(&self, run_id: &str) -> Result<(), WorkflowError> {
        let store = RunStore::new(&self.db);
        loop {
            let run = store.get(run_id)?;
            if run.state != RunState::Running {
                return Ok(());
            }
            let ready: Vec<RunStep> = run.ready().into_iter().cloned().collect();
            let mut progressed = false;
            for step in ready {
                if self.sessions.ensure_agent_slot().is_err() {
                    break; // retried when a session exits
                }
                self.start_step(&run, &step)?;
                if store
                    .get(run_id)?
                    .steps
                    .iter()
                    .any(|s| s.id == step.id && s.state != StepState::Pending)
                {
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        let run = store.get(run_id)?;
        if run.state == RunState::Running {
            if let Some(end) = run.settled() {
                store.set_run_state(run_id, end)?;
                let _ = self.gate.end_run(run_id);
            }
        }
        Ok(())
    }

    fn start_step(&self, run: &Run, step: &RunStep) -> Result<(), WorkflowError> {
        let store = RunStore::new(&self.db);
        let fail = |n: &str| {
            store
                .finish_step(&step.id, StepState::Failed, None, Some(n))
                .map(|_| ())
        };
        let action = match self.launcher.action(&run.workspace_id, &step.agent_id) {
            Ok(a) => a,
            Err(LaunchProblem::NotFound) => return fail(note::AGENT_NOT_FOUND),
            Err(LaunchProblem::Busy) => return Ok(()),
            Err(LaunchProblem::Failed) => return fail(note::COULD_NOT_START),
        };
        let approval = self
            .gate
            .authorize_in_run(&action, &run.id)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        let Some(approval_id) = approval else {
            return fail(note::APPROVAL_MISSING);
        };
        let success = self
            .launcher
            .success_exit_codes(&run.workspace_id, &step.agent_id);
        match self
            .launcher
            .launch(&run.workspace_id, &step.agent_id, &approval_id)
        {
            Ok(session) => {
                store.start_step(&step.id)?;
                self.live.lock().unwrap().insert(
                    session,
                    LiveStep {
                        run_id: run.id.clone(),
                        step_id: step.id.clone(),
                        success,
                    },
                );
                Ok(())
            }
            Err(LaunchProblem::NotFound) => fail(note::AGENT_NOT_FOUND),
            Err(LaunchProblem::Busy) => Ok(()),
            Err(LaunchProblem::Failed) => fail(note::COULD_NOT_START),
        }
    }

    fn on_exit(&self, session: SessionId, code: Option<u32>, signalled: bool) {
        let _g = self.lock.lock().unwrap();
        let Some(LiveStep {
            run_id,
            step_id,
            success,
        }) = self.live.lock().unwrap().remove(&session)
        else {
            return;
        };
        let store = RunStore::new(&self.db);
        let ok = !signalled && code.is_some_and(|c| success.contains(&c));
        let _ = store.finish_step(
            &step_id,
            if ok {
                StepState::Succeeded
            } else {
                StepState::Failed
            },
            code.map(i64::from),
            (!ok).then_some(note::EXITED),
        );
        let _ = self.advance_locked(&run_id);
        if let Ok(run) = store.get(&run_id) {
            self.publish(&run);
        }
    }

    /// Try to start steps that are ready but were held back (for example by the agent limit).
    fn retry_waiting(&self) {
        let _g = self.lock.lock().unwrap();
        let store = RunStore::new(&self.db);
        for run in store.active().unwrap_or_default() {
            if run.ready().is_empty() {
                continue;
            }
            let before: Vec<StepState> = run.steps.iter().map(|s| s.state).collect();
            let _ = self.advance_locked(&run.id);
            if let Ok(after) = store.get(&run.id) {
                if after.state != run.state || after.steps.iter().map(|s| s.state).ne(before) {
                    self.publish(&after);
                }
            }
        }
    }

    /// Catch up on exits the lossy event bus may have dropped: a step recorded as running whose session
    /// has ended is finished now.
    pub fn reconcile(&self) {
        let gone: Vec<(SessionId, Option<u32>, bool)> = {
            let live = self.live.lock().unwrap();
            let infos = self.sessions.list();
            live.keys()
                .filter_map(|sid| match infos.iter().find(|i| i.id == *sid) {
                    Some(i) => i.exit.as_ref().map(|e| (*sid, e.code, e.signal.is_some())),
                    None => Some((*sid, None, true)),
                })
                .collect()
        };
        for (sid, code, signalled) in gone {
            self.on_exit(sid, code, signalled);
        }
    }

    pub fn get_run(&self, run_id: &str) -> Result<Run, WorkflowError> {
        self.reconcile();
        RunStore::new(&self.db).get(run_id)
    }

    pub fn list_runs(&self, workspace_id: &str, limit: u32) -> Result<Vec<Run>, WorkflowError> {
        self.reconcile();
        RunStore::new(&self.db).list(workspace_id, limit)
    }

    /// The live session of a running step, for "Open terminal".
    pub fn session_of(&self, step_id: &str) -> Option<SessionId> {
        self.live
            .lock()
            .unwrap()
            .iter()
            .find(|(_, l)| l.step_id == step_id)
            .map(|(sid, _)| *sid)
    }

    fn step_in(&self, run_id: &str, step_id: &str) -> Result<(Run, RunStep), WorkflowError> {
        let run = RunStore::new(&self.db).get(run_id)?;
        let step = run
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .cloned()
            .ok_or(WorkflowError::NotFound)?;
        Ok((run, step))
    }

    /// The user says a running step is finished. The session is left alone.
    pub fn mark_done(&self, run_id: &str, step_id: &str) -> Result<Run, WorkflowError> {
        let _g = self.lock.lock().unwrap();
        let (run, step) = self.step_in(run_id, step_id)?;
        if run.state != RunState::Running || step.state != StepState::Running {
            return Err(WorkflowError::Invalid(
                "Only a running step can be marked done.".into(),
            ));
        }
        RunStore::new(&self.db).finish_step(step_id, StepState::Succeeded, None, None)?;
        self.live
            .lock()
            .unwrap()
            .retain(|_, l| l.step_id != step_id);
        self.advance_locked(run_id)?;
        let run = RunStore::new(&self.db).get(run_id)?;
        self.publish(&run);
        Ok(run)
    }

    /// The user chooses to go on without a step that has not started. Its dependents may start.
    pub fn skip(&self, run_id: &str, step_id: &str) -> Result<Run, WorkflowError> {
        let _g = self.lock.lock().unwrap();
        let (run, step) = self.step_in(run_id, step_id)?;
        if run.state != RunState::Running || step.state != StepState::Pending {
            return Err(WorkflowError::Invalid(
                "Only a step that has not started can be skipped.".into(),
            ));
        }
        RunStore::new(&self.db).finish_step(step_id, StepState::Skipped, None, None)?;
        self.advance_locked(run_id)?;
        let run = RunStore::new(&self.db).get(run_id)?;
        self.publish(&run);
        Ok(run)
    }

    /// Stop the run. Nothing new starts; steps still in flight stop being tracked and their sessions are
    /// left alone (they are ordinary sessions the user can close). Run-scoped grants end.
    pub fn cancel(&self, run_id: &str) -> Result<Run, WorkflowError> {
        let _g = self.lock.lock().unwrap();
        let store = RunStore::new(&self.db);
        let run = store.get(run_id)?;
        if run.state != RunState::Running {
            return Ok(run);
        }
        for s in run.steps.iter().filter(|s| !s.state.is_final()) {
            store.finish_step(&s.id, StepState::Cancelled, None, None)?;
        }
        self.live.lock().unwrap().retain(|_, l| l.run_id != run_id);
        store.set_run_state(run_id, RunState::Cancelled)?;
        let _ = self.gate.end_run(run_id);
        let run = store.get(run_id)?;
        self.publish(&run);
        Ok(run)
    }

    /// At start-up: nothing survives a restart, so running runs become `interrupted`.
    pub fn mark_interrupted(&self) -> usize {
        let store = RunStore::new(&self.db);
        let active = store.active().unwrap_or_default();
        let n = store.mark_interrupted().unwrap_or(0);
        for r in active {
            let _ = self.gate.end_run(&r.id);
        }
        n
    }
}
