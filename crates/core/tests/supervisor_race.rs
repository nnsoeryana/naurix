#![cfg(unix)] // Unix mock programs (ADR 0021).
//! A child that exits immediately must still leave the registry: the exit callback can run before the
//! session has been inserted, and must not leave a ghost behind.
use naurix_core::pty::{SpawnSpec, Supervisor};
use std::time::{Duration, Instant};

#[test]
fn fast_exiting_children_never_leave_a_ghost_in_the_registry() {
    let dir = std::env::temp_dir();
    let sup = Supervisor::new();
    for _ in 0..400 {
        sup.spawn(&SpawnSpec {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "exit 0".into()],
            cwd: dir.clone(),
            env: vec![],
            rows: 24,
            cols: 80,
        })
        .unwrap();
    }
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end && !sup.list().is_empty() {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        sup.list().is_empty(),
        "{} exited sessions are still registered",
        sup.list().len()
    );
}
