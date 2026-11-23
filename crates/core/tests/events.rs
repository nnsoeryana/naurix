#![cfg(unix)] // Unix mock programs; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Session domain events (ADR 0024): a real session publishes started, then exited.
use naurix_core::events::Event;
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::Outbound;
use naurix_core::session::{SessionLaunch, SessionManager};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

const T: Duration = Duration::from_secs(10);

fn launch(script: &str, kind: SessionKind, adapter: Option<&str>) -> SessionLaunch {
    SessionLaunch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        kind,
        title: "ev".into(),
        adapter_id: adapter.map(str::to_string),
        approval_id: None,
        worktree_id: None,
        cwd: None,
    }
}

fn setup() -> (Arc<SessionManager>, String, std::path::PathBuf) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let dir = std::env::temp_dir().join(format!(
        "nx-events-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap();
    (
        Arc::new(SessionManager::new(db, Duration::from_millis(900))),
        ws.id,
        dir,
    )
}

#[test]
fn a_session_publishes_started_then_exited_with_its_exit_code() {
    let (mgr, ws, dir) = setup();
    let events = mgr.events().subscribe();
    let (tx, rx) = channel();
    let id = mgr
        .start(
            &ws,
            launch("echo hi; exit 7", SessionKind::Agent, Some("mock")),
            24,
            80,
            move |o| tx.send(o).is_ok(),
        )
        .unwrap();
    // Drain output so the forwarder can finish.
    while let Ok(o) = rx.recv_timeout(T) {
        if matches!(o, Outbound::Exited(_)) {
            break;
        }
    }

    assert_eq!(
        events.recv_timeout(T).unwrap(),
        Event::SessionStarted {
            session: id,
            workspace_id: ws.clone(),
            kind: SessionKind::Agent,
            adapter_id: Some("mock".into()),
        }
    );
    assert_eq!(
        events.recv_timeout(T).unwrap(),
        Event::SessionExited {
            session: id,
            workspace_id: ws,
            code: Some(7),
            signal: None,
        }
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_failed_start_publishes_nothing() {
    let (mgr, ws, dir) = setup();
    let events = mgr.events().subscribe();
    let mut bad = launch("true", SessionKind::Terminal, None);
    bad.program = "/definitely/not/here".into();
    assert!(mgr.start(&ws, bad, 24, 80, |_| true).is_err());
    assert!(mgr
        .start(
            "no-such-workspace",
            launch("true", SessionKind::Terminal, None),
            24,
            80,
            |_| true
        )
        .is_err());
    assert!(events.try_recv().is_err());
    std::fs::remove_dir_all(dir).ok();
}
