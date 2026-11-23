#![cfg(unix)] // Unix mock programs; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Sessions outlive their consumer (ADR 0026): detach, reattach with replay, draining while
//! detached, exited sessions, close, retention and start-time titles.
use naurix_core::events::Event;
use naurix_core::persistence::{Db, SessionKind};
use naurix_core::pty::{Outbound, SessionId};
use naurix_core::session::{SessionError, SessionLaunch, SessionManager, MAX_EXITED_RETAINED};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(15);

fn setup(limit: usize) -> (Arc<SessionManager>, Arc<Db>, String, std::path::PathBuf) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let dir = std::env::temp_dir().join(format!(
        "nx-reattach-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap();
    let mgr = Arc::new(
        SessionManager::new(db.clone(), Duration::from_millis(900)).with_replay_limit(limit),
    );
    (mgr, db, ws.id, dir)
}

fn sh(script: &str) -> SessionLaunch {
    SessionLaunch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        kind: SessionKind::Terminal,
        title: "t".into(),
        adapter_id: None,
        approval_id: None,
        cwd: None,
    }
}

/// A consumer that forwards everything to a channel (and acknowledges like the real UI would).
fn consumer() -> (
    impl FnMut(Outbound) -> bool + Send + 'static,
    Receiver<Outbound>,
) {
    let (tx, rx) = channel();
    (move |o| tx.send(o).is_ok(), rx)
}

/// Collect output (acknowledging it) until `done(text)` holds or the process exits.
fn read_until(
    mgr: &SessionManager,
    id: SessionId,
    rx: &Receiver<Outbound>,
    done: impl Fn(&str) -> bool,
) -> (Vec<u8>, bool) {
    let end = Instant::now() + T;
    let mut bytes = Vec::new();
    while Instant::now() < end {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Outbound::Data(d)) => {
                mgr.ack(id, d.len()).ok();
                bytes.extend(d);
                if done(&String::from_utf8_lossy(&bytes)) {
                    return (bytes, false);
                }
            }
            Ok(Outbound::Exited(_)) => return (bytes, true),
            Err(_) => {}
        }
    }
    (bytes, false)
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
fn a_session_keeps_running_while_detached_and_reattach_replays_what_it_missed() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    let (sink, rx) = consumer();
    let id = mgr
        .start(
            &ws,
            sh("echo first; read x; echo second; sleep 30"),
            24,
            80,
            sink,
        )
        .unwrap();
    read_until(&mgr, id, &rx, |t| t.contains("first"));

    mgr.detach(id).unwrap();
    mgr.write(id, b"go\n").unwrap();
    // `second` is produced while nobody is attached.
    assert!(wait_for(|| {
        let (s2, r2) = consumer();
        mgr.attach(id, s2).unwrap();
        let (b, _) = read_until(&mgr, id, &r2, |t| t.contains("second"));
        let hit = String::from_utf8_lossy(&b).contains("second");
        if !hit {
            mgr.detach(id).unwrap();
        }
        hit
    }));
    assert_eq!(mgr.session_count(), 1, "the process never stopped");
    mgr.close(id).unwrap();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn reattach_replays_exactly_what_was_sent_with_no_gap_and_no_duplicate() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    let (sink, rx) = consumer();
    let id = mgr
        .start(
            &ws,
            sh("for i in $(seq 1 2000); do echo line$i; done; sleep 30"),
            24,
            80,
            sink,
        )
        .unwrap();
    let (first, _) = read_until(&mgr, id, &rx, |t| t.contains("line2000"));
    assert!(String::from_utf8_lossy(&first).contains("line2000"));

    mgr.detach(id).unwrap();
    let (s2, r2) = consumer();
    mgr.attach(id, s2).unwrap();
    let (second, _) = read_until(&mgr, id, &r2, |t| t.contains("line2000"));
    assert_eq!(
        first, second,
        "replay must equal the original stream byte for byte"
    );
    mgr.close(id).unwrap();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_flooding_detached_session_is_not_blocked_and_keeps_only_the_tail() {
    let limit = 4096;
    let (mgr, _db, ws, dir) = setup(limit);
    let events = mgr.events().subscribe();
    let (sink, _rx) = consumer();
    let id = mgr
        .start(
            &ws,
            sh("yes abcdefgh | head -c 5000000; echo DONE"),
            24,
            80,
            sink,
        )
        .unwrap();
    mgr.detach(id).unwrap();

    // It finishes by itself even though nobody reads: the PTY is drained into the ring.
    let exited = loop {
        match events.recv_timeout(T).expect("session did not exit") {
            Event::SessionExited { session, code, .. } if session == id => break code,
            _ => {}
        }
    };
    assert_eq!(exited, Some(0));

    let (s2, r2) = consumer();
    mgr.attach(id, s2).unwrap();
    let mut data = Vec::new();
    let mut saw_exit = false;
    while let Ok(o) = r2.recv_timeout(Duration::from_secs(2)) {
        match o {
            Outbound::Data(d) => data.extend(d),
            Outbound::Exited(i) => {
                saw_exit = true;
                assert_eq!(i.code, Some(0));
                break;
            }
        }
    }
    assert!(
        saw_exit,
        "an exited session replays its output, then its exit"
    );
    assert!(data.len() <= limit, "ring is bounded: {} bytes", data.len());
    assert!(String::from_utf8_lossy(&data).trim_end().ends_with("DONE"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn list_reports_metadata_and_exit_state_and_close_forgets_the_session() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    let (s1, _r1) = consumer();
    let long = mgr.start(&ws, sh("sleep 30"), 24, 80, s1).unwrap();
    let (s2, _r2) = consumer();
    let mut quick = sh("exit 4");
    quick.kind = SessionKind::Agent;
    quick.adapter_id = Some("mock".into());
    quick.title = "agent one".into();
    let short = mgr.start(&ws, quick, 24, 80, s2).unwrap();

    assert!(wait_for(|| mgr
        .list()
        .iter()
        .any(|i| i.id == short && !i.running())));
    let list = mgr.list();
    assert_eq!(list.len(), 2);
    let l = list.iter().find(|i| i.id == long).unwrap();
    assert!(l.running() && l.workspace_id == ws && l.kind == SessionKind::Terminal);
    let s = list.iter().find(|i| i.id == short).unwrap();
    assert_eq!(s.exit.as_ref().unwrap().code, Some(4));
    assert_eq!(
        (s.title.as_str(), s.adapter_id.as_deref()),
        ("agent one", Some("mock"))
    );
    assert_eq!(s.kind, SessionKind::Agent);

    mgr.close(long).unwrap();
    assert!(
        wait_for(|| mgr.session_count() == 0),
        "close stops the process"
    );
    assert!(mgr.list().iter().all(|i| i.id != long));
    assert!(matches!(
        mgr.attach(long, |_| true),
        Err(SessionError::NotFound)
    ));
    assert!(matches!(mgr.close(long), Err(SessionError::NotFound)));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn exited_sessions_that_are_never_closed_are_bounded() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    for _ in 0..(MAX_EXITED_RETAINED + 6) {
        let (s, _r) = consumer();
        mgr.start(&ws, sh("echo x"), 24, 80, s).unwrap();
    }
    assert!(wait_for(|| mgr.session_count() == 0));
    assert!(wait_for(|| mgr.list().len() <= MAX_EXITED_RETAINED));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_consumer_that_goes_away_does_not_end_the_session_or_its_metadata() {
    let (mgr, db, ws, dir) = setup(1 << 20);
    let (tx, rx) = channel::<Outbound>();
    let id = mgr
        .start(
            &ws,
            sh("echo before; read x; echo after; exit 0"),
            24,
            80,
            move |o| tx.send(o).is_ok(),
        )
        .unwrap();
    // The webview disappears (receiver dropped) while the process is still alive.
    drop(rx);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(mgr.session_count(), 1);
    assert_eq!(db.recent_sessions(&ws, 5).unwrap()[0].lifecycle, "running");

    mgr.write(id, b"go\n").unwrap();
    assert!(wait_for(|| db.recent_sessions(&ws, 5).unwrap()[0]
        .lifecycle
        == "exited"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn an_unusable_title_does_not_stop_a_session_from_starting() {
    let (mgr, db, ws, dir) = setup(1 << 20);
    let mut l = sh("true");
    l.title = format!("\x1b[31m{}", "p".repeat(300));
    let (s, _r) = consumer();
    mgr.start(&ws, l, 24, 80, s).unwrap();
    let t = &db.recent_sessions(&ws, 5).unwrap()[0].title;
    assert!(
        t.chars().count() <= 100 && !t.chars().any(char::is_control),
        "{t}"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_stale_detach_cannot_drop_a_newer_consumer() {
    // StrictMode-style remount: consumer 1 is replaced by consumer 2, then consumer 1 "cleans up".
    let (mgr, _db, ws, dir) = setup(1 << 20);
    let (s0, _r0) = consumer();
    let id = mgr
        .start(&ws, sh("read x; echo after-$x; sleep 30"), 24, 80, s0)
        .unwrap();
    let (s1, _r1) = consumer();
    let t1 = mgr.attach(id, s1).unwrap();
    let (s2, r2) = consumer();
    let t2 = mgr.attach(id, s2).unwrap();
    assert!(t2 > t1);

    assert!(
        !mgr.detach_if(id, t1).unwrap(),
        "stale generation is ignored"
    );
    mgr.write(id, b"ok\n").unwrap();
    let (b, _) = read_until(&mgr, id, &r2, |t| t.contains("after-ok"));
    assert!(
        String::from_utf8_lossy(&b).contains("after-ok"),
        "newer consumer still receives"
    );

    assert!(mgr.detach_if(id, t2).unwrap());
    // The consumer given to `start` is generation 0 only until the first attach.
    mgr.close(id).unwrap();
    assert!(matches!(mgr.detach_if(id, 0), Err(SessionError::NotFound)));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn detach_all_keeps_processes_running_and_reattach_still_works() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    let (s, r) = consumer();
    let id = mgr.start(&ws, sh("echo hi; sleep 30"), 24, 80, s).unwrap();
    read_until(&mgr, id, &r, |t| t.contains("hi"));
    mgr.detach_all();
    assert_eq!(mgr.session_count(), 1);
    let (s2, r2) = consumer();
    mgr.attach(id, s2).unwrap();
    let (b, _) = read_until(&mgr, id, &r2, |t| t.contains("hi"));
    assert!(String::from_utf8_lossy(&b).contains("hi"));
    mgr.close(id).unwrap();
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn busy_count_counts_agents_but_not_idle_shells_or_exited_sessions() {
    let (mgr, _db, ws, dir) = setup(1 << 20);
    assert_eq!(mgr.busy_count(), 0);

    // An idle interactive shell is not "busy".
    let (s1, _r1) = consumer();
    let mut shell = sh("");
    shell.args = vec![];
    shell.program = "/bin/sh".into();
    let shell_id = mgr.start(&ws, shell, 24, 80, s1).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(mgr.busy_count(), 0, "idle shell");

    // A running agent is.
    let (s2, _r2) = consumer();
    let mut agent = sh("sleep 30");
    agent.kind = SessionKind::Agent;
    agent.adapter_id = Some("mock".into());
    let agent_id = mgr.start(&ws, agent, 24, 80, s2).unwrap();
    assert_eq!(mgr.busy_count(), 1, "running agent");

    // Once it has exited it is not.
    mgr.terminate(agent_id).unwrap();
    assert!(wait_for(|| mgr.busy_count() == 0), "exited agent");
    mgr.close(shell_id).unwrap();
    mgr.close(agent_id).unwrap();
    std::fs::remove_dir_all(dir).ok();
}
