#![cfg(unix)] // Unix mock programs; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! The agent concurrency limit (ADR 0027): only agents count, the limit is atomic, and lowering it
//! never stops what is already running.
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::session::{SessionError, SessionLaunch, SessionManager};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn setup(limit: usize) -> (Arc<SessionManager>, Arc<Db>, String, std::path::PathBuf) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let dir = std::env::temp_dir().join(format!(
        "nx-limit-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap();
    let mgr = Arc::new(SessionManager::new(db.clone(), Duration::from_millis(900)));
    mgr.set_max_agents(limit);
    (mgr, db, ws.id, dir)
}

fn launch(kind: SessionKind, script: &str) -> SessionLaunch {
    SessionLaunch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        kind,
        title: "t".into(),
        adapter_id: matches!(kind, SessionKind::Agent).then(|| "mock".to_string()),
        approval_id: None,
        worktree_id: None,
        cwd: None,
    }
}

fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    cond()
}

fn agent(m: &Arc<SessionManager>, ws: &str) -> Result<naurix_core::pty::SessionId, SessionError> {
    m.start(ws, launch(SessionKind::Agent, "sleep 30"), 24, 80, |_| true)
}

#[test]
fn an_agent_over_the_limit_fails_with_a_clear_message_and_starts_nothing() {
    let (m, db, ws, dir) = setup(2);
    agent(&m, &ws).unwrap();
    agent(&m, &ws).unwrap();
    let rows_before = db.recent_sessions(&ws, 50).unwrap().len();
    let e = agent(&m, &ws).unwrap_err();
    assert!(
        matches!(
            e,
            SessionError::AgentLimit {
                running: 2,
                limit: 2
            }
        ),
        "{e}"
    );
    assert!(
        e.to_string()
            .starts_with("2 agents are already running (the limit is 2)"),
        "{e}"
    );
    assert!(e.to_string().contains("Settings, AI Agents"), "{e}");
    assert_eq!(m.session_count(), 2, "no process was started");
    assert_eq!(
        db.recent_sessions(&ws, 50).unwrap().len(),
        rows_before,
        "and no session row was written"
    );
    m.shutdown();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn plain_terminals_are_never_limited() {
    let (m, _db, ws, dir) = setup(1);
    agent(&m, &ws).unwrap();
    for _ in 0..4 {
        m.start(
            &ws,
            launch(SessionKind::Terminal, "sleep 30"),
            24,
            80,
            |_| true,
        )
        .unwrap();
    }
    assert_eq!(m.running_agents(), 1);
    assert_eq!(m.session_count(), 5);
    m.shutdown();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_slot_frees_when_an_agent_exits_or_is_closed() {
    let (m, _db, ws, dir) = setup(1);
    let first = agent(&m, &ws).unwrap();
    assert!(agent(&m, &ws).is_err());
    m.terminate(first).unwrap();
    assert!(
        wait_for(|| agent(&m, &ws).is_ok()),
        "an exited agent no longer counts"
    );
    m.shutdown();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn lowering_the_limit_refuses_new_agents_but_never_stops_running_ones() {
    let (m, _db, ws, dir) = setup(3);
    for _ in 0..3 {
        agent(&m, &ws).unwrap();
    }
    m.set_max_agents(1);
    assert_eq!(m.running_agents(), 3, "nothing was stopped");
    assert!(matches!(
        agent(&m, &ws),
        Err(SessionError::AgentLimit {
            running: 3,
            limit: 1
        })
    ));
    assert!(m.ensure_agent_slot().is_err());
    m.shutdown();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn concurrent_launches_cannot_take_more_slots_than_exist() {
    let (m, _db, ws, dir) = setup(3);
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (m, ws) = (m.clone(), ws.clone());
            std::thread::spawn(move || agent(&m, &ws).is_ok())
        })
        .collect();
    let started = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(started, 3, "exactly the free slots are used, never more");
    assert_eq!(m.running_agents(), 3);
    m.shutdown();
    std::fs::remove_dir_all(dir).ok();
}
