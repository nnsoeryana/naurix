//! Opt-in project memory (ADR 0031, orchestration stage 9). No Tauri dependency.
//!
//! A note is text the user typed: a title and a body, stored per workspace in SQLite with an FTS5
//! index. Nothing is captured from agents, terminals or files. Note text is never logged and never
//! appears in an error message.

pub mod secrets;

use crate::persistence::{iso8601_utc_now, new_id, Db};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub const MAX_TITLE_CHARS: usize = 100;
pub const MAX_BODY_CHARS: usize = 8_000;
pub const MAX_NOTES: usize = 200;
pub const MAX_RESULTS: usize = 50;
const MAX_QUERY_WORDS: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("{0}")]
    Invalid(String),
    #[error("Memory is off for this project. Turn it on first.")]
    Disabled,
    #[error("That note no longer exists.")]
    NotFound,
    /// Line and kind only; the value is never repeated.
    #[error("Line {line} looks like {kind}. Notes cannot hold secrets: remove it and keep it in your password manager or the system keychain.")]
    Secret { line: usize, kind: &'static str },
    #[error("A project can hold at most {MAX_NOTES} notes. Delete some first.")]
    Full,
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<rusqlite::Error> for MemoryError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}

impl From<crate::persistence::DbError> for MemoryError {
    fn from(e: crate::persistence::DbError) -> Self {
        match e {
            crate::persistence::DbError::NotFound => Self::NotFound,
            other => Self::Storage(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    /// Notes stored, including those hidden while memory is off.
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

/// Keep newlines and tabs, drop other control characters, normalise line ends.
fn clean(s: &str) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// A title and body the database accepts, with secrets refused. Errors never contain the text.
pub fn validate(title: &str, body: &str) -> Result<(String, String), MemoryError> {
    let title: String = clean(title).replace(['\n', '\t'], " ").trim().to_string();
    let body = clean(body).trim().to_string();
    if title.is_empty() {
        return Err(MemoryError::Invalid("A note needs a title.".into()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(MemoryError::Invalid(format!(
            "A title is at most {MAX_TITLE_CHARS} characters."
        )));
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(MemoryError::Invalid(format!(
            "A note is at most {MAX_BODY_CHARS} characters."
        )));
    }
    if let Some(f) = secrets::scan(&title) {
        return Err(MemoryError::Secret {
            line: 1,
            kind: f.kind,
        });
    }
    if let Some(f) = secrets::scan(&body) {
        return Err(MemoryError::Secret {
            line: f.line,
            kind: f.kind,
        });
    }
    Ok((title, body))
}

/// The user's words as an FTS5 query that cannot contain operators: each word becomes a quoted prefix
/// term, all of which must match. `None` for a query with no usable word.
pub fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(MAX_QUERY_WORDS)
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

pub struct MemoryStore<'a> {
    db: &'a Db,
}

const COLS: &str = "n.id, n.workspace_id, n.title, n.body, n.created_at, n.updated_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

impl<'a> MemoryStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn status(&self, workspace_id: &str) -> Result<Status, MemoryError> {
        self.db.workspace(workspace_id)?;
        let c = self.db.c();
        let enabled: i64 = c.query_row(
            "SELECT memory_enabled FROM workspaces WHERE id = ?1",
            [workspace_id],
            |r| r.get(0),
        )?;
        let count: i64 = c.query_row(
            "SELECT COUNT(*) FROM memory_notes WHERE workspace_id = ?1",
            [workspace_id],
            |r| r.get(0),
        )?;
        Ok(Status {
            enabled: enabled != 0,
            count: count as usize,
        })
    }

    pub fn set_enabled(&self, workspace_id: &str, on: bool) -> Result<(), MemoryError> {
        self.db.workspace(workspace_id)?;
        self.db.c().execute(
            "UPDATE workspaces SET memory_enabled = ?2 WHERE id = ?1",
            params![workspace_id, on as i64],
        )?;
        Ok(())
    }

    fn require_enabled(&self, workspace_id: &str) -> Result<(), MemoryError> {
        if self.status(workspace_id)?.enabled {
            Ok(())
        } else {
            Err(MemoryError::Disabled)
        }
    }

    /// Create (`id` is `None`) or edit a note. Refused while memory is off, when the text holds a likely
    /// secret, and when the project already has [`MAX_NOTES`] notes.
    pub fn save(
        &self,
        workspace_id: &str,
        id: Option<&str>,
        title: &str,
        body: &str,
    ) -> Result<Note, MemoryError> {
        self.require_enabled(workspace_id)?;
        let (title, body) = validate(title, body)?;
        let now = iso8601_utc_now();
        let nid = match id {
            Some(id) => {
                let n = self.db.c().execute(
                    "UPDATE memory_notes SET title = ?3, body = ?4, updated_at = ?5 WHERE id = ?1 AND workspace_id = ?2",
                    params![id, workspace_id, title, body, now],
                )?;
                if n == 0 {
                    return Err(MemoryError::NotFound);
                }
                id.to_string()
            }
            None => {
                if self.status(workspace_id)?.count >= MAX_NOTES {
                    return Err(MemoryError::Full);
                }
                let nid = new_id();
                self.db.c().execute(
                    "INSERT INTO memory_notes (id, workspace_id, title, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![nid, workspace_id, title, body, now],
                )?;
                nid
            }
        };
        self.get(workspace_id, &nid)
    }

    pub fn get(&self, workspace_id: &str, id: &str) -> Result<Note, MemoryError> {
        self.require_enabled(workspace_id)?;
        self.db
            .c()
            .query_row(
                &format!(
                    "SELECT {COLS} FROM memory_notes n WHERE n.id = ?1 AND n.workspace_id = ?2"
                ),
                params![id, workspace_id],
                row,
            )
            .optional()?
            .ok_or(MemoryError::NotFound)
    }

    /// Notes of a workspace: newest first, or ranked by relevance when `query` has usable words.
    pub fn list(&self, workspace_id: &str, query: Option<&str>) -> Result<Vec<Note>, MemoryError> {
        self.require_enabled(workspace_id)?;
        let c = self.db.c();
        match query.and_then(fts_query) {
            Some(q) => {
                let mut stmt = c.prepare(&format!(
                    "SELECT {COLS} FROM memory_fts f JOIN memory_notes n ON n.rid = f.rowid
                     WHERE memory_fts MATCH ?1 AND n.workspace_id = ?2 ORDER BY f.rank LIMIT ?3"
                ))?;
                let rows = stmt.query_map(params![q, workspace_id, MAX_RESULTS as i64], row)?;
                Ok(rows.collect::<Result<_, _>>()?)
            }
            None => {
                let mut stmt = c.prepare(&format!(
                    "SELECT {COLS} FROM memory_notes n WHERE n.workspace_id = ?1 ORDER BY n.updated_at DESC, n.rid DESC LIMIT ?2"
                ))?;
                let rows = stmt.query_map(params![workspace_id, MAX_NOTES as i64], row)?;
                Ok(rows.collect::<Result<_, _>>()?)
            }
        }
    }

    /// Removing a note is never blocked, even while memory is off.
    pub fn delete(&self, workspace_id: &str, id: &str) -> Result<(), MemoryError> {
        let n = self.db.c().execute(
            "DELETE FROM memory_notes WHERE id = ?1 AND workspace_id = ?2",
            params![id, workspace_id],
        )?;
        if n == 0 {
            return Err(MemoryError::NotFound);
        }
        Ok(())
    }

    /// Delete every note of a workspace (also while memory is off). Returns how many.
    pub fn delete_all(&self, workspace_id: &str) -> Result<usize, MemoryError> {
        self.db.workspace(workspace_id)?;
        Ok(self.db.c().execute(
            "DELETE FROM memory_notes WHERE workspace_id = ?1",
            [workspace_id],
        )?)
    }

    /// The whole memory of a workspace as text for a file the user chose.
    pub fn export(&self, workspace_id: &str, format: ExportFormat) -> Result<String, MemoryError> {
        let notes = {
            let mut n = self.list(workspace_id, None)?;
            n.reverse(); // oldest first reads like a log
            n
        };
        let ws = self.db.workspace(workspace_id)?;
        let now = iso8601_utc_now();
        Ok(match format {
            ExportFormat::Json => serde_json::to_string_pretty(&serde_json::json!({
                "workspace": ws.name,
                "exportedAt": now,
                "notes": notes.iter().map(|n| serde_json::json!({
                    "title": n.title, "body": n.body, "createdAt": n.created_at, "updatedAt": n.updated_at
                })).collect::<Vec<_>>(),
            }))
            .map_err(|e| MemoryError::Storage(e.to_string()))?,
            ExportFormat::Markdown => {
                let mut s = format!("# Memory: {}\n\nExported {now}. {} notes.\n", ws.name, notes.len());
                for n in &notes {
                    s.push_str(&format!("\n## {}\n\n*Created {}, updated {}.*\n\n{}\n", n.title, n.created_at, n.updated_at, n.body));
                }
                s
            }
        })
    }
}

/// True when `path` is inside a Git repository: a `.git` entry in its folder or any parent. Looked up on
/// disk without running Git. Used to warn before an export lands in a repository (ADR 0031 §7).
pub fn inside_git_repo(path: &Path) -> bool {
    let start = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    let mut cur = start;
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return true;
        }
        cur = d.parent();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_become_quoted_prefix_terms_and_never_carry_operators() {
        assert_eq!(
            fts_query("sqlite fts5").as_deref(),
            Some("\"sqlite\"* \"fts5\"*")
        );
        assert_eq!(fts_query("  ").as_deref(), None);
        assert_eq!(
            fts_query("\"\" ) NEAR( * AND OR").as_deref(),
            Some("\"NEAR\"* \"AND\"* \"OR\"*")
        );
        assert_eq!(
            fts_query("naïve café").as_deref(),
            Some("\"naïve\"* \"café\"*")
        );
        assert_eq!(
            fts_query(&"w ".repeat(20)).unwrap().matches('*').count(),
            MAX_QUERY_WORDS
        );
    }

    #[test]
    fn titles_and_bodies_are_cleaned_and_limited_and_errors_never_hold_the_text() {
        let (t, b) = validate(" A\ttitle\n", "line1\r\nline2\u{7}\n").unwrap();
        assert_eq!((t.as_str(), b.as_str()), ("A title", "line1\nline2"));
        assert!(validate("", "x").is_err());
        assert!(validate(&"t".repeat(101), "x").is_err());
        assert!(validate("t", &"b".repeat(8001)).is_err());
        let e = validate("t", "line\npassword = Xk29fjA0sd9Qw8e7r6")
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("Line 2")
                && e.contains("a password, token or key assignment")
                && !e.contains("Xk29")
        );
    }

    #[test]
    fn git_repository_detection_walks_up_without_running_git() {
        let d = std::env::temp_dir().join(format!("nx-mem-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("repo/.git")).unwrap();
        std::fs::create_dir_all(d.join("repo/docs")).unwrap();
        std::fs::create_dir_all(d.join("plain")).unwrap();
        assert!(inside_git_repo(&d.join("repo/docs/out.md")));
        assert!(inside_git_repo(&d.join("repo/out.md")));
        assert!(!inside_git_repo(&d.join("plain/out.md")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
