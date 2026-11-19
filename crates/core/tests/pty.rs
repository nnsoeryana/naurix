#![cfg(unix)] // Unix mock programs and symlinks; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Integration tests: real processes on real PTYs.
use naurix_core::pty::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

fn spec(program: &str, args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: program.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: "/tmp".into(),
        env: vec![],
        rows: 24,
        cols: 80,
    }
}

/// Read events until `pred(accumulated_output)` holds or the timeout passes.
fn read_until(
    rx: &Receiver<SessionEvent>,
    timeout: Duration,
    pred: impl Fn(&str) -> bool,
) -> (String, Option<ExitInfo>) {
    let end = Instant::now() + timeout;
    let mut out = Vec::new();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(SessionEvent::Output(b)) => {
                out.extend(b);
                if pred(&String::from_utf8_lossy(&out)) {
                    return (String::from_utf8_lossy(&out).into(), None);
                }
            }
            Ok(SessionEvent::Exited(i)) => return (String::from_utf8_lossy(&out).into(), Some(i)),
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                return (String::from_utf8_lossy(&out).into(), None)
            }
        }
    }
}

fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

fn wait_dead(pid: i32, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !alive(pid)
}

const T: Duration = Duration::from_secs(10);

#[test]
fn runs_command_and_reports_exit_code() {
    let sup = Supervisor::new();
    let (_, rx) = sup
        .spawn(&spec("/bin/sh", &["-c", "echo hello-pty; exit 7"]))
        .unwrap();
    let (out, exit) = read_until(&rx, T, |_| false);
    assert!(out.contains("hello-pty"), "output was {out:?}");
    let exit = exit.expect("exit event");
    assert_eq!(exit.code, Some(7));
    assert_eq!(exit.signal, None);
}

#[test]
fn exited_is_last_event_and_session_is_reaped() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec("/bin/sh", &["-c", "echo a"])).unwrap();
    let (_, exit) = read_until(&rx, T, |_| false);
    assert!(exit.is_some());
    assert!(
        rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "nothing after Exited"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(matches!(sup.write(id, b"x"), Err(PtyError::NotFound)));
    assert!(sup.list().is_empty());
}

#[test]
fn interactive_input_roundtrip() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec("/bin/cat", &[])).unwrap();
    sup.write(id, b"ping-123\n").unwrap();
    let (out, _) = read_until(&rx, T, |o| {
        o.matches("ping-123").count() >= 2 || o.contains("ping-123\r\nping-123")
    });
    assert!(out.contains("ping-123"), "output was {out:?}");
    sup.write(id, &[0x04]).unwrap(); // Ctrl-D ends cat
    let (_, exit) = read_until(&rx, T, |_| false);
    assert_eq!(exit.unwrap().code, Some(0));
}

#[test]
fn resize_is_seen_by_the_child() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec("/bin/sh", &[])).unwrap();
    sup.write(id, b"stty size\n").unwrap();
    let (out, _) = read_until(&rx, T, |o| o.contains("24 80"));
    assert!(out.contains("24 80"), "output was {out:?}");
    sup.resize(id, 40, 120).unwrap();
    sup.write(id, b"stty size\n").unwrap();
    let (out, _) = read_until(&rx, T, |o| o.contains("40 120"));
    assert!(out.contains("40 120"), "output was {out:?}");
    sup.terminate(id, Duration::from_secs(3)).unwrap();
}

#[test]
fn starts_in_requested_directory_with_env_additions() {
    let sup = Supervisor::new();
    let mut s = spec("/bin/sh", &["-c", "pwd; echo $AT_TEST_VAR $TERM"]);
    s.cwd = std::fs::canonicalize("/tmp").unwrap();
    s.env = vec![("AT_TEST_VAR".into(), "val42".into())];
    let (_, rx) = sup.spawn(&s).unwrap();
    let (out, _) = read_until(&rx, T, |_| false);
    assert!(out.contains(s.cwd.to_str().unwrap()), "output was {out:?}");
    assert!(out.contains("val42 xterm-256color"), "output was {out:?}");
}

#[test]
fn spawn_errors_are_clear() {
    let sup = Supervisor::new();
    assert!(matches!(
        sup.spawn(&spec("/no/such/binary", &[])),
        Err(PtyError::ExecutableNotFound(_))
    ));
    let mut s = spec("/bin/sh", &[]);
    s.cwd = "/no/such/dir".into();
    assert!(matches!(
        sup.spawn(&s),
        Err(PtyError::BadWorkingDirectory(_))
    ));
    let mut s = spec("/bin/sh", &[]);
    s.rows = 0;
    assert!(matches!(sup.spawn(&s), Err(PtyError::Invalid(_))));
    assert!(sup.list().is_empty());
}

#[test]
fn oversized_input_is_rejected_not_truncated() {
    let sup = Supervisor::new();
    let (id, _rx) = sup.spawn(&spec("/bin/cat", &[])).unwrap();
    let big = vec![b'a'; MAX_INPUT_BYTES + 1];
    assert!(matches!(sup.write(id, &big), Err(PtyError::Invalid(_))));
    sup.terminate(id, Duration::from_secs(3)).unwrap();
}

#[test]
fn terminate_kills_a_stubborn_process() {
    let sup = Supervisor::new();
    // Ignores HUP and TERM; only SIGKILL works.
    let (id, rx) = sup
        .spawn(&spec(
            "/bin/sh",
            &[
                "-c",
                "trap '' HUP TERM; echo ready; while :; do sleep 1; done",
            ],
        ))
        .unwrap();
    read_until(&rx, T, |o| o.contains("ready"));
    let outcome = sup.terminate(id, Duration::from_secs(3)).unwrap();
    assert_eq!(outcome, TerminateOutcome::Forced);
    let (_, exit) = read_until(&rx, T, |_| false);
    assert_eq!(exit.unwrap().signal.as_deref(), Some("Killed: 9"));
}

#[test]
fn terminate_is_graceful_for_a_cooperative_process() {
    let sup = Supervisor::new();
    let (id, rx) = sup
        .spawn(&spec(
            "/bin/sh",
            &["-c", "echo ready; while :; do sleep 1; done"],
        ))
        .unwrap();
    read_until(&rx, T, |o| o.contains("ready"));
    assert_eq!(
        sup.terminate(id, Duration::from_secs(6)).unwrap(),
        TerminateOutcome::Graceful
    );
}

#[test]
fn terminate_cleans_up_background_children() {
    let sup = Supervisor::new();
    let (id, rx) = sup
        .spawn(&spec("/bin/sh", &["-c", "sleep 300 & echo CHILD=$!; wait"]))
        .unwrap();
    let (out, _) = read_until(&rx, T, |o| o.contains("CHILD=") && o.contains('\n'));
    let child: i32 = out
        .split("CHILD=")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(child));
    sup.terminate(id, Duration::from_secs(3)).unwrap();
    assert!(
        wait_dead(child, Duration::from_secs(3)),
        "background child {child} survived terminate"
    );
}

/// Interactive shells put jobs in their own process groups; the foreground job must still die.
#[test]
fn terminate_cleans_up_foreground_job_of_interactive_shell() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec("/bin/zsh", &["-f", "-i"])).unwrap();
    read_until(&rx, T, |o| o.contains('%')); // wait for the first prompt
    sup.write(id, b"sleep 301 & printf 'BGPID:%s:END\\n' $!\n")
        .unwrap();
    let (out, _) = read_until(&rx, T, |o| {
        o.split("BGPID:").skip(1).any(|r| {
            r.split(":END")
                .next()
                .is_some_and(|n| n.parse::<i32>().is_ok())
                && r.contains(":END")
        })
    });
    let bg: i32 = out
        .split("BGPID:")
        .skip(1)
        .find_map(|r| r.split(":END").next()?.parse().ok())
        .unwrap_or_else(|| panic!("bg pid not found in {out:?}"));
    sup.write(id, b"sleep 302\n").unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(alive(bg));
    sup.terminate(id, Duration::from_secs(3)).unwrap();
    assert!(
        wait_dead(bg, Duration::from_secs(3)),
        "background job {bg} survived"
    );
    let fg = std::process::Command::new("pgrep")
        .args(["-f", "sleep 302"])
        .output()
        .unwrap();
    assert!(
        fg.stdout.is_empty(),
        "foreground job survived: {}",
        String::from_utf8_lossy(&fg.stdout)
    );
}

/// Backpressure, tested without racing the reader: the child writes 20 MB, the consumer does not read
/// for a while. If output were buffered without bound the child would already have finished; with a
/// bounded queue it is still blocked on write. Draining afterwards must deliver every byte.
#[test]
fn output_is_bounded_by_backpressure_and_nothing_is_dropped() {
    const TOTAL: usize = 20_000_000;
    let sup = Supervisor::new();
    let (id, rx) = sup
        .spawn(&spec(
            "/bin/sh",
            &["-c", &format!("head -c {TOTAL} /dev/zero")],
        ))
        .unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(
        sup.list().contains(&id),
        "child finished while nobody was reading: output is not bounded"
    );

    let mut got = 0usize;
    let end = Instant::now() + Duration::from_secs(30);
    let exit = loop {
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(SessionEvent::Output(b)) => got += b.len(),
            Ok(SessionEvent::Exited(i)) => break i,
            Err(e) => panic!("stream ended early after {got} bytes: {e:?}"),
        }
    };
    assert_eq!(got, TOTAL, "bytes were dropped or duplicated");
    assert_eq!(exit.code, Some(0));
}

#[test]
fn shutdown_all_terminates_every_session() {
    let sup = Supervisor::new();
    let mut pids = vec![];
    let mut rxs = vec![];
    for _ in 0..4 {
        let (_, rx) = sup
            .spawn(&spec(
                "/bin/sh",
                &["-c", "echo P=$$; while :; do sleep 1; done"],
            ))
            .unwrap();
        let (out, _) = read_until(&rx, T, |o| o.contains("P=") && o.contains('\n'));
        pids.push(
            out.split("P=")
                .nth(1)
                .unwrap()
                .lines()
                .next()
                .unwrap()
                .trim()
                .parse::<i32>()
                .unwrap(),
        );
        rxs.push(rx);
    }
    assert_eq!(sup.list().len(), 4);
    sup.shutdown_all(Duration::from_secs(3));
    for p in pids {
        assert!(
            wait_dead(p, Duration::from_secs(3)),
            "pid {p} survived shutdown"
        );
    }
}

#[test]
fn dropping_the_supervisor_kills_children() {
    let pid = {
        let sup = Supervisor::new();
        let (_, rx) = sup
            .spawn(&spec(
                "/bin/sh",
                &["-c", "echo P=$$; while :; do sleep 1; done"],
            ))
            .unwrap();
        let (out, _) = read_until(&rx, T, |o| o.contains("P=") && o.contains('\n'));
        out.split("P=")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
    };
    assert!(
        wait_dead(pid, Duration::from_secs(3)),
        "pid {pid} survived Supervisor drop"
    );
}

#[test]
fn foreground_job_detection() {
    let sup = Supervisor::new();
    let (id, rx) = sup.spawn(&spec("/bin/zsh", &["-f", "-i"])).unwrap();
    read_until(&rx, T, |o| o.contains('%'));
    assert!(
        !sup.has_foreground_job(id).unwrap(),
        "idle shell has no foreground job"
    );
    sup.write(id, b"sleep 303\n").unwrap();
    let end = Instant::now() + T;
    while !sup.has_foreground_job(id).unwrap() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        sup.has_foreground_job(id).unwrap(),
        "running command should be detected"
    );
    sup.terminate(id, Duration::from_secs(3)).unwrap();
}
