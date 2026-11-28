#![cfg(unix)] // Unix mock programs (ADR 0021).
//! Workflows and runs (ADR 0030): real sessions driven by a fake launcher whose "agents" are shell scripts.
use naurix_core::approval::{Action, ActionClass, ApprovalGate, Decision, Scope, Verdict};
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::SessionId;
use naurix_core::session::{SessionLaunch, SessionManager};
use naurix_core::workflow::{
    Engine, LaunchProblem, Launcher, RunState, RunStore, StartVerdict, StepInput, StepState,
    WorkflowError, WorkflowStore,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(15);

/// Agents: `ok` exits 0, `fail` exits 3, `slow` exits 0 after a second, `hang` runs until closed,
/// `secret` prints a marker and exits 0, `findings` exits 1 and lists 0 and 1 as success (a scanner
/// that reports issues, ADR 0032 §7). Anything else is "not installed".
struct Fake {
    sessions: Arc<SessionManager>,
    /// Changes every action's fingerprint, to simulate a program path that changed after review.
    salt: AtomicU64,
}

impl Fake {
    fn script(agent: &str) -> Option<&'static str> {
        Some(match agent {
            "ok" => "exit 0",
            "fail" => "exit 3",
            "slow" => "sleep 1; exit 0",
            "hang" => "sleep 60",
            "secret" => "echo PLANTED-OUTPUT-MARKER; exit 0",
            "findings" => "exit 1",
            _ => return None,
        })
    }
}

impl Launcher for Fake {
    fn action(&self, ws: &str, agent: &str) -> Result<Action, LaunchProblem> {
        Self::script(agent).ok_or(LaunchProblem::NotFound)?;
        let salt = self.salt.load(Ordering::SeqCst).to_string();
        Ok(Action::new(
            ActionClass::LaunchAgent,
            ws,
            &[agent, &salt],
            format!("{agent} fake"),
            format!("/fake/{agent}"),
            vec![Scope::Once, Scope::Workspace],
        ))
    }

    fn launch(&self, ws: &str, agent: &str, approval: &str) -> Result<SessionId, LaunchProblem> {
        let script = Self::script(agent).ok_or(LaunchProblem::NotFound)?;
        self.sessions
            .start(
                ws,
                SessionLaunch {
                    program: "/bin/sh".into(),
                    args: vec!["-c".into(), script.into()],
                    env: vec![],
                    kind: SessionKind::Agent,
                    title: agent.into(),
                    adapter_id: Some(agent.into()),
                    approval_id: Some(approval.into()),
                    worktree_id: None,
                    cwd: None,
                },
                24,
                80,
                |_| false, // detached: no output consumer
            )
            .map_err(|e| match e {
                naurix_core::session::SessionError::AgentLimit { .. } => LaunchProblem::Busy,
                _ => LaunchProblem::Failed,
            })
    }

    fn success_exit_codes(&self, _ws: &str, agent: &str) -> Vec<u32> {
        if agent == "findings" {
            vec![0, 1]
        } else {
            vec![0]
        }
    }
}

struct Rig {
    db: Arc<Db>,
    gate: Arc<ApprovalGate>,
    sessions: Arc<SessionManager>,
    engine: Arc<Engine>,
    fake: Arc<Fake>,
    ws: String,
    dir: std::path::PathBuf,
}

/// The engine owns its launcher, so share the fake through a thin wrapper.
struct Shared(Arc<Fake>);
impl Launcher for Shared {
    fn action(&self, w: &str, a: &str) -> Result<Action, LaunchProblem> {
        self.0.action(w, a)
    }
    fn launch(&self, w: &str, a: &str, p: &str) -> Result<SessionId, LaunchProblem> {
        self.0.launch(w, a, p)
    }
    fn success_exit_codes(&self, w: &str, a: &str) -> Vec<u32> {
        self.0.success_exit_codes(w, a)
    }
}

fn rig() -> Rig {
    let dir = std::env::temp_dir().join(format!(
        "nx-wf-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Db::open(&dir.join("naurix.db")).unwrap());
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap().id;
    let sessions = Arc::new(SessionManager::new(db.clone(), Duration::from_millis(900)));
    sessions.set_max_agents(8);
    let gate = Arc::new(ApprovalGate::new(db.clone()));
    let fake = Arc::new(Fake {
        sessions: sessions.clone(),
        salt: AtomicU64::new(0),
    });
    let engine = Engine::new(
        db.clone(),
        gate.clone(),
        sessions.clone(),
        Box::new(Shared(fake.clone())),
    );
    engine.spawn_listener();
    Rig {
        db,
        gate,
        sessions,
        engine,
        fake,
        ws,
        dir,
    }
}

fn step(name: &str, agent: &str, deps: &[usize]) -> StepInput {
    StepInput {
        name: name.into(),
        agent_id: agent.into(),
        depends_on: deps.to_vec(),
    }
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + T;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

/// Start a workflow, approving the run once as the user would.
fn run_it(r: &Rig, wf: &str) -> naurix_core::workflow::Run {
    let StartVerdict::Required(req) = r.engine.start(wf).unwrap() else {
        panic!("a run needs its approval first")
    };
    r.gate
        .decide(&req.id, Decision::Allow(Scope::Once))
        .unwrap();
    match r.engine.start(wf).unwrap() {
        StartVerdict::Started(run) => *run,
        StartVerdict::Required(_) => panic!("approved but asked again"),
    }
}

fn states(r: &Rig, run: &str) -> Vec<StepState> {
    r.engine
        .get_run(run)
        .unwrap()
        .steps
        .iter()
        .map(|s| s.state)
        .collect()
}

fn finished(r: &Rig, run: &str) -> bool {
    r.engine.get_run(run).unwrap().state != RunState::Running
}

#[test]
fn a_diamond_runs_in_dependency_order_and_ends_its_grants() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Diamond",
            &[
                step("a", "ok", &[]),
                step("b", "ok", &[0]),
                step("c", "ok", &[0]),
                step("d", "ok", &[1, 2]),
            ],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(wait_until(|| finished(&r, &run.id)));
    let run = r.engine.get_run(&run.id).unwrap();
    assert_eq!(run.state, RunState::Succeeded);
    assert!(run
        .steps
        .iter()
        .all(|s| s.state == StepState::Succeeded && s.exit_code == Some(0)));
    let by = |n: &str| run.steps.iter().find(|s| s.name == n).unwrap();
    let (a, b, c, d) = (by("a"), by("b"), by("c"), by("d"));
    assert!(b.started_at >= a.finished_at && c.started_at >= a.finished_at);
    assert!(d.started_at >= b.finished_at && d.started_at >= c.finished_at);
    // The run's own grants are gone; the once approval was spent.
    assert!(r.gate.list_active().unwrap().is_empty());
    let history: Vec<_> = r
        .gate
        .history(50)
        .unwrap()
        .iter()
        .map(|g| (g.class.clone(), g.scope.clone(), g.state.clone()))
        .collect();
    assert!(history
        .iter()
        .any(|h| h.0 == "run_workflow" && h.2 == "consumed"));
    assert!(history.iter().any(|h| h.1 == "run" && h.2 == "revoked"));
}

#[test]
fn an_exit_code_listed_as_success_completes_the_step_and_others_still_fail() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Scan",
            &[
                step("scan", "findings", &[]),
                step("triage", "ok", &[0]),
                step("other", "fail", &[]),
            ],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(wait_until(|| finished(&r, &run.id)));
    let run = r.engine.get_run(&run.id).unwrap();
    assert_eq!(
        states(&r, &run.id),
        [
            StepState::Succeeded,
            StepState::Succeeded,
            StepState::Failed
        ]
    );
    assert_eq!(run.steps[0].exit_code, Some(1));
    assert_eq!(run.steps[0].note, None);
}

#[test]
fn a_failed_step_blocks_what_depends_on_it_and_fails_the_run() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Fails",
            &[
                step("a", "fail", &[]),
                step("b", "ok", &[0]),
                step("c", "ok", &[]),
            ],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(wait_until(|| finished(&r, &run.id)));
    let run = r.engine.get_run(&run.id).unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(
        states(&r, &run.id),
        [StepState::Failed, StepState::Pending, StepState::Succeeded]
    );
    assert_eq!(run.steps[0].exit_code, Some(3));
    assert_eq!(run.steps[0].note.as_deref(), Some("exited with an error"));
}

#[test]
fn skip_lets_dependents_go_and_mark_done_finishes_a_running_step_without_stopping_it() {
    let r = rig();
    // a (hangs) -> b -> c, plus d after a.
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Manual",
            &[
                step("a", "hang", &[]),
                step("b", "ok", &[0]),
                step("c", "ok", &[1]),
                step("d", "ok", &[0]),
            ],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert_eq!(
        states(&r, &run.id),
        [
            StepState::Running,
            StepState::Pending,
            StepState::Pending,
            StepState::Pending
        ]
    );
    let (a, b) = (run.steps[0].id.clone(), run.steps[1].id.clone());
    // A running step cannot be skipped, a pending one cannot be marked done.
    assert!(r.engine.skip(&run.id, &a).is_err());
    assert!(r.engine.mark_done(&run.id, &b).is_err());
    // Skipping b releases c while a still runs.
    r.engine.skip(&run.id, &b).unwrap();
    assert!(wait_until(|| states(&r, &run.id)[2] == StepState::Succeeded));
    // Marking a done starts d; a's process is untouched.
    let sid = r.engine.session_of(&a).expect("a has a live session");
    r.engine.mark_done(&run.id, &a).unwrap();
    assert!(r.sessions.list().iter().any(|s| s.id == sid && s.running()));
    assert!(wait_until(|| finished(&r, &run.id)));
    assert_eq!(
        r.engine.get_run(&run.id).unwrap().state,
        RunState::Succeeded
    );
    r.sessions.close(sid).unwrap();
}

#[test]
fn the_agent_limit_delays_ready_steps_and_never_exceeds_it() {
    let r = rig();
    r.sessions.set_max_agents(1);
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Limited",
            &[
                step("a", "slow", &[]),
                step("b", "slow", &[]),
                step("c", "slow", &[]),
            ],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    let mut peak = 0;
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end && !finished(&r, &run.id) {
        let running = states(&r, &run.id)
            .iter()
            .filter(|s| **s == StepState::Running)
            .count();
        peak = peak.max(running);
        assert!(r.sessions.running_agents() <= 1, "never over the limit");
        std::thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(peak, 1);
    assert_eq!(
        r.engine.get_run(&run.id).unwrap().state,
        RunState::Succeeded
    );
}

#[test]
fn the_run_approval_is_once_and_a_second_run_of_the_same_workflow_asks_again() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(&r.ws, None, "Once", &[step("a", "ok", &[])])
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(wait_until(|| finished(&r, &run.id)));
    assert!(
        matches!(r.engine.start(&wf.id).unwrap(), StartVerdict::Required(_)),
        "once is spent"
    );
}

#[test]
fn an_unapproved_run_starts_nothing_and_the_request_shows_every_command() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Review",
            &[step("build", "ok", &[]), step("ship", "slow", &[0])],
        )
        .unwrap();
    let StartVerdict::Required(req) = r.engine.start(&wf.id).unwrap() else {
        panic!()
    };
    assert_eq!(
        req.allowed_scopes,
        [Scope::Once],
        "a run can only be approved once"
    );
    assert!(req.display.contains("Step 1: build") && req.display.contains("/fake/ok"));
    assert!(
        req.display.contains("Step 2: ship (after step 1)") && req.display.contains("/fake/slow")
    );
    assert!(
        RunStore::new(&r.db).list(&r.ws, 10).unwrap().is_empty(),
        "nothing was created"
    );
    assert_eq!(r.sessions.running_agents(), 0);
    // A scope the action does not allow is refused.
    assert!(r
        .gate
        .decide(&req.id, Decision::Allow(Scope::Workspace))
        .is_err());
    assert!(r.gate.decide(&req.id, Decision::Allow(Scope::Run)).is_err());
    r.gate.decide(&req.id, Decision::Deny).unwrap();
    assert!(matches!(
        r.engine.start(&wf.id).unwrap(),
        StartVerdict::Required(_)
    ));
}

#[test]
fn a_changed_agent_after_review_asks_again_and_a_step_outside_the_review_fails_without_asking() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Drift",
            &[step("a", "slow", &[]), step("b", "ok", &[0])],
        )
        .unwrap();
    // Reviewed with salt 0, approved.
    let StartVerdict::Required(req) = r.engine.start(&wf.id).unwrap() else {
        panic!()
    };
    r.gate
        .decide(&req.id, Decision::Allow(Scope::Once))
        .unwrap();
    // The agent's program changed between review and start: the approval no longer matches.
    r.fake.salt.store(1, Ordering::SeqCst);
    assert!(matches!(
        r.engine.start(&wf.id).unwrap(),
        StartVerdict::Required(_)
    ));
    r.fake.salt.store(0, Ordering::SeqCst);
    let StartVerdict::Started(run) = r.engine.start(&wf.id).unwrap() else {
        panic!()
    };
    // While a runs, the program of b changes: its launch is not covered by the run's grants.
    r.fake.salt.store(2, Ordering::SeqCst);
    assert!(wait_until(|| finished(&r, &run.id)));
    let run = r.engine.get_run(&run.id).unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(run.steps[1].state, StepState::Failed);
    assert_eq!(run.steps[1].note.as_deref(), Some("approval missing"));
    assert_eq!(
        r.gate.list_active().unwrap().len(),
        0,
        "no request was opened mid-run and no grant remains"
    );
}

#[test]
fn run_grants_are_listed_revocable_and_only_usable_by_their_run() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Grants",
            &[step("a", "hang", &[]), step("b", "ok", &[0])],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    let grants = r.gate.list_active().unwrap();
    assert_eq!(grants.len(), 2, "one run grant per distinct launch");
    assert!(grants.iter().all(|g| g.scope == "run"));
    // An ordinary launch of the same agent is not covered by a run grant.
    let a = r.fake.action(&r.ws, "hang").unwrap();
    assert!(matches!(r.gate.preview(&a).unwrap(), Verdict::Required(_)));
    assert!(r
        .gate
        .authorize_in_run(&a, "some-other-run")
        .unwrap()
        .is_none());
    // Revoking mid-run stops the next step from starting.
    for g in &grants {
        r.gate.revoke(&g.id).unwrap();
    }
    let a_id = run.steps[0].id.clone();
    r.engine.mark_done(&run.id, &a_id).unwrap();
    let run2 = r.engine.get_run(&run.id).unwrap();
    assert_eq!(run2.steps[1].note.as_deref(), Some("approval missing"));
    assert_eq!(run2.state, RunState::Failed);
    r.sessions
        .close(r.engine.session_of(&a_id).unwrap_or(SessionId(u64::MAX)))
        .ok();
}

#[test]
fn cancel_stops_tracking_leaves_sessions_alone_and_ends_the_grants() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Cancel",
            &[step("a", "hang", &[]), step("b", "ok", &[0])],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    let sid = r.engine.session_of(&run.steps[0].id).unwrap();
    let run = r.engine.cancel(&run.id).unwrap();
    assert_eq!(run.state, RunState::Cancelled);
    assert_eq!(
        states(&r, &run.id),
        [StepState::Cancelled, StepState::Cancelled]
    );
    assert!(
        r.sessions.list().iter().any(|s| s.id == sid && s.running()),
        "the agent keeps running"
    );
    assert!(r.gate.list_active().unwrap().is_empty());
    r.sessions.close(sid).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        r.engine.get_run(&run.id).unwrap().state,
        RunState::Cancelled,
        "a late exit changes nothing"
    );
}

#[test]
fn a_missing_agent_cannot_be_run_and_is_named_in_the_refusal() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Missing",
            &[step("a", "ok", &[]), step("b", "not-installed", &[0])],
        )
        .unwrap();
    let e = r.engine.start(&wf.id).unwrap_err().to_string();
    assert!(
        e.contains("Step 2 (b)") && e.contains("not installed"),
        "{e}"
    );
}

#[test]
fn a_workflow_with_an_active_run_cannot_be_edited_or_deleted_and_history_outlives_it() {
    let r = rig();
    let store = WorkflowStore::new(&r.db);
    let wf = store
        .save(&r.ws, None, "Busy", &[step("a", "hang", &[])])
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(matches!(
        store.save(&r.ws, Some(&wf.id), "Busy", &[step("a", "ok", &[])]),
        Err(WorkflowError::Active)
    ));
    assert!(matches!(store.delete(&wf.id), Err(WorkflowError::Active)));
    assert!(matches!(r.engine.start(&wf.id), Err(WorkflowError::Active)));
    let sid = r.engine.session_of(&run.steps[0].id).unwrap();
    r.engine.cancel(&run.id).unwrap();
    r.sessions.close(sid).unwrap();
    store
        .save(
            &r.ws,
            Some(&wf.id),
            "Busy v2",
            &[step("changed", "ok", &[])],
        )
        .unwrap();
    let old = r.engine.get_run(&run.id).unwrap();
    assert_eq!(
        (old.name.as_str(), old.steps[0].name.as_str()),
        ("Busy", "a"),
        "a run is a snapshot"
    );
    store.delete(&wf.id).unwrap();
    let kept = r.engine.get_run(&run.id).unwrap();
    assert_eq!(kept.workflow_id, None);
    assert_eq!(kept.name, "Busy");
}

#[test]
fn saving_round_trips_replaces_steps_and_stays_inside_its_workspace() {
    let r = rig();
    let other =
        r.db.create_workspace(r.dir.join("..").to_str().unwrap())
            .ok();
    let store = WorkflowStore::new(&r.db);
    let wf = store
        .save(
            &r.ws,
            None,
            "  Flow ",
            &[step("a", "ok", &[]), step("b", "ok", &[0])],
        )
        .unwrap();
    assert_eq!(wf.name, "Flow");
    assert_eq!(wf.steps[1].depends_on, [wf.steps[0].id.clone()]);
    let again = store
        .save(&r.ws, Some(&wf.id), "Flow", &[step("only", "ok", &[])])
        .unwrap();
    assert_eq!(again.steps.len(), 1);
    assert_eq!(store.list(&r.ws).unwrap().len(), 1);
    assert!(
        store
            .save(
                &r.ws,
                None,
                "bad",
                &[step("a", "ok", &[1]), step("b", "ok", &[0])]
            )
            .is_err(),
        "a cycle is refused"
    );
    if let Some(o) = other {
        assert!(matches!(
            store.save(&o.id, Some(&wf.id), "x", &[step("a", "ok", &[])]),
            Err(WorkflowError::NotFound)
        ));
        assert!(store.list(&o.id).unwrap().is_empty());
    }
}

#[test]
fn a_restart_marks_running_runs_interrupted_and_ends_their_grants() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(
            &r.ws,
            None,
            "Restart",
            &[step("a", "hang", &[]), step("b", "ok", &[0])],
        )
        .unwrap();
    let run = run_it(&r, &wf.id);
    let sid = r.engine.session_of(&run.steps[0].id).unwrap();
    // A new process: a fresh engine over the same database.
    let sessions2 = Arc::new(SessionManager::new(
        r.db.clone(),
        Duration::from_millis(900),
    ));
    let fake2 = Arc::new(Fake {
        sessions: sessions2.clone(),
        salt: AtomicU64::new(0),
    });
    let engine2 = Engine::new(
        r.db.clone(),
        r.gate.clone(),
        sessions2,
        Box::new(Shared(fake2)),
    );
    assert_eq!(engine2.mark_interrupted(), 1);
    let run = engine2.get_run(&run.id).unwrap();
    assert_eq!(run.state, RunState::Interrupted);
    assert_eq!(
        (run.steps[0].state, run.steps[0].note.as_deref()),
        (StepState::Failed, Some("interrupted"))
    );
    assert_eq!(run.steps[1].state, StepState::Pending);
    assert!(r.gate.list_active().unwrap().is_empty());
    r.sessions.close(sid).unwrap();
}

#[test]
fn nothing_an_agent_prints_or_the_run_does_reaches_the_database() {
    let r = rig();
    let wf = WorkflowStore::new(&r.db)
        .save(&r.ws, None, "Quiet", &[step("a", "secret", &[])])
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert!(wait_until(|| finished(&r, &run.id)));
    assert_eq!(
        r.engine.get_run(&run.id).unwrap().state,
        RunState::Succeeded
    );
    for f in ["naurix.db", "naurix.db-wal"] {
        let bytes = std::fs::read(r.dir.join(f)).unwrap_or_default();
        assert!(
            !bytes.windows(21).any(|w| w == b"PLANTED-OUTPUT-MARKER"),
            "{f} holds agent output"
        );
    }
}

#[test]
fn a_waiting_step_starts_when_a_manual_agent_frees_the_slot() {
    let r = rig();
    r.sessions.set_max_agents(1);
    // A manual (non-workflow) agent holds the only slot.
    let manual = r.fake.launch(&r.ws, "hang", "x").unwrap();
    let wf = WorkflowStore::new(&r.db)
        .save(&r.ws, None, "Waits", &[step("a", "ok", &[])])
        .unwrap();
    let run = run_it(&r, &wf.id);
    assert_eq!(
        states(&r, &run.id),
        [StepState::Pending],
        "no free slot: the step waits"
    );
    r.sessions.close(manual).unwrap();
    assert!(
        wait_until(|| finished(&r, &run.id)),
        "the step must start once the slot is free"
    );
    assert_eq!(
        r.engine.get_run(&run.id).unwrap().state,
        RunState::Succeeded
    );
}
