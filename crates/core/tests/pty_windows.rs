#![cfg(windows)] // ConPTY smoke tests. The Unix suite is in pty.rs (ADR 0021).
use naurix_core::pty::*;
use std::time::{Duration, Instant};

fn spec(args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: "cmd.exe".into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: std::env::temp_dir(),
        env: vec![],
        rows: 24,
        cols: 80,
    }
}

fn drain(rx: &std::sync::mpsc::Receiver<SessionEvent>) -> (String, Option<ExitInfo>) {
    let end = Instant::now() + Duration::from_secs(20);
    let mut out = Vec::new();
    while let Ok(ev) = rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
        match ev {
            SessionEvent::Output(b) => out.extend(b),
            SessionEvent::Exited(i) => return (String::from_utf8_lossy(&out).into(), Some(i)),
        }
    }
    (String::from_utf8_lossy(&out).into(), None)
}

#[test]
fn runs_a_command_and_reports_its_exit_code() {
    let sup = Supervisor::new();
    let (_id, rx) = sup
        .spawn(&spec(&["/C", "echo hello-conpty & exit 7"]))
        .unwrap();
    let (out, exit) = drain(&rx);
    assert!(out.contains("hello-conpty"), "output was: {out:?}");
    assert_eq!(exit.expect("exit event").code, Some(7));
}

#[test]
fn terminate_ends_an_interactive_shell() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec(&[])).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    sup.terminate(id, Duration::from_secs(3)).unwrap();
    let (_, exit) = drain(&rx);
    assert!(exit.is_some(), "no Exited event after terminate");
}
