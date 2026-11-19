#![cfg(unix)] // Unix mock programs and symlinks; Windows coverage lives in the *_windows.rs tests (ADR 0021).
//! MCP registry, secrets, approval, Test Connection and agent-config sync.
use naurix_core::agents::search_dirs;
use naurix_core::mcp::model::{McpInput, McpServer, Pair, Transport};
use naurix_core::mcp::probe::probe;
use naurix_core::mcp::sync::{self, Action};
use naurix_core::mcp::{McpError, McpStore, MemoryStore};
use naurix_core::persistence::{new_id, Db};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("at-mcp-{tag}-{}-{}", std::process::id(), new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pair(n: &str, v: &str) -> Pair {
    Pair {
        name: n.into(),
        value: v.into(),
    }
}

fn stdio(name: &str, command: &str, args: &[&str]) -> McpInput {
    McpInput {
        id: None,
        name: name.into(),
        transport: Transport::Stdio,
        command: Some(command.into()),
        args: args.iter().map(|s| s.to_string()).collect(),
        url: None,
        env: vec![],
        headers: vec![],
        secrets: vec![],
    }
}

fn http(name: &str) -> McpInput {
    McpInput {
        id: None,
        name: name.into(),
        transport: Transport::Http,
        command: None,
        args: vec![],
        url: Some("https://api.example.com/mcp".into()),
        env: vec![],
        headers: vec![pair("Authorization", "Bearer ${secret:TOKEN}")],
        secrets: vec!["TOKEN".into()],
    }
}

fn approved(st: &McpStore<'_>, i: McpInput) -> McpServer {
    let s = st.save(i).unwrap();
    st.approve(&s.id, &s.fingerprint()).unwrap()
}

// ---------- registry, approval, secrets ----------

#[test]
fn registry_crud_names_and_approval_lifecycle() {
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let s = st.save(stdio("fs", "npx", &["-y", "pkg"])).unwrap();
    assert!(
        s.enabled && !s.is_approved(),
        "new servers start enabled but NOT approved"
    );
    assert!(
        matches!(st.save(stdio("fs", "npx", &[])), Err(McpError::Invalid(_))),
        "duplicate name"
    );
    assert!(st.save(stdio("bad name", "npx", &[])).is_err());

    // Approval is bound to the fingerprint the user reviewed.
    assert!(matches!(
        st.approve(&s.id, "deadbeef"),
        Err(McpError::ApprovalStale)
    ));
    let a = st.approve(&s.id, &s.fingerprint()).unwrap();
    assert!(a.is_approved() && a.approved_at.is_some());

    // Editing what runs invalidates approval; renaming does not.
    let mut edit = a.as_input();
    edit.name = "files".into();
    assert!(
        st.save(edit.clone()).unwrap().is_approved(),
        "name is not part of the fingerprint"
    );
    edit.args.push("--extra".into());
    assert!(
        !st.save(edit).unwrap().is_approved(),
        "changed args need a new approval"
    );

    st.set_enabled(&s.id, false).unwrap();
    assert!(!st.get(&s.id).unwrap().enabled);
    st.delete(&s.id).unwrap();
    assert!(matches!(st.get(&s.id), Err(McpError::NotFound)));
    assert!(st.list().unwrap().is_empty());
}

#[test]
fn secret_values_live_only_in_the_secret_store_never_in_sqlite() {
    let dir = tmp("nosecret");
    let db_path = dir.join("m.db");
    let ss = MemoryStore::default();
    const VALUE: &str = "ghp_SuperSecretValue_123456";
    {
        let db = Db::open(&db_path).unwrap();
        let st = McpStore::new(&db, &ss);
        let s = st.save(http("gh")).unwrap();
        assert!(
            st.set_secret(&s.id, "NOPE", "x").is_err(),
            "undeclared secret"
        );
        assert!(st.set_secret(&s.id, "TOKEN", "").is_err());
        st.set_secret(&s.id, "TOKEN", VALUE).unwrap();
        assert_eq!(
            st.secret_status(&s).unwrap(),
            vec![("TOKEN".to_string(), true)]
        );
        assert_eq!(
            st.resolve(&s, "Bearer ${secret:TOKEN}").unwrap(),
            format!("Bearer {VALUE}")
        );
        st.clear_secret(&s.id, "TOKEN").unwrap();
        assert!(
            matches!(st.resolve(&s, "${secret:TOKEN}"), Err(McpError::Invalid(_))),
            "no value -> clear error, no empty token"
        );
        st.set_secret(&s.id, "TOKEN", VALUE).unwrap();
    }
    // Scan every byte SQLite wrote (main file, WAL, shm).
    for f in std::fs::read_dir(&dir).unwrap().flatten() {
        let bytes = std::fs::read(f.path()).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(VALUE),
            "secret value found in {}",
            f.path().display()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn removing_a_secret_declaration_or_the_server_clears_the_keychain_item() {
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let s = st.save(http("gh")).unwrap();
    st.set_secret(&s.id, "TOKEN", "v").unwrap();
    let acct = format!("{}:TOKEN", s.id);
    assert!(naurix_core::mcp::SecretStore::get(&ss, &acct)
        .unwrap()
        .is_some());
    let mut edit = s.as_input();
    edit.headers.clear();
    edit.secrets.clear();
    edit.transport = Transport::Http;
    st.save(edit).unwrap();
    assert!(
        naurix_core::mcp::SecretStore::get(&ss, &acct)
            .unwrap()
            .is_none(),
        "orphaned secret removed"
    );

    let s2 = st.save(http("other")).unwrap();
    st.set_secret(&s2.id, "TOKEN", "v2").unwrap();
    st.delete(&s2.id).unwrap();
    assert!(
        naurix_core::mcp::SecretStore::get(&ss, &format!("{}:TOKEN", s2.id))
            .unwrap()
            .is_none()
    );
}

#[test]
fn launch_environment_only_for_enabled_approved_servers() {
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let ok = approved(&st, http("gh"));
    st.set_secret(&ok.id, "TOKEN", "tok").unwrap();
    let unapproved = st.save(http("pending")).unwrap();
    st.set_secret(&unapproved.id, "TOKEN", "nope").unwrap();
    let off = approved(&st, http("off"));
    st.set_secret(&off.id, "TOKEN", "nope2").unwrap();
    st.set_enabled(&off.id, false).unwrap();
    assert_eq!(
        st.launch_environment().unwrap(),
        vec![("NAURIX_MCP_GH_TOKEN".to_string(), "tok".to_string())]
    );
}

// ---------- Test Connection (mock MCP servers) ----------

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

const GOOD: &str = r#"#!/usr/bin/env python3
import sys, json, os, subprocess
child = subprocess.Popen(["sleep", "300"]); open(os.environ.get("PIDFILE", "/dev/null"), "w").write(str(child.pid)) if os.environ.get("PIDFILE") else None
for line in sys.stdin:
    m = json.loads(line)
    if m.get("method") == "initialize":
        info = {"name": "mock", "version": os.environ.get("TOK", "") + "|" + (sys.argv[1] if len(sys.argv) > 1 else "")}
        print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": info}}), flush=True)
    elif m.get("method") == "tools/list":
        if not m.get("params", {}).get("cursor"):
            print(json.dumps({"jsonrpc": "2.0", "method": "notifications/message", "params": {}}), flush=True)
            print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": {"tools": [{"name": "read", "description": "Read a thing"}], "nextCursor": "p2"}}), flush=True)
        else:
            print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": {"tools": [{"name": "write"}]}}), flush=True)
"#;

fn dirs() -> Vec<PathBuf> {
    search_dirs(
        std::env::var_os("PATH").as_deref(),
        Path::new("/nonexistent-home"),
    )
}

#[test]
fn probe_connects_lists_paged_tools_passes_secrets_and_kills_the_whole_process_group() {
    let d = tmp("probe");
    let exe = script(&d, "good.py", GOOD);
    let pidfile = d.join("child.pid");
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let mut i = stdio("mock", exe.to_str().unwrap(), &["${secret:DB}"]);
    i.env = vec![
        pair("TOK", "${secret:TOK}"),
        pair("PIDFILE", pidfile.to_str().unwrap()),
    ];
    i.secrets = vec!["DB".into(), "TOK".into()];
    let s = st.save(i).unwrap();
    // Not approved yet: refuse to run anything.
    assert!(matches!(
        probe(&st, &s, &dirs(), Duration::from_secs(5)),
        Err(McpError::NotApproved)
    ));
    let s = st.approve(&s.id, &s.fingerprint()).unwrap();
    // Approved but a secret has no value: clear error, nothing started.
    assert!(matches!(
        probe(&st, &s, &dirs(), Duration::from_secs(5)),
        Err(McpError::Invalid(_))
    ));
    st.set_secret(&s.id, "DB", "arg-secret").unwrap();
    st.set_secret(&s.id, "TOK", "env-secret").unwrap();

    let r = probe(&st, &s, &dirs(), Duration::from_secs(10)).unwrap();
    assert!(r.ok, "{:?}", r.error);
    assert_eq!(r.server_name.as_deref(), Some("mock"));
    assert_eq!(
        r.server_version.as_deref(),
        Some("env-secret|arg-secret"),
        "env and args were resolved from the secret store"
    );
    assert_eq!(r.protocol.as_deref(), Some("2025-06-18"));
    assert_eq!(
        r.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["read", "write"],
        "followed nextCursor"
    );
    assert_eq!(r.tools[0].description.as_deref(), Some("Read a thing"));

    let child: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(3);
    while unsafe { libc::kill(child, 0) } == 0 && std::time::Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_ne!(
        unsafe { libc::kill(child, 0) },
        0,
        "the server's child process survived the probe"
    );
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn probe_reports_clear_failures() {
    let d = tmp("probefail");
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let run = |name: &str, body: &str, secs: u64| {
        let exe = script(&d, &format!("{name}.py"), body);
        let s = approved(&st, stdio(name, exe.to_str().unwrap(), &[]));
        probe(&st, &s, &dirs(), Duration::from_secs(secs)).unwrap()
    };
    let crash = run("crash", "#!/bin/sh\nexit 3\n", 5);
    assert!(
        !crash.ok && crash.error.as_deref().unwrap().contains("exited"),
        "{:?}",
        crash.error
    );
    let silent = run("silent", "#!/bin/sh\nsleep 30\n", 1);
    assert!(
        !silent.ok && silent.error.as_deref().unwrap().contains("in time"),
        "{:?}",
        silent.error
    );
    let rejects = run("rejects", "#!/usr/bin/env python3\nimport sys,json\nfor l in sys.stdin:\n    m=json.loads(l)\n    print(json.dumps({'jsonrpc':'2.0','id':m['id'],'error':{'code':-32602,'message':'bad version'}}),flush=True)\n", 5);
    assert!(!rejects.ok && rejects.error.as_deref().unwrap().contains("bad version"));
    let missing = approved(&st, stdio("ghost", "definitely-not-installed-xyz", &[]));
    let r = probe(&st, &missing, &dirs(), Duration::from_secs(2)).unwrap();
    assert!(!r.ok && r.error.unwrap().contains("not found"));
    // Remote servers: honest "not testable", and nothing is executed.
    let remote = approved(&st, http("remote"));
    let r = probe(&st, &remote, &dirs(), Duration::from_secs(2)).unwrap();
    assert_eq!(r.status, "not_testable");
    std::fs::remove_dir_all(d).unwrap();
}

// ---------- sync ----------

struct Env {
    db: Db,
    ss: MemoryStore,
    ws: PathBuf,
    home: PathBuf,
    backups: PathBuf,
}

fn env(tag: &str) -> Env {
    let base = tmp(tag);
    let (ws, home, backups) = (base.join("ws"), base.join("home"), base.join("backups"));
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    Env {
        db: Db::open_in_memory().unwrap(),
        ss: MemoryStore::default(),
        ws,
        home,
        backups,
    }
}

impl Env {
    fn st(&self) -> McpStore<'_> {
        McpStore::new(&self.db, &self.ss)
    }
    fn plan(&self, agent: &str) -> sync::SyncPlan {
        sync::plan(&self.st(), agent, &self.ws, &self.home).unwrap()
    }
    fn apply(&self, agent: &str) -> Result<sync::SyncRun, McpError> {
        let p = self.plan(agent);
        sync::apply(
            &self.st(),
            agent,
            &self.ws,
            &self.home,
            &p.plan_hash,
            &self.backups,
        )
    }
    fn mcp_json(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.ws.join(".mcp.json")).unwrap()).unwrap()
    }
}

#[test]
fn only_verified_agents_can_be_synced() {
    let e = env("verified");
    for agent in ["codex", "gemini-cli", "antigravity-cli", "nonexistent"] {
        assert!(
            sync::plan(&e.st(), agent, &e.ws, &e.home).is_err(),
            "{agent}"
        );
    }
    assert!(sync::plan(&e.st(), "claude-code", &e.ws, &e.home).is_ok());
    assert!(sync::plan(&e.st(), "opencode", &e.ws, &e.home).is_ok());
}

#[test]
fn claude_sync_adds_entries_in_the_verified_shape_with_secret_references_only() {
    let e = env("claude-add");
    let st = e.st();
    approved(
        &st,
        stdio(
            "fs",
            "npx",
            &["-y", "@modelcontextprotocol/server-filesystem", "/p"],
        ),
    );
    let mut gh = http("github");
    gh.headers = vec![pair("Authorization", "Bearer ${secret:TOKEN}")];
    let gh = approved(&st, gh);
    st.set_secret(&gh.id, "TOKEN", "ghp_REALVALUE").unwrap();
    let mut pg = stdio("pg", "npx", &["-y", "pkg", "${secret:DATABASE_URL}"]);
    pg.secrets = vec!["DATABASE_URL".into()];
    pg.env = vec![pair("MODE", "ro")];
    approved(&st, pg);

    let plan = e.plan("claude-code");
    assert!(plan.blocked.is_none() && plan.has_writes());
    assert!(plan.changes.iter().all(|c| c.action == Action::Add));
    assert!(
        !e.ws.join(".mcp.json").exists(),
        "planning must not write anything"
    );

    let run = e.apply("claude-code").unwrap();
    assert_eq!(run.summary, "3 added, 0 updated, 0 removed");
    let text = std::fs::read_to_string(e.ws.join(".mcp.json")).unwrap();
    assert!(
        !text.contains("ghp_REALVALUE"),
        "the secret value must never be written"
    );
    let v = e.mcp_json();
    let servers = &v["mcpServers"];
    // Same keys the real `claude mcp add` produced (tests/fixtures/agents/claude-2.1.322-project.mcp.json).
    assert_eq!(
        servers["fs"],
        serde_json::json!({"type": "stdio", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/p"]})
    );
    assert_eq!(
        servers["github"],
        serde_json::json!({"type": "http", "url": "https://api.example.com/mcp", "headers": {"Authorization": "Bearer ${NAURIX_MCP_GITHUB_TOKEN}"}})
    );
    assert_eq!(servers["pg"]["args"][2], "${NAURIX_MCP_PG_DATABASE_URL}");
    assert_eq!(servers["pg"]["env"]["MODE"], "ro");
    // Re-planning right after applying finds nothing to do.
    let again = e.plan("claude-code");
    assert!(again.changes.iter().all(|c| c.action == Action::Unchanged) && !again.has_writes());
}

#[test]
fn user_owned_entries_and_other_keys_are_preserved_and_conflicts_never_overwritten() {
    let e = env("preserve");
    let original = "{\n    \"someOtherKey\": {\"keep\": true},\n    \"mcpServers\": {\n        \"mine\": {\"type\": \"stdio\", \"command\": \"my-tool\", \"args\": []},\n        \"fs\": {\"type\": \"stdio\", \"command\": \"USER-VERSION\", \"args\": []}\n    }\n}\n";
    std::fs::write(e.ws.join(".mcp.json"), original).unwrap();
    let st = e.st();
    approved(&st, stdio("fs", "npx", &["-y", "pkg"])); // same name as the user's entry
    approved(&st, stdio("extra", "npx", &["-y", "other"]));
    let plan = e.plan("claude-code");
    let by = |k: &str| plan.changes.iter().find(|c| c.entry_key == k).unwrap();
    assert_eq!(by("fs").action, Action::Conflict);
    assert_eq!(by("extra").action, Action::Add);
    e.apply("claude-code").unwrap();
    let v = e.mcp_json();
    assert_eq!(v["someOtherKey"], serde_json::json!({"keep": true}));
    assert_eq!(v["mcpServers"]["mine"]["command"], "my-tool");
    assert_eq!(
        v["mcpServers"]["fs"]["command"], "USER-VERSION",
        "an entry we did not write is never overwritten"
    );
    assert_eq!(v["mcpServers"]["extra"]["command"], "npx");
    let keys: Vec<_> = v["mcpServers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        keys,
        ["mine", "fs", "extra"],
        "user's order preserved, ours appended"
    );
    // Only conflicts and nothing else to write -> apply refuses rather than rewriting.
    let only_conflict = env("conflict-only");
    std::fs::write(only_conflict.ws.join(".mcp.json"), original).unwrap();
    approved(&only_conflict.st(), stdio("fs", "npx", &[]));
    assert!(only_conflict.apply("claude-code").is_err());
    assert_eq!(
        std::fs::read_to_string(only_conflict.ws.join(".mcp.json")).unwrap(),
        original,
        "file untouched byte for byte"
    );
}

#[test]
fn update_remove_and_user_edits_are_handled_per_entry() {
    let e = env("lifecycle");
    let st = e.st();
    let a = approved(&st, stdio("a", "npx", &["v1"]));
    approved(&st, stdio("b", "npx", &["keep"]));
    e.apply("claude-code").unwrap();

    // Edit + re-approve `a`: Update. `b` unchanged.
    let mut ed = a.as_input();
    ed.args = vec!["v2".into()];
    let a2 = st.save(ed).unwrap();
    let plan = e.plan("claude-code");
    assert!(
        plan.changes
            .iter()
            .any(|c| c.entry_key == "a" && c.action == Action::Remove),
        "an edited server is unapproved, so its entry would be removed"
    );
    st.approve(&a2.id, &a2.fingerprint()).unwrap();
    let plan = e.plan("claude-code");
    assert_eq!(
        plan.changes
            .iter()
            .find(|c| c.entry_key == "a")
            .unwrap()
            .action,
        Action::Update
    );
    e.apply("claude-code").unwrap();
    assert_eq!(e.mcp_json()["mcpServers"]["a"]["args"][0], "v2");

    // The user edits our entry by hand: from then on we leave it alone, even if disabling.
    let mut v = e.mcp_json();
    v["mcpServers"]["a"]["args"] = serde_json::json!(["hand-edited"]);
    std::fs::write(
        e.ws.join(".mcp.json"),
        serde_json::to_string_pretty(&v).unwrap(),
    )
    .unwrap();
    st.set_enabled(&a2.id, false).unwrap();
    let plan = e.plan("claude-code");
    assert_eq!(
        plan.changes
            .iter()
            .find(|c| c.entry_key == "a")
            .unwrap()
            .action,
        Action::Conflict
    );

    // Disabling the unedited one removes just its entry.
    let b = st
        .list()
        .unwrap()
        .into_iter()
        .find(|s| s.name == "b")
        .unwrap();
    st.set_enabled(&b.id, false).unwrap();
    let plan = e.plan("claude-code");
    assert_eq!(
        plan.changes
            .iter()
            .find(|c| c.entry_key == "b")
            .unwrap()
            .action,
        Action::Remove
    );
    e.apply("claude-code").unwrap();
    let v = e.mcp_json();
    assert!(v["mcpServers"].get("b").is_none());
    assert_eq!(
        v["mcpServers"]["a"]["args"][0], "hand-edited",
        "the user's edit survived"
    );

    // Deleting a server also cleans up the entry we wrote (when unedited).
    let c = approved(&st, stdio("c", "npx", &[]));
    e.apply("claude-code").unwrap();
    st.delete(&c.id).unwrap();
    assert_eq!(
        e.plan("claude-code")
            .changes
            .iter()
            .find(|x| x.entry_key == "c")
            .unwrap()
            .action,
        Action::Remove
    );
}

#[test]
fn stale_plans_are_refused_and_rollback_restores_exact_bytes() {
    let e = env("rollback");
    let original =
        "{ \"mcpServers\" : { \"mine\" : { \"command\" :\"x\" } },\n\n  \"theme\":  \"dark\" }";
    std::fs::write(e.ws.join(".mcp.json"), original).unwrap();
    let st = e.st();
    approved(&st, stdio("fs", "npx", &[]));

    let stale = e.plan("claude-code");
    // The file changes after the user reviewed the plan.
    std::fs::write(e.ws.join(".mcp.json"), format!("{original} ")).unwrap();
    assert!(
        sync::apply(
            &st,
            "claude-code",
            &e.ws,
            &e.home,
            &stale.plan_hash,
            &e.backups
        )
        .is_err(),
        "stale plan"
    );
    std::fs::write(e.ws.join(".mcp.json"), original).unwrap();

    let run = e.apply("claude-code").unwrap();
    assert!(std::fs::read_to_string(e.ws.join(".mcp.json"))
        .unwrap()
        .contains("\"fs\""));
    assert_eq!(
        std::fs::read_dir(&e.backups).unwrap().count(),
        1,
        "one backup was taken"
    );
    // A second run to test stacking.
    approved(&st, stdio("second", "npx", &[]));
    let run2 = e.apply("claude-code").unwrap();
    assert!(
        sync::rollback(&st, &run.id).is_err(),
        "the newest run must be undone first"
    );
    sync::rollback(&st, &run2.id).unwrap();
    assert!(sync::rollback(&st, &run2.id).is_err(), "already undone");
    sync::rollback(&st, &run.id).unwrap();
    assert_eq!(
        std::fs::read_to_string(e.ws.join(".mcp.json")).unwrap(),
        original,
        "byte-for-byte restore"
    );
    assert!(sync::runs(&st, "claude-code", 10)
        .unwrap()
        .iter()
        .all(|r| r.rolled_back_at.is_some()));
    // After rolling back, the plan sees nothing as ours, so it would add again (consistent state).
    assert!(e
        .plan("claude-code")
        .changes
        .iter()
        .all(|c| c.action == Action::Add));
}

#[test]
fn rollback_refuses_when_the_user_changed_the_file_afterwards_and_removes_files_it_created() {
    let e = env("rollback-edit");
    let st = e.st();
    approved(&st, stdio("fs", "npx", &[]));
    let run = e.apply("claude-code").unwrap(); // file did not exist before
    let mut v = e.mcp_json();
    v["userAdded"] = serde_json::json!(1);
    std::fs::write(e.ws.join(".mcp.json"), serde_json::to_string(&v).unwrap()).unwrap();
    assert!(
        sync::rollback(&st, &run.id).is_err(),
        "would lose the user's later edit"
    );
    assert!(e.ws.join(".mcp.json").exists());
    // Without a later edit, undoing deletes the file we created.
    let e2 = env("rollback-create");
    approved(&e2.st(), stdio("fs", "npx", &[]));
    let run = e2.apply("claude-code").unwrap();
    assert!(e2.ws.join(".mcp.json").exists());
    sync::rollback(&e2.st(), &run.id).unwrap();
    assert!(!e2.ws.join(".mcp.json").exists());
}

#[test]
fn opencode_flat_shape_creation_references_and_blocked_cases() {
    // New file: flat mcp.<name>, local/remote types, {env:...} references.
    let e = env("oc-new");
    let st = e.st();
    let mut pg = stdio("pg", "npx", &["-y", "pkg", "${secret:DATABASE_URL}"]);
    pg.secrets = vec!["DATABASE_URL".into()];
    pg.env = vec![pair("MODE", "ro")];
    approved(&st, pg);
    approved(&st, http("gh"));
    e.apply("opencode").unwrap();
    let path = e.home.join(".config/opencode/opencode.jsonc");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["$schema"], "https://opencode.ai/config.json");
    assert_eq!(
        v["mcp"]["pg"],
        serde_json::json!({"type": "local", "command": ["npx", "-y", "pkg", "{env:NAURIX_MCP_PG_DATABASE_URL}"], "environment": {"MODE": "ro"}})
    );
    assert_eq!(
        v["mcp"]["gh"],
        serde_json::json!({"type": "remote", "url": "https://api.example.com/mcp", "headers": {"Authorization": "Bearer {env:NAURIX_MCP_GH_TOKEN}"}})
    );
    assert!(
        v["mcp"].get("servers").is_none(),
        "flat shape, not mcp.servers"
    );

    // Comments in the user's file: never rewritten.
    let e = env("oc-comments");
    std::fs::create_dir_all(e.home.join(".config/opencode")).unwrap();
    let commented = "{\n  // my settings\n  \"theme\": \"dark\",\n}\n";
    std::fs::write(e.home.join(".config/opencode/opencode.jsonc"), commented).unwrap();
    approved(&e.st(), stdio("fs", "npx", &[]));
    let p = e.plan("opencode");
    assert!(p.blocked.as_deref().unwrap().contains("comments") && p.preview.contains("\"fs\""));
    assert!(e.apply("opencode").is_err());
    assert_eq!(
        std::fs::read_to_string(e.home.join(".config/opencode/opencode.jsonc")).unwrap(),
        commented
    );

    // The newer mcp.servers layout is unverified: blocked.
    let e = env("oc-v2");
    std::fs::create_dir_all(e.home.join(".config/opencode")).unwrap();
    std::fs::write(
        e.home.join(".config/opencode/opencode.json"),
        r#"{"mcp":{"servers":{"x":{"type":"local","command":["a"]}}}}"#,
    )
    .unwrap();
    approved(&e.st(), stdio("fs", "npx", &[]));
    assert!(e.plan("opencode").blocked.unwrap().contains("mcp.servers"));
    assert!(
        e.plan("opencode").file.ends_with("opencode.json"),
        "an existing opencode.json is used instead of creating .jsonc"
    );
}

#[test]
fn rendered_entries_have_exactly_the_keys_the_real_clis_wrote() {
    // Fixture keys captured from Claude Code 2.1.322 / OpenCode 1.18.10 (see tests/fixtures/agents).
    let fx = |n: &str| -> serde_json::Value {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/agents")
                    .join(n),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    let claude = fx("claude-2.1.322-project.mcp.json");
    let oc = fx("opencode-1.18.10-global.opencode.jsonc");
    let e = env("shapes");
    let st = e.st();
    let mut s = stdio("demo", "echo", &["hello"]);
    s.env = vec![pair("DEMO_MODE", "abc")];
    approved(&st, s);
    let mut w = http("web");
    w.headers = vec![pair("X-Demo", "1")];
    w.secrets.clear();
    approved(&st, w);
    e.apply("claude-code").unwrap();
    e.apply("opencode").unwrap();
    let ours = e.mcp_json();
    assert_eq!(
        keys(&ours["mcpServers"]["demo"]),
        keys(&claude["mcpServers"]["demo"])
    );
    assert_eq!(
        keys(&ours["mcpServers"]["web"]),
        keys(&claude["mcpServers"]["web"])
    );
    let ocv: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(e.home.join(".config/opencode/opencode.jsonc")).unwrap(),
    )
    .unwrap();
    assert_eq!(keys(&ocv["mcp"]["demo"]), keys(&oc["mcp"]["demo"]));
}

#[test]
fn revoking_all_approvals_makes_every_server_inert_again() {
    let (db, ss) = (Db::open_in_memory().unwrap(), MemoryStore::default());
    let st = McpStore::new(&db, &ss);
    let a = approved(&st, stdio("a", "npx", &[]));
    let b = approved(&st, stdio("b", "npx", &["x"]));
    st.save(stdio("never", "npx", &["y"])).unwrap();
    assert_eq!(st.revoke_all_approvals().unwrap(), 2);
    assert!(!st.get(&a.id).unwrap().is_approved() && !st.get(&b.id).unwrap().is_approved());
    assert_eq!(st.revoke_all_approvals().unwrap(), 0);
    assert!(matches!(
        probe(
            &st,
            &st.get(&a.id).unwrap(),
            &dirs(),
            Duration::from_secs(2)
        ),
        Err(McpError::NotApproved)
    ));
}
