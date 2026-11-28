#![cfg(unix)] // Unix mock programs and symlinks; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Agent store rules and a mock CLI agent launched through the real PTY supervisor.
use naurix_core::agents::*;
use naurix_core::approval::{ApprovalGate, Decision, Scope, Verdict};
use naurix_core::persistence::Db;
use naurix_core::pty::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
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
        ..Default::default()
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

/// The action the gate sees for launching `id` in `ws`, built exactly as `agent_launch` builds it.
fn action_for(
    st: &AgentStore<'_>,
    id: &str,
    dirs: &[PathBuf],
    ws: &str,
) -> naurix_core::approval::Action {
    let (def, plan) = st.prepare_launch(id, dirs).unwrap();
    launch_action(&def, &plan.program, ws)
}

fn allowed(g: &ApprovalGate, a: &naurix_core::approval::Action) -> bool {
    matches!(g.authorize(a).unwrap(), Verdict::Allowed { .. })
}

fn request(
    g: &ApprovalGate,
    a: &naurix_core::approval::Action,
) -> naurix_core::approval::ApprovalRequest {
    match g.preview(a).unwrap() {
        Verdict::Required(r) => r,
        v => panic!("expected a request, got {v:?}"),
    }
}

#[test]
fn review_rule_for_presets_and_custom_agents_now_runs_through_the_gate() {
    let dir = tmpdir("review");
    make_exec(&dir, "claude", "#!/bin/sh\n");
    make_exec(&dir, "mytool", "#!/bin/sh\n");
    let dirs = vec![dir.clone()];
    let db = Arc::new(Db::open_in_memory().unwrap());
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap().id;
    let gate = ApprovalGate::new(db.clone());
    let st = AgentStore::new(&db);

    // Preset: the first launch needs approval; approved for every workspace it needs no more.
    let a = action_for(&st, "claude-code", &dirs, &ws);
    assert!(a.allowed_scopes.contains(&Scope::App));
    let r = request(&gate, &a);
    assert!(r.display.contains("claude"), "{}", r.display);
    gate.decide(&r.id, Decision::Allow(Scope::App)).unwrap();
    assert!(allowed(&gate, &a));
    assert!(gate.has_app_grant(&a).unwrap());

    // Choosing a different executable invalidates the approval (different fingerprint).
    let other = make_exec(&dir, "claude-alt", "#!/bin/sh\n");
    st.set_path_override("claude-code", Some(other.to_str().unwrap()))
        .unwrap();
    let moved = action_for(&st, "claude-code", &dirs, &ws);
    assert!(!allowed(&gate, &moved));
    st.set_path_override("claude-code", None).unwrap();
    assert!(
        allowed(&gate, &action_for(&st, "claude-code", &dirs, &ws)),
        "back to the approved path"
    );
    assert!(st
        .set_path_override("claude-code", Some("/no/such/file"))
        .is_err());
    assert!(st.set_path_override("no-such-agent", None).is_err());

    // Custom agent: cannot be approved for every workspace; a workspace approval binds to the full definition.
    let c = st.save_custom(new_custom("Mine", "mytool")).unwrap();
    let ca = action_for(&st, &c.id, &dirs, &ws);
    assert!(!ca.allowed_scopes.contains(&Scope::App));
    let cr = request(&gate, &ca);
    assert!(matches!(
        gate.decide(&cr.id, Decision::Allow(Scope::App)),
        Err(naurix_core::approval::ApprovalError::ScopeNotAllowed(
            Scope::App
        ))
    ));
    gate.decide(&cr.id, Decision::Allow(Scope::Workspace))
        .unwrap();
    assert!(allowed(&gate, &ca));
    let mut edited = new_custom("Mine", "mytool");
    edited.id = c.id.clone();
    edited.args.push("--different".into());
    st.save_custom(edited).unwrap();
    assert!(
        !allowed(&gate, &action_for(&st, &c.id, &dirs, &ws)),
        "editing the command asks again"
    );
    assert!(
        !gate.has_app_grant(&ca).unwrap(),
        "a custom agent never holds an app-wide grant"
    );

    // Not installed: clear error, regardless of approval.
    assert!(matches!(
        st.prepare_launch("codex", &dirs),
        Err(AgentError::NotFound { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_action_shows_the_full_command_but_persists_only_a_name_and_path() {
    let dir = tmpdir("display");
    make_exec(&dir, "mytool", "#!/bin/sh\n");
    let db = Arc::new(Db::open_in_memory().unwrap());
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap().id;
    let st = AgentStore::new(&db);
    let mut c = new_custom("Mine", "mytool");
    c.args = vec!["--flag".into(), "has space".into()];
    c.env = vec![EnvVar {
        name: "MODE".into(),
        value: "fast".into(),
    }];
    let c = st.save_custom(c).unwrap();
    let a = action_for(&st, &c.id, std::slice::from_ref(&dir), &ws);
    assert!(
        a.display.contains("--flag 'has space'") && a.display.contains("MODE=fast"),
        "{}",
        a.display
    );
    assert!(
        a.summary.starts_with("Mine at ")
            && !a.summary.contains("--flag")
            && !a.summary.contains("MODE"),
        "{}",
        a.summary
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_preset_approvals_become_app_grants_once_and_the_old_setting_is_removed() {
    let dir = tmpdir("legacy");
    let exe = make_exec(&dir, "claude", "#!/bin/sh\n");
    let db = Arc::new(Db::open_in_memory().unwrap());
    let gate = ApprovalGate::new(db.clone());
    let legacy = serde_json::json!({
        "claude-code": exe.to_string_lossy(),
        "no-such-agent": "/x",
    });
    db.set_setting("agents.approved", &legacy.to_string())
        .unwrap();

    assert_eq!(migrate_legacy_approvals(&db, &gate).unwrap(), 1);
    assert_eq!(db.get_setting("agents.approved").unwrap(), None);
    let a = launch_action(&AgentDef::preset("claude-code").unwrap(), &exe, "");
    assert!(
        gate.has_app_grant(&a).unwrap(),
        "the earlier approval still holds, so no new prompt"
    );
    assert_eq!(gate.list_active().unwrap().len(), 1);
    // Running it again changes nothing.
    assert_eq!(migrate_legacy_approvals(&db, &gate).unwrap(), 0);
    assert_eq!(gate.list_active().unwrap().len(), 1);
    // A different path was never approved.
    let elsewhere = launch_action(
        &AgentDef::preset("claude-code").unwrap(),
        std::path::Path::new("/tmp/other/claude"),
        "",
    );
    assert!(!gate.has_app_grant(&elsewhere).unwrap());
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
    let (_, plan) = st.prepare_launch(&c.id, &all_dirs).unwrap();

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

#[test]
fn custom_agents_saved_before_roles_still_load_and_new_fields_round_trip() {
    let db = Db::open_in_memory().unwrap();
    db.set_setting(
        "agents.custom",
        r#"[{"id":"custom-old","name":"Old","executable":"old","args":[],"env":[]}]"#,
    )
    .unwrap();
    let st = AgentStore::new(&db);
    let old = st.get("custom-old").unwrap();
    assert_eq!((old.role, old.success_codes()), (None, vec![0]));

    let saved = st
        .save_custom(CustomAgent {
            role: Some(naurix_core::agents::Role::Purple),
            success_exit_codes: vec![1, 0],
            ..new_custom("Scanner", "semgrep")
        })
        .unwrap();
    let def = st.get(&saved.id).unwrap();
    assert_eq!(def.role, Some(naurix_core::agents::Role::Purple));
    assert_eq!(def.success_codes(), vec![0, 1]);
}
