#![cfg(unix)] // Unix mock programs; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! SessionManager (ADR 0023): start, output, exit metadata, terminate, workspace removal.
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::{Outbound, SessionId};
use naurix_core::session::{SessionError, SessionLaunch, SessionManager};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(10);

fn setup() -> (Arc<SessionManager>, Arc<Db>, String, std::path::PathBuf) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let dir = std::env::temp_dir().join(format!(
        "nx-session-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap();
    let mgr = Arc::new(SessionManager::new(db.clone(), Duration::from_millis(900)));
    (mgr, db, ws.id, dir)
}

fn sh(script: &str, kind: SessionKind) -> SessionLaunch {
    SessionLaunch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        kind,
        title: "test".into(),
        adapter_id: matches!(kind, SessionKind::Agent).then(|| "mock".to_string()),
        approval_id: None,
        cwd: None,
    }
}

/// Collect output until the process reports its exit.
fn run_to_exit(rx: &Receiver<Outbound>) -> (String, Option<u32>) {
    let end = Instant::now() + T;
    let mut out = Vec::new();
    while let Ok(o) = rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
        match o {
            Outbound::Data(d) => out.extend(d),
            Outbound::Exited(i) => return (String::from_utf8_lossy(&out).into(), i.code),
        }
    }
    panic!(
        "no exit within timeout; output so far: {}",
        String::from_utf8_lossy(&out)
    );
}

fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + T;
    while Instant::now() < end {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    cond()
}

#[test]
fn start_forwards_output_and_records_metadata() {
    let (mgr, db, ws, dir) = setup();
    let (tx, rx) = channel();
    let id = mgr
        .start(
            &ws,
            sh("echo hello-session; exit 3", SessionKind::Agent),
            24,
            80,
            move |o| tx.send(o).is_ok(),
        )
        .unwrap();
    let (out, code) = run_to_exit(&rx);
    assert!(out.contains("hello-session"), "output was: {out}");
    assert_eq!(code, Some(3));

    // The session row is finished and the entry removed once the forwarder ends.
    assert!(wait_for(|| db.recent_sessions(&ws, 5).unwrap()[0]
        .lifecycle
        == "exited"));
    let rec = &db.recent_sessions(&ws, 5).unwrap()[0];
    assert_eq!(rec.kind, "agent");
    assert_eq!(rec.adapter_id.as_deref(), Some("mock"));
    assert!(rec.ended_at.is_some());
    assert!(wait_for(|| mgr.session_count() == 0));
    // An exited session stays available for replay until it is closed (ADR 0026).
    assert!(mgr.ack(id, 1).is_ok());
    mgr.close(id).unwrap();
    assert!(matches!(mgr.ack(id, 1), Err(SessionError::NotFound)));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn input_and_resize_reach_a_live_session_and_terminate_stops_it() {
    let (mgr, _db, ws, dir) = setup();
    let (tx, rx) = channel();
    let id = mgr
        .start(
            &ws,
            sh("read line; echo got:$line; sleep 30", SessionKind::Terminal),
            24,
            80,
            move |o| tx.send(o).is_ok(),
        )
        .unwrap();
    mgr.resize(id, 30, 100).unwrap();
    mgr.write(id, b"ping\n").unwrap();
    let end = Instant::now() + T;
    let mut seen = String::new();
    while !seen.contains("got:ping") && Instant::now() < end {
        if let Ok(Outbound::Data(d)) = rx.recv_timeout(Duration::from_millis(200)) {
            seen.push_str(&String::from_utf8_lossy(&d));
            mgr.ack(id, d.len()).ok();
        }
    }
    assert!(seen.contains("got:ping"), "output was: {seen}");
    assert_eq!(mgr.session_count(), 1);
    mgr.terminate(id).unwrap();
    assert!(wait_for(|| mgr.session_count() == 0));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn terminate_workspace_stops_only_that_workspaces_sessions() {
    let (mgr, db, ws_a, dir_a) = setup();
    let dir_b = dir_a.with_file_name(format!(
        "{}-b",
        dir_a.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&dir_b).unwrap();
    let ws_b = db.create_workspace(dir_b.to_str().unwrap()).unwrap().id;
    let (txa, _rxa) = channel();
    let (txb, _rxb) = channel();
    mgr.start(
        &ws_a,
        sh("sleep 30", SessionKind::Terminal),
        24,
        80,
        move |o| txa.send(o).is_ok(),
    )
    .unwrap();
    mgr.start(
        &ws_b,
        sh("sleep 30", SessionKind::Terminal),
        24,
        80,
        move |o| txb.send(o).is_ok(),
    )
    .unwrap();
    assert_eq!(mgr.session_count(), 2);

    mgr.terminate_workspace(&ws_a);
    assert!(wait_for(|| mgr.session_count() == 1));
    mgr.shutdown();
    assert!(wait_for(|| mgr.session_count() == 0));
    std::fs::remove_dir_all(dir_a).ok();
    std::fs::remove_dir_all(dir_b).ok();
}

#[test]
fn start_errors_are_clear() {
    let (mgr, _db, ws, dir) = setup();
    let (tx, _rx) = channel::<Outbound>();

    let e = mgr
        .start(
            "no-such-workspace",
            sh("true", SessionKind::Terminal),
            24,
            80,
            |_| true,
        )
        .unwrap_err();
    assert!(matches!(e, SessionError::Db(_)), "{e}");

    // A folder that disappeared after the workspace was created.
    std::fs::remove_dir_all(&dir).unwrap();
    let e = mgr
        .start(&ws, sh("true", SessionKind::Terminal), 24, 80, move |o| {
            tx.send(o).is_ok()
        })
        .unwrap_err();
    assert!(matches!(e, SessionError::MissingFolder(_)));
    assert!(e
        .to_string()
        .starts_with("The workspace folder is missing: "));

    assert_eq!(
        mgr.write(SessionId(999), b"x").unwrap_err().to_string(),
        "session not found"
    );
    assert_eq!(
        mgr.ack(SessionId(999), 1).unwrap_err().to_string(),
        "session not found"
    );
    assert_eq!(mgr.session_count(), 0);
}
