#![cfg(unix)] // Unix mock programs and symlinks; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Agent store rules and a mock CLI agent launched through the real PTY supervisor.
use naurix_core::agents::*;
use naurix_core::persistence::Db;
use naurix_core::pty::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "at-agents-{tag}-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn make_exec(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn new_custom(name: &str, exe: &str) -> CustomAgent {
    CustomAgent {
        id: String::new(),
        name: name.into(),
        executable: exe.into(),
        args: vec![],
        env: vec![],
    }
}

#[test]
fn custom_agents_crud_and_limits() {
    let db = Db::open_in_memory().unwrap();
    let st = AgentStore::new(&db);
    assert_eq!(
        st.all().unwrap().len(),
        PRESETS.len(),
        "presets only, no customs"
    );

    let saved = st.save_custom(new_custom("  Aider ", "aider")).unwrap();
    assert!(saved.id.starts_with("custom-"));
    assert_eq!(saved.name, "Aider");
    assert_eq!(st.all().unwrap().len(), PRESETS.len() + 1);
    assert_eq!(st.get(&saved.id).unwrap().kind, AgentKind::Custom);

    let mut edited = saved.clone();
    edited.args = vec!["--yes".into()];
    st.save_custom(edited).unwrap();
    assert_eq!(st.get(&saved.id).unwrap().args, ["--yes"]);

    assert!(st.save_custom(new_custom("", "x")).is_err());
    assert!(matches!(
        st.save_custom(CustomAgent {
            id: "custom-missing".into(),
            ..new_custom("n", "n")
        }),
        Err(AgentError::Unknown)
    ));

    st.delete_custom(&saved.id).unwrap();
    assert!(matches!(st.get(&saved.id), Err(AgentError::Unknown)));
    assert!(matches!(
        st.delete_custom(&saved.id),
        Err(AgentError::Unknown)
    ));

    for i in 0..20 {
        st.save_custom(new_custom(&format!("a{i}"), "x")).unwrap();
    }
    assert!(st.save_custom(new_custom("one-too-many", "x")).is_err());
}

#[test]
fn custom_agents_persist_across_reopen() {
    let dir = tmpdir("persist");
    let path = dir.join("db.sqlite");
    let id = AgentStore::new(&Db::open(&path).unwrap())
        .save_custom(new_custom("Persisted", "tool"))
        .unwrap()
        .id;
    let db = Db::open(&path).unwrap();
    assert_eq!(AgentStore::new(&db).get(&id).unwrap().name, "Persisted");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn review_rule_for_presets_and_custom_agents() {
    let dir = tmpdir("review");
    make_exec(&dir, "claude", "#!/bin/sh\n");
    make_exec(&dir, "mytool", "#!/bin/sh\n");
    let dirs = vec![dir.clone()];
    let db = Db::open_in_memory().unwrap();
    let st = AgentStore::new(&db);

    // Preset: first launch needs review; after approval it does not.
    assert!(matches!(
        st.prepare_launch("claude-code", &dirs, false),
        Err(AgentError::ReviewRequired)
    ));
    let (_, plan) = st.prepare_launch("claude-code", &dirs, true).unwrap();
    st.approve("claude-code", &plan.program).unwrap();
    assert!(st.prepare_launch("claude-code", &dirs, false).is_ok());

    // Choosing a different executable invalidates the approval.
    let other = make_exec(&dir, "claude-alt", "#!/bin/sh\n");
    st.set_path_override("claude-code", Some(other.to_str().unwrap()))
        .unwrap();
    assert!(matches!(
        st.prepare_launch("claude-code", &dirs, false),
        Err(AgentError::ReviewRequired)
    ));
    st.set_path_override("claude-code", None).unwrap();
    assert!(matches!(
        st.prepare_launch("claude-code", &dirs, false),
        Err(AgentError::ReviewRequired)
    ));
    assert!(st
        .set_path_override("claude-code", Some("/no/such/file"))
        .is_err());
    assert!(st.set_path_override("no-such-agent", None).is_err());

    // Custom agents must always be reviewed, even after approve().
    let c = st.save_custom(new_custom("Mine", "mytool")).unwrap();
    assert!(matches!(
        st.prepare_launch(&c.id, &dirs, false),
        Err(AgentError::ReviewRequired)
    ));
    let (_, p) = st.prepare_launch(&c.id, &dirs, true).unwrap();
    st.approve(&c.id, &p.program).unwrap();
    assert!(matches!(
        st.prepare_launch(&c.id, &dirs, false),
        Err(AgentError::ReviewRequired)
    ));

    // Not installed: clear error, regardless of review.
    assert!(matches!(
        st.prepare_launch("codex", &dirs, true),
        Err(AgentError::NotFound { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

fn collect(
    rx: &std::sync::mpsc::Receiver<SessionEvent>,
    until: impl Fn(&str) -> bool,
) -> (String, Option<ExitInfo>) {
    let end = Instant::now() + Duration::from_secs(10);
    let mut out = String::new();
    while Instant::now() < end {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(SessionEvent::Output(b)) => {
                out.push_str(&String::from_utf8_lossy(&b));
                if until(&out) {
                    return (out, None);
                }
            }
            Ok(SessionEvent::Exited(i)) => return (out, Some(i)),
            Err(_) => {}
        }
    }
    (out, None)
}

/// A scripted stand-in for an agent CLI: proves launch, args, env, interaction and exit reporting
/// without any real provider or account.
#[test]
fn mock_agent_launches_interacts_and_reports_exit() {
    let dir = tmpdir("mock");
    make_exec(
        &dir,
        "mock-agent",
        "#!/bin/sh\necho \"MOCK READY args=$* mode=$MOCK_MODE\"\ncase \":$PATH:\" in *\":$MOCK_DIR:\"*) echo PATH_OK;; esac\nwhile read line; do\n  [ \"$line\" = quit ] && exit 3\n  echo \"GOT:$line\"\ndone\n",
    );
    let db = Db::open_in_memory().unwrap();
    let st = AgentStore::new(&db);
    let mut c = new_custom("Mock Agent", "mock-agent");
    c.args = vec!["--flag".into(), "two words".into()];
    c.env = vec![
        EnvVar {
            name: "MOCK_MODE".into(),
            value: "test".into(),
        },
        EnvVar {
            name: "MOCK_DIR".into(),
            value: dir.to_string_lossy().into_owned(),
        },
    ];
    let c = st.save_custom(c).unwrap();

    let dirs = search_dirs(
        Some(std::ffi::OsStr::new("/usr/bin:/bin")),
        &tmpdir("emptyhome"),
    );
    let mut all_dirs = vec![dir.clone()];
    all_dirs.extend(dirs);
    let (_, plan) = st.prepare_launch(&c.id, &all_dirs, true).unwrap();

    let sup = Supervisor::new();
    let (id, rx) = sup
        .spawn(&SpawnSpec {
            program: plan.program.to_string_lossy().into_owned(),
            args: plan.args.clone(),
            cwd: std::env::temp_dir(),
            env: plan.env.clone(),
            rows: 24,
            cols: 80,
        })
        .unwrap();
    let (out, _) = collect(&rx, |o| o.contains("PATH_OK"));
    assert!(
        out.contains("MOCK READY args=--flag two words mode=test"),
        "{out:?}"
    );
    assert!(
        out.contains("PATH_OK"),
        "augmented PATH reached the child: {out:?}"
    );

    sup.write(id, b"hello\n").unwrap();
    let (out, _) = collect(&rx, |o| o.contains("GOT:hello"));
    assert!(out.contains("GOT:hello"), "{out:?}");
    sup.write(id, b"quit\n").unwrap();
    let (_, exit) = collect(&rx, |_| false);
    assert_eq!(
        exit.unwrap().code,
        Some(3),
        "exit code of the agent is reported"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn custom_agents_refuse_values_that_look_like_credentials() {
    use naurix_core::agents::{validate_custom, EnvVar};
    let mut a = new_custom("Tool", "tool");
    a.env = vec![EnvVar {
        name: "FOO".into(),
        value: "ghp_abcdefghijklmnopqrstuvwxyz0123456789".into(),
    }];
    assert!(validate_custom(&a).is_err(), "token under an innocent name");
    a.env = vec![EnvVar {
        name: "LANG".into(),
        value: "en_US.UTF-8".into(),
    }];
    assert!(validate_custom(&a).is_ok());
    a.args = vec!["--token=Zq8dK2mP9xV4nB7cL1wR6tY3uH5jF0aS".into()];
    assert!(validate_custom(&a).is_err(), "token in an argument");
}
