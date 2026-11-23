//! The approval gate (ADR 0027): requests, scopes, single use, expiry, revocation, history.
use naurix_core::approval::*;
use naurix_core::persistence::Db;
use std::sync::Arc;
use std::time::Duration;

struct Env {
    db: Arc<Db>,
    ws: Vec<String>,
    dirs: Vec<std::path::PathBuf>,
}

fn env(workspaces: usize) -> Env {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let mut ws = vec![];
    let mut dirs = vec![];
    for i in 0..workspaces {
        let d = std::env::temp_dir().join(format!(
            "nx-approval-{}-{}-{i}",
            std::process::id(),
            naurix_core::persistence::new_id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        ws.push(db.create_workspace(d.to_str().unwrap()).unwrap().id);
        dirs.push(d);
    }
    Env { db, ws, dirs }
}

impl Drop for Env {
    fn drop(&mut self) {
        for d in &self.dirs {
            std::fs::remove_dir_all(d).ok();
        }
    }
}

const SECRET_ARG: &str = "--token-looking-argument-xyz";

fn action(ws: &str, parts: &[&str], scopes: &[Scope]) -> Action {
    Action::new(
        ActionClass::LaunchAgent,
        ws,
        parts,
        "Claude Code at /opt/bin/claude".into(),
        format!("/opt/bin/claude {SECRET_ARG}\nEnvironment: A=1"),
        scopes.to_vec(),
    )
}

const ALL: [Scope; 3] = [Scope::Once, Scope::Workspace, Scope::App];

fn request(g: &ApprovalGate, a: &Action) -> ApprovalRequest {
    match g.authorize(a).unwrap() {
        Verdict::Required(r) => r,
        Verdict::Allowed { .. } => panic!("expected a request"),
    }
}

fn allowed(g: &ApprovalGate, a: &Action) -> bool {
    matches!(g.authorize(a).unwrap(), Verdict::Allowed { .. })
}

#[test]
fn an_unknown_action_needs_a_request_that_shows_the_server_built_display() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["claude", "/opt/bin/claude"], &ALL);
    let r = request(&g, &a);
    assert!(
        r.display.contains(SECRET_ARG),
        "the review shows the full command"
    );
    assert_eq!(r.allowed_scopes, ALL);
    assert_eq!(r.class, ActionClass::LaunchAgent);
    assert!(r.expires_in_secs > 0);
}

#[test]
fn a_once_grant_authorizes_exactly_one_action() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["x"], &ALL);
    let r = request(&g, &a);
    let id = g
        .decide(&r.id, Decision::Allow(Scope::Once))
        .unwrap()
        .unwrap();
    match g.authorize(&a).unwrap() {
        Verdict::Allowed { approval_id } => assert_eq!(approval_id, id),
        v => panic!("{v:?}"),
    }
    assert!(!allowed(&g, &a), "a second use needs a new decision");
    let h = g.history(10).unwrap();
    assert_eq!(
        (h[0].state.as_str(), h[0].scope.as_str()),
        ("consumed", "once")
    );
    assert!(g.list_active().unwrap().is_empty());
}

#[test]
fn a_once_grant_cannot_be_spent_twice_even_under_concurrent_use() {
    let e = env(1);
    let g = Arc::new(ApprovalGate::new(e.db.clone()));
    let a = action(&e.ws[0], &["x"], &ALL);
    let r = request(&g, &a);
    g.decide(&r.id, Decision::Allow(Scope::Once)).unwrap();
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let (g, a) = (g.clone(), a.clone());
            std::thread::spawn(move || allowed(&g, &a))
        })
        .collect();
    let wins = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|w| *w)
        .count();
    assert_eq!(wins, 1, "exactly one launch may use a once grant");
}

#[test]
fn a_workspace_grant_covers_its_workspace_only_and_any_number_of_uses() {
    let e = env(2);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["x"], &ALL);
    let r = request(&g, &a);
    g.decide(&r.id, Decision::Allow(Scope::Workspace)).unwrap();
    assert!(allowed(&g, &a) && allowed(&g, &a) && allowed(&g, &a));
    let other = action(&e.ws[1], &["x"], &ALL);
    assert!(!allowed(&g, &other), "another workspace asks again");
    assert!(g.list_active().unwrap()[0].last_used_at.is_some());
}

#[test]
fn an_app_grant_covers_every_workspace_and_outlives_a_removed_one_unlike_a_workspace_grant() {
    let e = env(3);
    let g = ApprovalGate::new(e.db.clone());
    let app = action(&e.ws[0], &["preset"], &ALL);
    g.decide(&request(&g, &app).id, Decision::Allow(Scope::App))
        .unwrap();
    let scoped = action(&e.ws[0], &["custom"], &ALL);
    g.decide(&request(&g, &scoped).id, Decision::Allow(Scope::Workspace))
        .unwrap();

    assert!(allowed(&g, &action(&e.ws[1], &["preset"], &ALL)));
    assert!(allowed(&g, &action(&e.ws[2], &["preset"], &ALL)));
    assert!(!allowed(&g, &action(&e.ws[1], &["custom"], &ALL)));

    e.db.remove_workspace(&e.ws[0]).unwrap();
    let left: Vec<_> = g
        .list_active()
        .unwrap()
        .into_iter()
        .map(|a| a.scope)
        .collect();
    assert_eq!(
        left,
        ["app"],
        "workspace grants go with the workspace, app grants stay"
    );
}

#[test]
fn any_change_to_what_is_launched_invalidates_every_grant() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(
        &e.ws[0],
        &["claude", "/opt/bin/claude", "--model", "x"],
        &ALL,
    );
    g.decide(&request(&g, &a).id, Decision::Allow(Scope::App))
        .unwrap();
    assert!(allowed(&g, &a));
    for changed in [
        &["claude", "/opt/bin/claude", "--model", "y"][..],
        &["claude", "/tmp/evil/claude", "--model", "x"][..],
        &["claude", "/opt/bin/claude"][..],
        &["claude", "/opt/bin/claude", "--model", "x", "--extra"][..],
    ] {
        assert!(
            !allowed(&g, &action(&e.ws[0], changed, &ALL)),
            "{changed:?}"
        );
    }
}

#[test]
fn a_scope_the_action_does_not_allow_is_refused_and_leaves_the_request_open() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["custom"], &[Scope::Once, Scope::Workspace]);
    let r = request(&g, &a);
    assert!(matches!(
        g.decide(&r.id, Decision::Allow(Scope::App)),
        Err(ApprovalError::ScopeNotAllowed(Scope::App))
    ));
    assert!(g.list_active().unwrap().is_empty(), "nothing was granted");
    assert!(
        g.decide(&r.id, Decision::Allow(Scope::Once))
            .unwrap()
            .is_some(),
        "still open"
    );
}

#[test]
fn a_request_is_single_use_and_a_denial_grants_nothing() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["x"], &ALL);
    let r = request(&g, &a);
    assert_eq!(g.decide(&r.id, Decision::Deny).unwrap(), None);
    assert!(matches!(
        g.decide(&r.id, Decision::Allow(Scope::App)),
        Err(ApprovalError::UnknownRequest)
    ));
    assert!(matches!(
        g.decide("no-such-request", Decision::Deny),
        Err(ApprovalError::UnknownRequest)
    ));
    assert!(!allowed(&g, &a));

    let r2 = request(&g, &a);
    g.decide(&r2.id, Decision::Allow(Scope::Workspace)).unwrap();
    assert!(matches!(
        g.decide(&r2.id, Decision::Allow(Scope::App)),
        Err(ApprovalError::UnknownRequest)
    ));
}

#[test]
fn a_request_cannot_be_retargeted_the_decision_binds_the_action_the_backend_built() {
    // The UI supplies only a request id and a scope. Approving request A can never authorize action B.
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["benign"], &ALL);
    let b = action(&e.ws[0], &["dangerous"], &ALL);
    let ra = request(&g, &a);
    g.decide(&ra.id, Decision::Allow(Scope::App)).unwrap();
    assert!(allowed(&g, &a));
    assert!(!allowed(&g, &b));
}

#[test]
fn requests_and_once_grants_expire() {
    let e = env(1);
    let g = ApprovalGate::with_ttl(e.db.clone(), Duration::from_secs(1));
    let a = action(&e.ws[0], &["x"], &ALL);

    let stale = request(&g, &a);
    std::thread::sleep(Duration::from_millis(1100));
    assert!(matches!(
        g.decide(&stale.id, Decision::Allow(Scope::Once)),
        Err(ApprovalError::UnknownRequest)
    ));

    let r = request(&g, &a);
    g.decide(&r.id, Decision::Allow(Scope::Once)).unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    assert!(!allowed(&g, &a), "an unused once grant does not linger");
    assert_eq!(g.history(5).unwrap()[0].state, "expired");
}

#[test]
fn approving_the_same_thing_twice_does_not_pile_up_grants() {
    // Two launches of the same thing race: both open a request before either is decided.
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["x"], &ALL);
    let (r1, r2) = (request(&g, &a), request(&g, &a));
    let first = g.decide(&r1.id, Decision::Allow(Scope::Workspace)).unwrap();
    let second = g.decide(&r2.id, Decision::Allow(Scope::Workspace)).unwrap();
    assert_eq!(first, second);
    assert_eq!(g.list_active().unwrap().len(), 1);
}

#[test]
fn a_standing_grant_is_used_before_a_once_grant_so_the_once_grant_is_not_wasted() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["x"], &ALL);
    let (r1, r2) = (request(&g, &a), request(&g, &a));
    g.decide(&r1.id, Decision::Allow(Scope::Once)).unwrap();
    g.decide(&r2.id, Decision::Allow(Scope::Workspace)).unwrap();
    assert!(allowed(&g, &a) && allowed(&g, &a));
    assert_eq!(
        g.list_active().unwrap().len(),
        2,
        "the once grant is still unspent"
    );
}

#[test]
fn revoke_and_revoke_all_end_grants_and_keep_a_history() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(&e.ws[0], &["a"], &ALL);
    let b = action(&e.ws[0], &["b"], &ALL);
    let ia = g
        .decide(&request(&g, &a).id, Decision::Allow(Scope::App))
        .unwrap()
        .unwrap();
    g.decide(&request(&g, &b).id, Decision::Allow(Scope::Workspace))
        .unwrap();

    assert!(g.revoke(&ia).unwrap());
    assert!(!g.revoke(&ia).unwrap(), "already ended");
    assert!(!allowed(&g, &a));
    assert!(allowed(&g, &b));
    assert_eq!(g.revoke_all().unwrap(), 1);
    assert!(!allowed(&g, &b));
    let states: Vec<_> = g
        .history(10)
        .unwrap()
        .into_iter()
        .map(|x| x.state)
        .collect();
    assert_eq!(states, ["revoked", "revoked"]);
}

#[test]
fn only_metadata_is_stored_never_the_arguments_or_environment_shown_in_the_review() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action(
        &e.ws[0],
        &["claude", "/opt/bin/claude", SECRET_ARG, "A=1"],
        &ALL,
    );
    g.decide(&request(&g, &a).id, Decision::Allow(Scope::Workspace))
        .unwrap();
    let list = g.list_active().unwrap();
    assert_eq!(list[0].summary, "Claude Code at /opt/bin/claude");
    // Look at every stored column of every row, not only the ones the API returns.
    let dump: String = {
        let c = e.db.c_for_test();
        let mut st = c.prepare("SELECT id||class||scope||fingerprint||summary||COALESCE(workspace_id,'') FROM approvals").unwrap();
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert!(
        !dump.contains(SECRET_ARG) && !dump.contains("A=1"),
        "{dump}"
    );
}

#[test]
fn history_is_capped_and_never_drops_an_active_grant() {
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let keep = action(&e.ws[0], &["keep"], &ALL);
    g.decide(&request(&g, &keep).id, Decision::Allow(Scope::Workspace))
        .unwrap();
    for i in 0..230 {
        let a = action(&e.ws[0], &["once", &i.to_string()], &ALL);
        g.decide(&request(&g, &a).id, Decision::Allow(Scope::Once))
            .unwrap();
        assert!(allowed(&g, &a));
    }
    assert_eq!(g.history(1000).unwrap().len(), 200);
    assert!(allowed(&g, &keep), "the standing grant survived pruning");
}

#[test]
fn an_action_with_no_workspace_can_still_be_approved_once_and_is_spent_once() {
    // For example removing a worktree whose workspace is gone: there is no workspace id to store.
    let e = env(1);
    let g = ApprovalGate::new(e.db.clone());
    let a = action("", &["orphan-worktree"], &[Scope::Once]);
    assert_eq!(a.workspace(), None);
    let r = request(&g, &a);
    g.decide(&r.id, Decision::Allow(Scope::Once)).unwrap();
    assert!(allowed(&g, &a));
    assert!(!allowed(&g, &a), "spent");
    // It must not authorize the same thing in a real workspace.
    let r2 = request(&g, &a);
    g.decide(&r2.id, Decision::Allow(Scope::Once)).unwrap();
    assert!(!allowed(
        &g,
        &action(&e.ws[0], &["orphan-worktree"], &[Scope::Once])
    ));
}
