//! SQLite metadata store (M4). Metadata only: no terminal input/output, prompts, environment
//! values or file contents are ever written here (DATA-003).

mod time;
mod workspace;

pub use time::{iso8601_utc_in, iso8601_utc_now};
pub use workspace::{validate_name, validate_root};

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("{0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Terminal,
    Agent,
}

impl SessionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub workspace_id: String,
    pub kind: String,
    pub title: String,
    pub adapter_id: Option<String>,
    pub lifecycle: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

/// Ordered migrations. Never edit an existing entry; append a new one.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE workspaces (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       root_path TEXT NOT NULL UNIQUE,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL,
       last_opened_at TEXT
     );
     CREATE TABLE sessions (
       id TEXT PRIMARY KEY,
       workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
       kind TEXT NOT NULL,
       title TEXT NOT NULL,
       adapter_id TEXT,
       lifecycle TEXT NOT NULL,
       started_at TEXT,
       ended_at TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE INDEX sessions_by_workspace ON sessions(workspace_id, created_at);
     CREATE TABLE settings (
       key TEXT PRIMARY KEY,
       value_json TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );",
    // 2: MCP registry (ADR 0015). Secret VALUES are never stored here, only their names.
    "CREATE TABLE mcp_servers (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL UNIQUE,
       transport TEXT NOT NULL,
       command TEXT,
       args_json TEXT NOT NULL,
       url TEXT,
       env_json TEXT NOT NULL,
       headers_json TEXT NOT NULL,
       secrets_json TEXT NOT NULL,
       enabled INTEGER NOT NULL,
       approved_fingerprint TEXT,
       approved_at TEXT,
       last_status TEXT,
       last_error TEXT,
       last_tool_count INTEGER,
       last_tested_at TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE mcp_sync_entries (
       server_id TEXT NOT NULL,
       agent_id TEXT NOT NULL,
       config_path TEXT NOT NULL,
       entry_key TEXT NOT NULL,
       entry_hash TEXT NOT NULL,
       created_at TEXT NOT NULL,
       PRIMARY KEY (agent_id, config_path, entry_key)
     );
     CREATE TABLE mcp_sync_runs (
       id TEXT PRIMARY KEY,
       agent_id TEXT NOT NULL,
       config_path TEXT NOT NULL,
       backup_path TEXT,
       before_exists INTEGER NOT NULL,
       before_hash TEXT,
       after_hash TEXT NOT NULL,
       entries_before_json TEXT NOT NULL,
       summary TEXT NOT NULL,
       created_at TEXT NOT NULL,
       rolled_back_at TEXT
     );",
    // 3: skills registry (ADR 0016). Content lives on disk in the app data folder, never in SQLite.
    "CREATE TABLE skills (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       slug TEXT NOT NULL UNIQUE,
       description TEXT,
       source_kind TEXT NOT NULL,
       source_url TEXT NOT NULL,
       source_subpath TEXT NOT NULL,
       revision TEXT NOT NULL,
       tree_hash TEXT NOT NULL,
       rev_dir TEXT NOT NULL,
       prev_rev_dir TEXT,
       prev_tree_hash TEXT,
       prev_revision TEXT,
       flags_json TEXT NOT NULL,
       installed_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE skill_links (
       skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
       agent_id TEXT NOT NULL,
       scope TEXT NOT NULL,
       link_path TEXT NOT NULL,
       created_dir INTEGER NOT NULL,
       created_at TEXT NOT NULL,
       PRIMARY KEY (skill_id, link_path)
     );",
    // 4: approval gate (ADR 0027). Metadata only: the summary holds an agent name and program path, never
    // arguments, environment values, prompts or output. App-scoped grants have no workspace and outlive it.
    "CREATE TABLE approvals (
       id TEXT PRIMARY KEY,
       class TEXT NOT NULL,
       scope TEXT NOT NULL,
       scope_ref TEXT,
       fingerprint TEXT NOT NULL,
       summary TEXT NOT NULL,
       workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
       state TEXT NOT NULL,
       created_at TEXT NOT NULL,
       last_used_at TEXT,
       expires_at TEXT,
       ended_at TEXT
     );
     CREATE INDEX approvals_lookup ON approvals(class, fingerprint, state);
     ALTER TABLE sessions ADD COLUMN approval_id TEXT;",
    // 5: worktrees Naurix created for agents (ADR 0028). Metadata only. Removing a workspace never deletes
    // project files, so it does not delete worktree folders either: the row stays with the workspace cleared.
    "CREATE TABLE worktrees (
       id TEXT PRIMARY KEY,
       workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
       repo_root TEXT NOT NULL,
       path TEXT NOT NULL UNIQUE,
       branch TEXT NOT NULL,
       base_commit TEXT NOT NULL,
       created_at TEXT NOT NULL
     );
     ALTER TABLE sessions ADD COLUMN worktree_id TEXT;",
    // 6: workflows and runs (ADR 0030). Metadata only: ids, user-typed names, edges, states, times and a
    // short fixed-vocabulary note. No prompts, arguments, paths or output. A run is a snapshot of its
    // workflow; deleting the workflow keeps the run history.
    "CREATE TABLE workflows (
       id TEXT PRIMARY KEY,
       workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
       name TEXT NOT NULL,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE workflow_steps (
       id TEXT PRIMARY KEY,
       workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
       position INTEGER NOT NULL,
       name TEXT NOT NULL,
       agent_id TEXT NOT NULL
     );
     CREATE TABLE workflow_deps (
       step_id TEXT NOT NULL REFERENCES workflow_steps(id) ON DELETE CASCADE,
       depends_on TEXT NOT NULL REFERENCES workflow_steps(id) ON DELETE CASCADE,
       PRIMARY KEY (step_id, depends_on)
     );
     CREATE TABLE runs (
       id TEXT PRIMARY KEY,
       workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
       workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
       name TEXT NOT NULL,
       state TEXT NOT NULL,
       created_at TEXT NOT NULL,
       finished_at TEXT
     );
     CREATE INDEX runs_workspace ON runs(workspace_id, created_at);
     CREATE TABLE run_steps (
       id TEXT PRIMARY KEY,
       run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
       position INTEGER NOT NULL,
       name TEXT NOT NULL,
       agent_id TEXT NOT NULL,
       state TEXT NOT NULL,
       started_at TEXT,
       finished_at TEXT,
       exit_code INTEGER,
       note TEXT
     );
     CREATE TABLE run_deps (
       run_step_id TEXT NOT NULL REFERENCES run_steps(id) ON DELETE CASCADE,
       depends_on TEXT NOT NULL REFERENCES run_steps(id) ON DELETE CASCADE,
       PRIMARY KEY (run_step_id, depends_on)
     );",
    // 7: opt-in project memory (ADR 0031). Notes are text the user typed; nothing is captured. `rid` is a
    // stable integer key for the full-text index (a TEXT key's implicit rowid can change on VACUUM). The
    // triggers keep memory_fts in step, including when a workspace removal cascades to its notes.
    "ALTER TABLE workspaces ADD COLUMN memory_enabled INTEGER NOT NULL DEFAULT 0;
     CREATE TABLE memory_notes (
       rid INTEGER PRIMARY KEY AUTOINCREMENT,
       id TEXT NOT NULL UNIQUE,
       workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
       title TEXT NOT NULL,
       body TEXT NOT NULL,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE INDEX memory_notes_workspace ON memory_notes(workspace_id, updated_at);
     CREATE VIRTUAL TABLE memory_fts USING fts5(title, body, content='memory_notes', content_rowid='rid', tokenize='unicode61 remove_diacritics 2');
     CREATE TRIGGER memory_ai AFTER INSERT ON memory_notes BEGIN
       INSERT INTO memory_fts(rowid, title, body) VALUES (new.rid, new.title, new.body);
     END;
     CREATE TRIGGER memory_ad AFTER DELETE ON memory_notes BEGIN
       INSERT INTO memory_fts(memory_fts, rowid, title, body) VALUES ('delete', old.rid, old.title, old.body);
     END;
     CREATE TRIGGER memory_au AFTER UPDATE ON memory_notes BEGIN
       INSERT INTO memory_fts(memory_fts, rowid, title, body) VALUES ('delete', old.rid, old.title, old.body);
       INSERT INTO memory_fts(rowid, title, body) VALUES (new.rid, new.title, new.body);
     END;",
];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        Self::init(Connection::open(path)?, true)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?, false)
    }

    fn init(mut conn: Connection, file: bool) -> Result<Self, DbError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        if file {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
        }
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    /// Raw connection for integration tests that must inspect what is actually stored.
    #[doc(hidden)]
    pub fn c_for_test(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // ---- workspaces ----

    /// Add a workspace for `root`. If that directory is already a workspace, returns the existing
    /// one (idempotent). The path is validated and canonicalized.
    pub fn create_workspace(&self, root: &str) -> Result<Workspace, DbError> {
        let root = validate_root(root)?;
        let root_str = root.to_string_lossy().into_owned();
        if let Some(w) = self.workspace_by_path(&root_str)? {
            return Ok(w);
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root_str.clone());
        let now = iso8601_utc_now();
        let id = new_id();
        self.c().execute(
            "INSERT INTO workspaces (id, name, root_path, created_at, updated_at, last_opened_at) VALUES (?1, ?2, ?3, ?4, ?4, ?4)",
            params![id, name, root_str, now],
        )?;
        self.workspace(&id)
    }

    pub fn workspace(&self, id: &str) -> Result<Workspace, DbError> {
        self.c().query_row("SELECT id, name, root_path, created_at, updated_at, last_opened_at FROM workspaces WHERE id = ?1", [id], row_to_workspace).optional()?.ok_or(DbError::NotFound)
    }

    fn workspace_by_path(&self, path: &str) -> Result<Option<Workspace>, DbError> {
        Ok(self.c().query_row("SELECT id, name, root_path, created_at, updated_at, last_opened_at FROM workspaces WHERE root_path = ?1", [path], row_to_workspace).optional()?)
    }

    /// Most recently opened first.
    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, DbError> {
        let c = self.c();
        let mut st = c.prepare("SELECT id, name, root_path, created_at, updated_at, last_opened_at FROM workspaces ORDER BY COALESCE(last_opened_at, created_at) DESC, name")?;
        let rows = st
            .query_map([], row_to_workspace)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn rename_workspace(&self, id: &str, name: &str) -> Result<Workspace, DbError> {
        let name = validate_name(name)?;
        let n = self.c().execute(
            "UPDATE workspaces SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, iso8601_utc_now(), id],
        )?;
        if n == 0 {
            return Err(DbError::NotFound);
        }
        self.workspace(id)
    }

    pub fn touch_workspace(&self, id: &str) -> Result<(), DbError> {
        let n = self.c().execute(
            "UPDATE workspaces SET last_opened_at = ?1 WHERE id = ?2",
            params![iso8601_utc_now(), id],
        )?;
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Remove the workspace record and its session metadata. Never touches project files.
    pub fn remove_workspace(&self, id: &str) -> Result<(), DbError> {
        let n = self
            .c()
            .execute("DELETE FROM workspaces WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    // ---- settings (JSON values keyed by name; never secrets, see ADR 0004) ----

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        Ok(self
            .c()
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value_json: &str) -> Result<(), DbError> {
        if key.is_empty() || key.len() > 200 {
            return Err(DbError::Invalid("invalid setting key".into()));
        }
        self.c().execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = ?3",
            params![key, value_json, iso8601_utc_now()],
        )?;
        Ok(())
    }

    /// All `(key, value_json)` whose key starts with `prefix`, ordered by key.
    pub fn list_settings(&self, prefix: &str) -> Result<Vec<(String, String)>, DbError> {
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let c = self.c();
        let mut st = c.prepare(
            "SELECT key, value_json FROM settings WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key",
        )?;
        let rows = st
            .query_map([format!("{escaped}%")], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), DbError> {
        self.c()
            .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    // ---- sessions ----

    pub fn insert_session(
        &self,
        workspace_id: &str,
        kind: SessionKind,
        title: &str,
        adapter_id: Option<&str>,
        approval_id: Option<&str>,
    ) -> Result<String, DbError> {
        let title = validate_name(title)?;
        let id = new_id();
        let now = iso8601_utc_now();
        self.c().execute(
            "INSERT INTO sessions (id, workspace_id, kind, title, adapter_id, lifecycle, started_at, created_at, updated_at, approval_id) VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?6, ?6, ?7)",
            params![id, workspace_id, kind.as_str(), title, adapter_id, now, approval_id],
        )?;
        Ok(id)
    }

    /// Record which Naurix-created worktree a session runs in (ADR 0028).
    pub fn set_session_worktree(&self, session_id: &str, worktree_id: &str) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE sessions SET worktree_id = ?1 WHERE id = ?2",
            params![worktree_id, session_id],
        )?;
        Ok(())
    }

    pub fn finish_session(&self, id: &str, lifecycle: &str) -> Result<(), DbError> {
        if !matches!(lifecycle, "exited" | "failed" | "interrupted") {
            return Err(DbError::Invalid(format!("invalid lifecycle '{lifecycle}'")));
        }
        let now = iso8601_utc_now();
        self.c().execute("UPDATE sessions SET lifecycle = ?1, ended_at = COALESCE(ended_at, ?2), updated_at = ?2 WHERE id = ?3", params![lifecycle, now, id])?;
        Ok(())
    }

    pub fn rename_session(&self, id: &str, title: &str) -> Result<(), DbError> {
        let title = validate_name(title)?;
        self.c().execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, iso8601_utc_now(), id],
        )?;
        Ok(())
    }

    /// Startup recovery (DATA-004): processes do not survive an app restart, so any session still
    /// recorded as starting/running is marked `interrupted`. Returns how many were changed.
    pub fn mark_active_sessions_interrupted(&self) -> Result<usize, DbError> {
        let now = iso8601_utc_now();
        Ok(self.c().execute("UPDATE sessions SET lifecycle = 'interrupted', ended_at = COALESCE(ended_at, ?1), updated_at = ?1 WHERE lifecycle IN ('starting', 'running')", [now])?)
    }

    pub fn recent_sessions(
        &self,
        workspace_id: &str,
        limit: u32,
    ) -> Result<Vec<SessionRecord>, DbError> {
        let c = self.c();
        let mut st = c.prepare("SELECT id, workspace_id, kind, title, adapter_id, lifecycle, started_at, ended_at FROM sessions WHERE workspace_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2")?;
        let rows = st
            .query_map(params![workspace_id, limit], |r| {
                Ok(SessionRecord {
                    id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    kind: r.get(2)?,
                    title: r.get(3)?,
                    adapter_id: r.get(4)?,
                    lifecycle: r.get(5)?,
                    started_at: r.get(6)?,
                    ended_at: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Keep only the newest `keep` sessions per workspace so metadata stays small.
    pub fn prune_sessions(&self, keep: u32) -> Result<usize, DbError> {
        Ok(self.c().execute(
            "DELETE FROM sessions WHERE rowid IN (
               SELECT rowid FROM (
                 SELECT rowid, ROW_NUMBER() OVER (PARTITION BY workspace_id ORDER BY created_at DESC, rowid DESC) AS rn FROM sessions
               ) WHERE rn > ?1)",
            [keep],
        )?)
    }
}

fn row_to_workspace(r: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: r.get(0)?,
        name: r.get(1)?,
        root_path: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
        last_opened_at: r.get(5)?,
    })
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let current = conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? as usize;
    if current > MIGRATIONS.len() {
        return Err(DbError::Invalid(format!(
            "Your Naurix data was saved by a newer version of Naurix (database schema v{current}; this version supports up to v{}). Install the latest Naurix to open it.",
            MIGRATIONS.len()
        )));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{:x}{:x}{:x}",
        nanos,
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests;

/// One-time move of the data folder from the pre-rename app identifier to the current one.
/// Renames the whole folder (atomic, no copying of a live SQLite/WAL file), then the database
/// files inside it. Does nothing if the new folder already exists or there is no legacy folder.
/// Returns whether a migration happened.
pub fn migrate_legacy_data_dir(legacy_dir: &Path, current_dir: &Path) -> std::io::Result<bool> {
    if current_dir.exists() || !legacy_dir.is_dir() {
        return Ok(false);
    }
    std::fs::rename(legacy_dir, current_dir)?;
    for suffix in ["", "-wal", "-shm"] {
        let old = current_dir.join(format!("agent-terminal.db{suffix}"));
        if old.exists() {
            std::fs::rename(&old, current_dir.join(format!("naurix.db{suffix}")))?;
        }
    }
    Ok(true)
}
