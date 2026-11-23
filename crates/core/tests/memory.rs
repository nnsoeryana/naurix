//! Opt-in project memory (ADR 0031): real SQLite with FTS5, including the cascade on workspace removal.
use naurix_core::memory::{ExportFormat, MemoryError, MemoryStore, MAX_NOTES};
use naurix_core::persistence::Db;

/// Counts rows through a second connection to the same file, so the tests look at the real index.
trait RawCount {
    fn raw_count(&self, sql: &str) -> i64;
}
struct Probe(std::path::PathBuf);
impl RawCount for Probe {
    fn raw_count(&self, sql: &str) -> i64 {
        rusqlite::Connection::open(&self.0)
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
}

fn setup() -> (Db, String) {
    let file = std::env::temp_dir().join(format!(
        "nx-mem-db-{}-{}.db",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    let db = Db::open(&file).unwrap();
    PROBE.with(|p| *p.borrow_mut() = Some(Probe(file)));
    let dir = std::env::temp_dir().join(format!(
        "nx-mem-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ws = db.create_workspace(dir.to_str().unwrap()).unwrap().id;
    (db, ws)
}

thread_local! {
    static PROBE: std::cell::RefCell<Option<Probe>> = const { std::cell::RefCell::new(None) };
}

fn raw(sql: &str) -> i64 {
    PROBE.with(|p| p.borrow().as_ref().unwrap().raw_count(sql))
}

fn titles(v: &[naurix_core::memory::Note]) -> Vec<&str> {
    v.iter().map(|n| n.title.as_str()).collect()
}

#[test]
fn memory_is_off_by_default_and_per_workspace() {
    let (db, ws) = setup();
    let other = db
        .create_workspace(std::env::temp_dir().to_str().unwrap())
        .unwrap()
        .id;
    let m = MemoryStore::new(&db);
    assert!(!m.status(&ws).unwrap().enabled);
    assert!(matches!(
        m.save(&ws, None, "t", "b"),
        Err(MemoryError::Disabled)
    ));
    assert!(matches!(m.list(&ws, None), Err(MemoryError::Disabled)));
    m.set_enabled(&ws, true).unwrap();
    m.save(&ws, None, "t", "b").unwrap();
    assert!(
        !m.status(&other).unwrap().enabled,
        "enabling one workspace does not enable another"
    );
    assert!(matches!(m.list(&other, None), Err(MemoryError::Disabled)));
}

#[test]
fn fts5_search_ranks_matches_prefixes_and_survives_hostile_queries() {
    let (db, ws) = setup();
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    m.save(
        &ws,
        None,
        "Database choice",
        "We use SQLite with FTS5 for search.",
    )
    .unwrap();
    m.save(&ws, None, "Naming", "Crates are named naurix-*.")
        .unwrap();
    m.save(&ws, None, "Café notes", "Résumé of the meeting.")
        .unwrap();
    assert_eq!(
        titles(&m.list(&ws, Some("sqlite")).unwrap()),
        ["Database choice"]
    );
    assert_eq!(
        titles(&m.list(&ws, Some("fts")).unwrap()),
        ["Database choice"],
        "prefix match"
    );
    assert_eq!(
        titles(&m.list(&ws, Some("cafe resume")).unwrap()),
        ["Café notes"],
        "diacritics are folded"
    );
    assert!(m.list(&ws, Some("nothing-like-this")).unwrap().is_empty());
    for hostile in [
        "\"",
        "\"unbalanced",
        "AND",
        "NEAR(",
        ")(",
        "*",
        "title:sqlite",
        "a OR b",
        "-sqlite",
        "^x",
    ] {
        m.list(&ws, Some(hostile))
            .unwrap_or_else(|e| panic!("{hostile:?} broke search: {e}"));
    }
    assert_eq!(
        m.list(&ws, Some("  ")).unwrap().len(),
        3,
        "an empty query lists everything"
    );
}

fn fts_rows(_db: &Db, word: &str) -> i64 {
    // Probe the index directly, so "deleted" means "no longer searchable" and not just "row gone".
    raw(&format!(
        "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH '\"{word}\"'"
    ))
}

#[test]
fn deleting_a_note_deleting_all_and_removing_the_workspace_each_clear_the_index() {
    let (db, ws) = setup();
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    let a = m.save(&ws, None, "alpha", "zebrafish unique one").unwrap();
    m.save(&ws, None, "beta", "zebrafish unique two").unwrap();
    assert_eq!(fts_rows(&db, "zebrafish"), 2);
    // Editing replaces the indexed text.
    m.save(&ws, Some(&a.id), "alpha", "walrus now").unwrap();
    assert_eq!(
        (fts_rows(&db, "zebrafish"), fts_rows(&db, "walrus")),
        (1, 1)
    );
    m.delete(&ws, &a.id).unwrap();
    assert_eq!(fts_rows(&db, "walrus"), 0);
    assert_eq!(m.delete_all(&ws).unwrap(), 1);
    assert_eq!(fts_rows(&db, "zebrafish"), 0);

    // Removing the workspace cascades to its notes, and the index follows.
    m.save(&ws, None, "gamma", "narwhal text").unwrap();
    assert_eq!(fts_rows(&db, "narwhal"), 1);
    db.remove_workspace(&ws).unwrap();
    assert_eq!(fts_rows(&db, "narwhal"), 0);
    assert_eq!(raw("SELECT COUNT(*) FROM memory_notes"), 0);
}

#[test]
fn turning_memory_off_hides_notes_without_deleting_them_and_delete_still_works() {
    let (db, ws) = setup();
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    let n = m.save(&ws, None, "kept", "body").unwrap();
    m.set_enabled(&ws, false).unwrap();
    assert!(matches!(m.list(&ws, None), Err(MemoryError::Disabled)));
    assert!(matches!(m.get(&ws, &n.id), Err(MemoryError::Disabled)));
    assert!(matches!(
        m.export(&ws, ExportFormat::Json),
        Err(MemoryError::Disabled)
    ));
    let s = m.status(&ws).unwrap();
    assert_eq!(
        (s.enabled, s.count),
        (false, 1),
        "the user can see that notes are kept"
    );
    m.set_enabled(&ws, true).unwrap();
    assert_eq!(titles(&m.list(&ws, None).unwrap()), ["kept"]);
    m.set_enabled(&ws, false).unwrap();
    assert_eq!(m.delete_all(&ws).unwrap(), 1, "delete all works while off");
}

#[test]
fn secrets_are_refused_on_create_and_edit_and_never_stored() {
    let (db, ws) = setup();
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    let e = m
        .save(&ws, None, "deploy", "ok\nAKIAIOSFODNN7EXAMPLE")
        .unwrap_err();
    assert!(matches!(
        e,
        MemoryError::Secret {
            line: 2,
            kind: "an AWS access key"
        }
    ));
    assert!(!e.to_string().contains("AKIAIOSFODNN7EXAMPLE"));
    let ok = m.save(&ws, None, "fine", "nothing secret").unwrap();
    assert!(matches!(
        m.save(&ws, Some(&ok.id), "fine", "token = Xk29fjA0sd9Qw8e7r6"),
        Err(MemoryError::Secret { .. })
    ));
    assert_eq!(
        m.get(&ws, &ok.id).unwrap().body,
        "nothing secret",
        "a refused edit changes nothing"
    );
    assert_eq!(m.status(&ws).unwrap().count, 1);
    assert!(
        matches!(
            m.save(&ws, None, "AKIAIOSFODNN7EXAMPLE", "x"),
            Err(MemoryError::Secret { line: 1, .. })
        ),
        "titles are scanned too"
    );
    assert_eq!(
        raw("SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'AKIAIOSFODNN7EXAMPLE'"),
        0
    );
}

#[test]
fn a_workspace_holds_at_most_the_limit_and_notes_stay_in_their_workspace() {
    let (db, ws) = setup();
    let other = db
        .create_workspace(std::env::temp_dir().to_str().unwrap())
        .unwrap()
        .id;
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    m.set_enabled(&other, true).unwrap();
    for i in 0..MAX_NOTES {
        m.save(&ws, None, &format!("n{i}"), "b").unwrap();
    }
    assert!(matches!(
        m.save(&ws, None, "one too many", "b"),
        Err(MemoryError::Full)
    ));
    m.save(&other, None, "elsewhere", "b").unwrap();
    let first = m.list(&ws, None).unwrap()[0].id.clone();
    assert!(matches!(m.get(&other, &first), Err(MemoryError::NotFound)));
    assert!(matches!(
        m.delete(&other, &first),
        Err(MemoryError::NotFound)
    ));
    assert!(matches!(
        m.save(&other, Some(&first), "x", "y"),
        Err(MemoryError::NotFound)
    ));
    assert_eq!(
        m.list(&other, Some("n1")).unwrap().len(),
        0,
        "search never crosses workspaces"
    );
}

#[test]
fn exports_hold_only_the_notes_text_and_dates() {
    let (db, ws) = setup();
    let m = MemoryStore::new(&db);
    m.set_enabled(&ws, true).unwrap();
    m.save(&ws, None, "First", "alpha body").unwrap();
    m.save(&ws, None, "Second", "beta body").unwrap();
    let md = m.export(&ws, ExportFormat::Markdown).unwrap();
    assert!(
        md.starts_with("# Memory: ")
            && md.contains("## First")
            && md.contains("alpha body")
            && md.contains("2 notes")
    );
    assert!(
        md.find("## First").unwrap() < md.find("## Second").unwrap(),
        "oldest first"
    );
    let json: serde_json::Value =
        serde_json::from_str(&m.export(&ws, ExportFormat::Json).unwrap()).unwrap();
    assert_eq!(json["notes"].as_array().unwrap().len(), 2);
    assert_eq!(json["notes"][0]["title"], "First");
    let keys: Vec<_> = json["notes"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        keys,
        ["title", "body", "createdAt", "updatedAt"],
        "no ids and no paths"
    );
    assert!(
        !json.to_string().contains(&ws),
        "the workspace id and path are not exported"
    );
    assert!(!md.contains(&ws));
}
