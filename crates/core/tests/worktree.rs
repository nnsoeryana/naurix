#![cfg(unix)] // Unix shell hooks and mock programs; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! Creating and removing worktrees for agents (ADR 0028), against real repositories, including hostile ones.
use naurix_core::approval::Scope;
use naurix_core::git::worktree::{
    create, create_action, folder_missing, plan_create, plan_remove, remove, remove_action,
    WorktreeStore,
};
use naurix_core::persistence::{new_id, Db};
use naurix_core::pty::Outbound;
use naurix_core::session::{SessionLaunch, SessionManager};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

const NOW: &str = "2026-11-23T11:05:00Z";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nx-wt-{tag}-{}-{}", std::process::id(), new_id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

/// Test-setup git: isolated from the developer's own global config.
fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn repo(tag: &str) -> PathBuf {
    let r = tmp(tag);
    g(&r, &["init", "-q", "-b", "main"]);
    write(&r.join(".gitignore"), ".env\nnode_modules/\n");
    write(&r.join("a.txt"), "one\n");
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "init"]);
    r
}

fn never(_: &Path) -> bool {
    false
}
fn always(_: &Path) -> bool {
    true
}

fn exec(p: &Path, body: &str) {
    write(p, body);
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Everything a test needs: a repository, a worktree folder, a database and a workspace for the repository.
struct Env {
    repo: PathBuf,
    root: PathBuf,
    db: Arc<Db>,
    ws: String,
}

fn env(tag: &str) -> Env {
    let repo = repo(tag);
    let db = Arc::new(Db::open_in_memory().unwrap());
    let ws = db.create_workspace(repo.to_str().unwrap()).unwrap().id;
    Env {
        repo,
        root: tmp(&format!("{tag}-root")),
        db,
        ws,
    }
}

fn plan(e: &Env) -> naurix_core::git::worktree::CreatePlan {
    plan_create(&e.repo, &never, &e.root, &e.ws, "claude-code", NOW).unwrap()
}

#[test]
fn creating_a_worktree_makes_a_branch_and_folder_and_changes_nothing_else() {
    let e = env("create");
    write(&e.repo.join(".env"), "SECRET=1\n");
    write(&e.repo.join("node_modules/x/index.js"), "x");
    let head_before = g(&e.repo, &["rev-parse", "HEAD"]);
    let branch_before = g(&e.repo, &["branch", "--show-current"]);

    let p = plan(&e);
    assert_eq!(p.branch, "naurix/claude-code-20261123-1105");
    assert!(p.path.starts_with(&e.root), "{}", p.path.display());
    assert_eq!(p.base, head_before);
    assert!(
        p.command
            .starts_with("git worktree add -b naurix/claude-code-20261123-1105 "),
        "{}",
        p.command
    );
    assert!(!p.path.exists(), "planning created nothing");
    create(&p, &never).unwrap();

    assert_eq!(
        std::fs::read_to_string(p.path.join("a.txt")).unwrap(),
        "one\n"
    );
    assert_eq!(g(&p.path, &["branch", "--show-current"]), p.branch);
    assert!(g(&e.repo, &["branch", "--list", &p.branch]).contains(&p.branch));
    assert!(g(&e.repo, &["worktree", "list"]).contains(&*p.path.to_string_lossy()));
    // The user's own checkout is untouched.
    assert_eq!(g(&e.repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(g(&e.repo, &["branch", "--show-current"]), branch_before);
    assert_eq!(g(&e.repo, &["status", "--porcelain"]), "");
    // Untracked and ignored files, which can hold secrets, are never carried over.
    assert!(!p.path.join(".env").exists());
    assert!(!p.path.join("node_modules").exists());
}

#[test]
fn a_repository_hook_does_not_run_while_a_worktree_is_created() {
    // A hook in the repository's own .git/hooks, and one named by its own config. Both are what a
    // hostile repository would use. First prove they fire for a plain `git worktree add`, so this test means something.
    let r = repo("hook");
    let tools = tmp("hook-tools");
    let m_hook = tools.join("MARK-hook");
    let m_cfg = tools.join("MARK-cfg");
    exec(
        &r.join(".git/hooks/post-checkout"),
        &format!("#!/bin/sh\ntouch {}\n", m_hook.display()),
    );
    let cfg_hooks = tools.join("cfg-hooks");
    exec(
        &cfg_hooks.join("post-checkout"),
        &format!("#!/bin/sh\ntouch {}\n", m_cfg.display()),
    );

    let naive = tools.join("naive-wt");
    g(
        &r,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "naive",
            naive.to_str().unwrap(),
        ],
    );
    assert!(
        m_hook.exists(),
        "test setup: a plain git worktree add runs the repository's own hook"
    );
    std::fs::remove_file(&m_hook).unwrap();
    g(
        &r,
        &["config", "core.hooksPath", cfg_hooks.to_str().unwrap()],
    );
    let naive2 = tools.join("naive-wt2");
    g(
        &r,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "naive2",
            naive2.to_str().unwrap(),
        ],
    );
    assert!(
        m_cfg.exists(),
        "test setup: and so does a hooks path set in its config"
    );
    std::fs::remove_file(&m_cfg).unwrap();

    let db = Db::open_in_memory().unwrap();
    let ws = db.create_workspace(r.to_str().unwrap()).unwrap().id;
    let root = tmp("hook-root");
    let p = plan_create(&r, &never, &root, &ws, "claude-code", NOW).unwrap();
    create(&p, &never).unwrap();
    assert!(p.path.is_dir());
    assert!(
        !m_hook.exists(),
        "the repository's own post-checkout hook must not run"
    );
    assert!(
        !m_cfg.exists(),
        "nor a hook named by the repository's config"
    );
}

#[test]
fn a_repository_with_a_local_filter_is_refused_until_it_is_trusted() {
    let e = env("filter");
    let tools = tmp("filter-tools");
    let marker = tools.join("MARK-filter");
    exec(
        &tools.join("clean.sh"),
        &format!("#!/bin/sh\ntouch {}\ncat\n", marker.display()),
    );
    g(
        &e.repo,
        &[
            "config",
            "filter.evil.clean",
            tools.join("clean.sh").to_str().unwrap(),
        ],
    );
    let err = plan_create(&e.repo, &never, &e.root, &e.ws, "claude-code", NOW).unwrap_err();
    assert!(err.to_string().contains("Trust it"), "{err}");
    assert!(!marker.exists(), "refusing must not run the filter");
    assert!(
        plan_create(&e.repo, &always, &e.root, &e.ws, "claude-code", NOW).is_ok(),
        "trusted: allowed"
    );
}

#[test]
fn things_that_are_not_a_plain_repository_with_a_commit_are_refused_clearly() {
    let root = tmp("refuse-root");
    let plain = tmp("not-a-repo");
    assert!(plan_create(&plain, &never, &root, "w", "a", NOW)
        .unwrap_err()
        .to_string()
        .contains("not a Git repository"));

    let empty = tmp("no-commits");
    g(&empty, &["init", "-q", "-b", "main"]);
    assert!(plan_create(&empty, &never, &root, "w", "a", NOW)
        .unwrap_err()
        .to_string()
        .contains("no commits"));
    assert!(!root.join("w").exists(), "nothing was created");
}

#[test]
fn a_taken_branch_or_folder_gets_a_numbered_name_instead_of_failing_or_overwriting() {
    let e = env("collide");
    let first = plan(&e);
    g(&e.repo, &["branch", &first.branch]);
    let second = plan(&e);
    assert_eq!(second.branch, format!("{}-1", first.branch));
    std::fs::create_dir_all(&second.path).unwrap(); // a folder is in the way too
    let third = plan(&e);
    assert_eq!(third.branch, format!("{}-2", first.branch));
    create(&third, &never).unwrap();
    assert!(third.path.join("a.txt").exists());
}

#[test]
fn the_creation_approval_is_once_only_and_ignores_the_clock_but_not_what_matters() {
    let e = env("action");
    let a = create_action(&plan(&e), &e.ws);
    assert_eq!(
        a.allowed_scopes,
        vec![Scope::Once],
        "a Git mutation can only ever be approved once"
    );
    let later = plan_create(
        &e.repo,
        &never,
        &e.root,
        &e.ws,
        "claude-code",
        "2026-11-23T18:40:00Z",
    )
    .unwrap();
    assert_eq!(
        create_action(&later, &e.ws).fingerprint,
        a.fingerprint,
        "a different time does not re-ask"
    );

    let other_agent = plan_create(&e.repo, &never, &e.root, &e.ws, "opencode", NOW).unwrap();
    assert_ne!(
        create_action(&other_agent, &e.ws).fingerprint,
        a.fingerprint
    );
    write(&e.repo.join("b.txt"), "two\n");
    g(&e.repo, &["add", "-A"]);
    g(&e.repo, &["commit", "-q", "-m", "second"]);
    assert_ne!(
        create_action(&plan(&e), &e.ws).fingerprint,
        a.fingerprint,
        "a new base commit re-asks"
    );
    let elsewhere = plan_create(
        &e.repo,
        &never,
        &tmp("other-root"),
        &e.ws,
        "claude-code",
        NOW,
    )
    .unwrap();
    assert_ne!(
        create_action(&elsewhere, &e.ws).fingerprint,
        a.fingerprint,
        "a different folder re-asks"
    );
    assert!(
        a.display.contains("untracked or ignored files") && a.display.contains("hooks do not run")
    );
}

/// Create a worktree and record it, as the launch path does.
fn made(
    e: &Env,
) -> (
    naurix_core::git::worktree::CreatePlan,
    naurix_core::git::worktree::ManagedWorktree,
) {
    let p = plan(e);
    create(&p, &never).unwrap();
    let rec = WorktreeStore::new(&e.db).insert(&e.ws, &p).unwrap();
    (p, rec)
}

#[test]
fn removing_a_clean_worktree_removes_the_folder_and_always_keeps_the_branch() {
    let e = env("remove");
    let (p, rec) = made(&e);
    write(&p.path.join("agent.txt"), "work\n");
    g(&p.path, &["add", "-A"]);
    g(&p.path, &["commit", "-q", "-m", "agent work"]);
    let commit = g(&p.path, &["rev-parse", "HEAD"]);

    let rp = plan_remove(&rec, &e.root, false, &never).unwrap();
    assert!(rp.display().contains("The branch 'naurix/") && rp.display().contains("is kept"));
    remove(&rp, &never).unwrap();

    assert!(!p.path.exists());
    assert!(!g(&e.repo, &["worktree", "list"]).contains(&*p.path.to_string_lossy()));
    assert_eq!(
        g(&e.repo, &["rev-parse", &p.branch]),
        commit,
        "the agent's commit is still on its branch"
    );
}

#[test]
fn a_worktree_with_uncommitted_work_is_never_removed_and_never_forced() {
    let e = env("dirty");
    let (p, rec) = made(&e);
    write(&p.path.join("scratch.txt"), "untracked\n");
    let err = plan_remove(&rec, &e.root, false, &never).unwrap_err();
    assert!(
        err.to_string().contains("uncommitted work") && err.to_string().contains("never forces"),
        "{err}"
    );
    std::fs::remove_file(p.path.join("scratch.txt")).unwrap();

    write(&p.path.join("a.txt"), "changed\n");
    assert!(
        plan_remove(&rec, &e.root, false, &never).is_err(),
        "a modified tracked file counts too"
    );
    g(&p.path, &["checkout", "--", "a.txt"]);

    // Work appears between the review and the removal: Git itself refuses, because no --force is ever passed.
    let reviewed = plan_remove(&rec, &e.root, false, &never).unwrap();
    write(&p.path.join("late.txt"), "late\n");
    assert!(remove(&reviewed, &never).is_err());
    assert!(p.path.join("late.txt").exists(), "nothing was deleted");
}

#[test]
fn the_removal_approval_changes_when_the_worktree_changes() {
    let e = env("remove-action");
    let (p, rec) = made(&e);
    let a = remove_action(&plan_remove(&rec, &e.root, false, &never).unwrap(), &e.ws);
    assert_eq!(a.allowed_scopes, vec![Scope::Once]);
    assert_eq!(
        remove_action(&plan_remove(&rec, &e.root, false, &never).unwrap(), &e.ws).fingerprint,
        a.fingerprint
    );
    write(&p.path.join("n.txt"), "n\n");
    g(&p.path, &["add", "-A"]);
    g(&p.path, &["commit", "-q", "-m", "n"]);
    let b = remove_action(&plan_remove(&rec, &e.root, false, &never).unwrap(), &e.ws);
    assert_ne!(
        a.fingerprint, b.fingerprint,
        "a new commit since the review asks again"
    );
}

#[test]
fn removal_is_refused_for_locked_in_use_foreign_and_unregistered_worktrees() {
    let e = env("refuse-remove");
    let (p, rec) = made(&e);

    assert!(plan_remove(&rec, &e.root, true, &never)
        .unwrap_err()
        .to_string()
        .contains("still running"));

    g(&e.repo, &["worktree", "lock", p.path.to_str().unwrap()]);
    assert!(plan_remove(&rec, &e.root, false, &never)
        .unwrap_err()
        .to_string()
        .contains("locked"));
    g(&e.repo, &["worktree", "unlock", p.path.to_str().unwrap()]);

    // A record that points outside Naurix's folder (for example a tampered database) is never removed.
    let mut foreign = rec.clone();
    foreign.path = e.repo.to_string_lossy().into_owned();
    assert!(plan_remove(&foreign, &e.root, false, &never)
        .unwrap_err()
        .to_string()
        .contains("not inside"));

    // A folder under Naurix's root that Git does not know as a worktree.
    let stray = e.root.join("stray");
    std::fs::create_dir_all(&stray).unwrap();
    let mut unregistered = rec.clone();
    unregistered.path = stray.to_string_lossy().into_owned();
    assert!(plan_remove(&unregistered, &e.root, false, &never)
        .unwrap_err()
        .to_string()
        .contains("does not list"));
    assert!(
        p.path.exists() && stray.exists(),
        "refusals deleted nothing"
    );
}

#[test]
fn a_worktree_whose_folder_was_deleted_by_hand_can_have_its_record_forgotten() {
    let e = env("missing");
    let (p, rec) = made(&e);
    assert!(!folder_missing(&rec));
    std::fs::remove_dir_all(&p.path).unwrap();
    assert!(folder_missing(&rec));
    assert!(plan_remove(&rec, &e.root, false, &never)
        .unwrap_err()
        .to_string()
        .contains("no longer exists"));
    WorktreeStore::new(&e.db).delete(&rec.id).unwrap();
    assert!(WorktreeStore::new(&e.db).get(&rec.id).unwrap().is_none());
}

#[test]
fn records_survive_their_workspace_and_are_listed_as_orphans_without_touching_the_folder() {
    let e = env("orphan");
    let (p, rec) = made(&e);
    let store = WorktreeStore::new(&e.db);
    assert_eq!(store.list(Some(&e.ws)).unwrap().len(), 1);
    assert!(store.orphaned().unwrap().is_empty());

    e.db.remove_workspace(&e.ws).unwrap();
    assert!(
        p.path.is_dir(),
        "removing a workspace never deletes folders"
    );
    let orphans = store.orphaned().unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(
        (orphans[0].id.as_str(), orphans[0].workspace_id.clone()),
        (rec.id.as_str(), None)
    );
    // It can still be removed (with the same checks), with no workspace to attach the approval to.
    let rp = plan_remove(&orphans[0], &e.root, false, &never).unwrap();
    assert_eq!(remove_action(&rp, "").workspace(), None);
    remove(&rp, &never).unwrap();
    assert!(!p.path.exists());
}

#[test]
fn a_session_runs_in_the_worktree_and_is_linked_to_it() {
    let e = env("session");
    let (p, rec) = made(&e);
    let mgr = Arc::new(SessionManager::new(
        e.db.clone(),
        Duration::from_millis(900),
    ));
    let (tx, rx) = channel();
    let id = mgr
        .start(
            &e.ws,
            SessionLaunch {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "pwd; sleep 30".into()],
                env: vec![],
                kind: naurix_core::persistence::SessionKind::Agent,
                title: "wt".into(),
                adapter_id: Some("mock".into()),
                approval_id: None,
                worktree_id: Some(rec.id.clone()),
                cwd: Some(p.path.clone()),
            },
            24,
            80,
            move |o| tx.send(o).is_ok(),
        )
        .unwrap();
    let mut out = String::new();
    while !out.contains(&*p.path.to_string_lossy()) {
        match rx.recv_timeout(Duration::from_secs(10)).expect("no output") {
            Outbound::Data(d) => out.push_str(&String::from_utf8_lossy(&d)),
            Outbound::Exited(_) => break,
        }
    }
    assert!(
        out.contains(&*p.path.to_string_lossy()),
        "ran in the worktree: {out}"
    );
    assert_eq!(mgr.list()[0].worktree_id.as_deref(), Some(rec.id.as_str()));
    let linked: Option<String> =
        e.db.c_for_test()
            .query_row("SELECT worktree_id FROM sessions", [], |r| r.get(0))
            .unwrap();
    assert_eq!(linked.as_deref(), Some(rec.id.as_str()));
    // The caller (Tauri) refuses removal while a session is running there.
    let running_here = mgr
        .list()
        .iter()
        .any(|s| s.worktree_id.as_deref() == Some(rec.id.as_str()) && s.running());
    assert!(plan_remove(&rec, &e.root, running_here, &never).is_err());
    mgr.close(id).unwrap();
}
