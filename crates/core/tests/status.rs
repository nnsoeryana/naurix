#![cfg(unix)] // Unix mock programs and a shell `printf` hook (ADR 0021).
//! Status signals (ADR 0029): a fake agent writes the fixed words a hook would write, and the
//! session reports exactly those, publishes events, and forgets them when the process ends.
use naurix_core::agents::adapter_for;
use naurix_core::events::Event;
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::Outbound;
use naurix_core::session::{SessionLaunch, SessionManager};
use naurix_core::signals::{self, SignalKind};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(10);

fn setup() -> (Arc<SessionManager>, String, std::path::PathBuf) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let dir = std::env::temp_dir().join(format!(
        "nx-status-{}-{}",
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

fn agent(script: &str) -> SessionLaunch {
    SessionLaunch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        kind: SessionKind::Agent,
        title: "fake".into(),
        adapter_id: Some("claude-code".into()),
        approval_id: None,
        worktree_id: None,
        cwd: None,
    }
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + T;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn reported_words_become_status_and_events_and_end_with_the_process() {
    let (mgr, ws, dir) = setup();
    let spool = signals::create_spool(&dir).unwrap();
    let events = mgr.events().subscribe();
    let (tx, rx) = channel();
    let p = spool.display();
    // Waits for a marker file between words so the test controls the pace.
    let go = dir.join("go");
    let script = format!(
        "printf 'working\\n' >> '{p}'; printf 'garbage that is not a word\\n' >> '{p}'; \
         printf 'awaiting_approval\\n' >> '{p}'; while [ ! -e '{}' ]; do sleep 0.05; done; \
         printf 'turn_done\\n' >> '{p}'; sleep 30",
        go.display()
    );
    let id = mgr
        .start(&ws, agent(&script), 24, 80, move |o| tx.send(o).is_ok())
        .unwrap();
    mgr.watch_status(id, spool.clone()).unwrap();

    let status = |m: &Arc<SessionManager>| {
        m.list()
            .into_iter()
            .find(|s| s.id == id)
            .and_then(|s| s.status)
            .map(|s| s.kind)
    };
    assert!(wait_until(
        || status(&mgr) == Some(SignalKind::AwaitingApproval)
    ));
    std::fs::write(&go, "").unwrap();
    assert!(wait_until(|| status(&mgr) == Some(SignalKind::TurnDone)));

    let seen: Vec<SignalKind> = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|e| match e {
            Event::SessionStatus { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        seen,
        [
            SignalKind::Working,
            SignalKind::AwaitingApproval,
            SignalKind::TurnDone
        ],
        "the garbage line produced nothing"
    );

    // Ending the process clears the status and removes the spool file.
    mgr.close(id).unwrap();
    assert!(wait_until(|| !spool.exists()), "spool removed after exit");
    while let Ok(o) = rx.recv_timeout(Duration::from_millis(200)) {
        if matches!(o, Outbound::Exited(_)) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_session_with_no_signals_has_no_status() {
    let (mgr, ws, dir) = setup();
    let (tx, _rx) = channel();
    let id = mgr
        .start(&ws, agent("sleep 5"), 24, 80, move |o| tx.send(o).is_ok())
        .unwrap();
    assert_eq!(mgr.list().iter().find(|s| s.id == id).unwrap().status, None);
    mgr.close(id).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_claude_adapter_offers_hooks_that_bind_to_a_spool_and_other_agents_do_not() {
    let args = adapter_for("claude-code").unwrap().status_args().unwrap();
    assert_eq!(args[0], "--settings");
    assert!(args[1].contains(signals::SPOOL_PLACEHOLDER));
    let bound = signals::bind_spool(&args, std::path::Path::new("/d/signals/x")).unwrap();
    assert!(!bound[1].contains(signals::SPOOL_PLACEHOLDER));
    for other in ["codex", "gemini-cli", "antigravity-cli", "opencode"] {
        assert!(
            adapter_for(other).unwrap().status_args().is_none(),
            "{other} has no verified status capability"
        );
    }
}

#[test]
fn the_hook_arguments_are_part_of_the_reviewed_launch_and_off_changes_nothing() {
    use naurix_core::agents::{launch_action, AgentStore};
    use std::os::unix::fs::PermissionsExt;
    let (_, _, dir) = setup();
    let exe = dir.join("claude");
    std::fs::write(&exe, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let db = Db::open_in_memory().unwrap();
    let st = AgentStore::new(&db);
    st.set_path_override("claude-code", Some(exe.to_str().unwrap()))
        .unwrap();
    let dirs = vec![dir.clone()];
    let extra = adapter_for("claude-code").unwrap().status_args().unwrap();

    let (off_def, off) = st.prepare_launch("claude-code", &dirs).unwrap();
    let (on_def, on) = st
        .prepare_launch_with("claude-code", &dirs, &extra)
        .unwrap();
    assert!(
        off.args.is_empty(),
        "off: the launch is exactly what it was"
    );
    assert_eq!(on.args, extra);
    assert!(
        on.display.contains("--settings"),
        "the review shows the hooks"
    );

    let a_off = launch_action(&off_def, &off.program, "w");
    let a_on = launch_action(&on_def, &on.program, "w");
    assert_ne!(
        a_off.fingerprint, a_on.fingerprint,
        "turning it on asks again"
    );
    // The per-session spool path is not in the reviewed text, so a once approval still matches.
    let bound = signals::bind_spool(&on.args, std::path::Path::new("/d/signals/abc")).unwrap();
    assert_ne!(bound, on.args);
    assert_eq!(
        launch_action(&on_def, &on.program, "w").fingerprint,
        a_on.fingerprint
    );
    let _ = std::fs::remove_dir_all(&dir);
}
