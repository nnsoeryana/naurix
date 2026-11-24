use super::*;

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("at-test-{tag}-{}-{}", std::process::id(), new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn migrations_apply_and_are_idempotent() {
    let dir = tmpdir("mig");
    let path = dir.join("db.sqlite");
    drop(Db::open(&path).unwrap());
    let db = Db::open(&path).unwrap(); // reopen: no re-run, no error
    let v: i64 = db
        .c()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(v as usize, MIGRATIONS.len());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn refuses_a_newer_schema() {
    let dir = tmpdir("newer");
    let path = dir.join("db.sqlite");
    drop(Db::open(&path).unwrap());
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", 999)
        .unwrap();
    assert!(matches!(Db::open(&path), Err(DbError::Invalid(_))));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn workspace_lifecycle() {
    let db = Db::open_in_memory().unwrap();
    let dir = tmpdir("ws");
    let w = db.create_workspace(dir.to_str().unwrap()).unwrap();
    assert_eq!(
        w.root_path,
        std::fs::canonicalize(&dir).unwrap().to_string_lossy()
    );
    // idempotent, including via a symlinked/non-canonical spelling
    let again = db
        .create_workspace(&format!("{}/.", dir.display()))
        .unwrap();
    assert_eq!(again.id, w.id);
    assert_eq!(db.list_workspaces().unwrap().len(), 1);

    let r = db.rename_workspace(&w.id, "  My Project ").unwrap();
    assert_eq!(r.name, "My Project");
    assert!(db.rename_workspace(&w.id, "  ").is_err());
    assert!(matches!(
        db.rename_workspace("nope", "x"),
        Err(DbError::NotFound)
    ));

    db.remove_workspace(&w.id).unwrap();
    assert!(db.list_workspaces().unwrap().is_empty());
    assert!(
        dir.exists(),
        "removing a workspace must not delete project files"
    );
    assert!(matches!(db.remove_workspace(&w.id), Err(DbError::NotFound)));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_bad_workspace_paths() {
    let db = Db::open_in_memory().unwrap();
    for bad in [
        "",
        "relative/dir",
        "/definitely/not/here",
        "/etc/hosts",
        "/tmp/\0x",
    ] {
        assert!(
            matches!(db.create_workspace(bad), Err(DbError::Invalid(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn workspaces_persist_across_reopen() {
    let dir = tmpdir("persist");
    let proj = tmpdir("proj");
    let path = dir.join("db.sqlite");
    let id = Db::open(&path)
        .unwrap()
        .create_workspace(proj.to_str().unwrap())
        .unwrap()
        .id;
    let list = Db::open(&path).unwrap().list_workspaces().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(proj).unwrap();
}

#[test]
fn restart_marks_active_sessions_interrupted() {
    let db = Db::open_in_memory().unwrap();
    let proj = tmpdir("sess");
    let w = db.create_workspace(proj.to_str().unwrap()).unwrap();
    let a = db
        .insert_session(&w.id, SessionKind::Terminal, "one", None, None)
        .unwrap();
    let b = db
        .insert_session(&w.id, SessionKind::Terminal, "two", None, None)
        .unwrap();
    db.finish_session(&b, "exited").unwrap();

    assert_eq!(db.mark_active_sessions_interrupted().unwrap(), 1);
    let s = db.recent_sessions(&w.id, 10).unwrap();
    let by = |id: &str| s.iter().find(|r| r.id == id).unwrap().clone();
    assert_eq!(by(&a).lifecycle, "interrupted");
    assert!(by(&a).ended_at.is_some());
    assert_eq!(by(&b).lifecycle, "exited");
    assert_eq!(db.mark_active_sessions_interrupted().unwrap(), 0);
    assert!(db.finish_session(&a, "bogus").is_err());
    std::fs::remove_dir_all(proj).unwrap();
}

#[test]
fn removing_a_workspace_removes_its_sessions() {
    let db = Db::open_in_memory().unwrap();
    let proj = tmpdir("cascade");
    let w = db.create_workspace(proj.to_str().unwrap()).unwrap();
    db.insert_session(&w.id, SessionKind::Terminal, "t", None, None)
        .unwrap();
    db.remove_workspace(&w.id).unwrap();
    let n: i64 = db
        .c()
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
    std::fs::remove_dir_all(proj).unwrap();
}

#[test]
fn prune_keeps_newest_per_workspace() {
    let db = Db::open_in_memory().unwrap();
    let proj = tmpdir("prune");
    let w = db.create_workspace(proj.to_str().unwrap()).unwrap();
    for i in 0..5 {
        db.insert_session(&w.id, SessionKind::Terminal, &format!("s{i}"), None, None)
            .unwrap();
    }
    assert_eq!(db.prune_sessions(2).unwrap(), 3);
    let titles: Vec<_> = db
        .recent_sessions(&w.id, 10)
        .unwrap()
        .into_iter()
        .map(|s| s.title)
        .collect();
    assert_eq!(titles, ["s4", "s3"]);
    std::fs::remove_dir_all(proj).unwrap();
}

#[test]
fn schema_has_no_place_for_terminal_content() {
    // DATA-003 guard: fail if someone adds a column that could hold terminal data.
    let db = Db::open_in_memory().unwrap();
    let c = db.c();
    for table in [
        "workspaces",
        "sessions",
        "settings",
        "mcp_servers",
        "mcp_sync_entries",
        "mcp_sync_runs",
        "skills",
        "skill_links",
    ] {
        let mut st = c.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        let cols: Vec<String> = st
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for col in cols {
            // `mcp_servers.env_json` stores environment *templates* whose secrets are `${secret:NAME}`
            // placeholders (validated in `mcp::model`); the values live only in the Keychain.
            let allowed = table == "mcp_servers" && col == "env_json";
            for banned in [
                "output",
                "input",
                "transcript",
                "prompt",
                "env",
                "content",
                "scrollback",
                "secret_value",
                "password",
                "token",
            ] {
                assert!(
                    allowed || !col.contains(banned),
                    "{table}.{col} looks like it could hold sensitive content"
                );
            }
        }
    }
}

#[test]
fn legacy_data_dir_is_migrated_once_with_its_contents() {
    let base = tmpdir("legacy");
    let (legacy, current) = (
        base.join("dev.agentterminal.app"),
        base.join("dev.naurix.app"),
    );
    std::fs::create_dir_all(&legacy).unwrap();

    // A real database written under the old file name must open under the new one with its data.
    let proj = tmpdir("legacy-proj");
    {
        let db = Db::open(&legacy.join("agent-terminal.db")).unwrap();
        db.create_workspace(proj.to_str().unwrap()).unwrap();
    }
    assert!(migrate_legacy_data_dir(&legacy, &current).unwrap());
    assert!(!legacy.exists());
    assert!(!current.join("agent-terminal.db").exists());
    let db = Db::open(&current.join("naurix.db")).unwrap();
    assert_eq!(db.list_workspaces().unwrap().len(), 1);

    // Idempotent: nothing to do the second time, and an existing new folder is never overwritten.
    assert!(!migrate_legacy_data_dir(&legacy, &current).unwrap());
    std::fs::create_dir_all(&legacy).unwrap();
    assert!(!migrate_legacy_data_dir(&legacy, &current).unwrap());
    assert!(
        legacy.exists(),
        "legacy folder untouched when the new one exists"
    );
    std::fs::remove_dir_all(base).unwrap();
    std::fs::remove_dir_all(proj).unwrap();
}
