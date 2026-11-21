//! Sync registry servers into agent config files: plan (dry run) -> approve -> backup -> apply ->
//! rollback. Only agents/shapes that M6 *verified* are supported (ADR 0014). Rules:
//! * never overwrite or remove an entry Naurix did not write, or one the user edited since;
//! * secrets are written as references (`${NAURIX_MCP_…}` / `{env:NAURIX_MCP_…}`), never as values;
//! * a file that is not plain JSON (comments, trailing commas) is never rewritten.

use super::model::{expand, secret_env_var, McpServer, Transport};
use super::store::McpStore;
use super::McpError;
use crate::agents::{adapter_for, Feature, McpFileFormat as Shape, McpTarget};
use crate::persistence::{iso8601_utc_now, new_id};
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Add,
    Update,
    Remove,
    Unchanged,
    /// A user-owned or user-edited entry exists under this name: left alone.
    Conflict,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedChange {
    pub entry_key: String,
    pub action: Action,
    pub detail: String,
    #[serde(skip)]
    server_id: Option<String>,
    #[serde(skip)]
    new_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPlan {
    pub agent_id: String,
    pub file: String,
    pub file_exists: bool,
    /// Why nothing can be written (unsupported shape, comments in the file, …).
    pub blocked: Option<String>,
    pub changes: Vec<PlannedChange>,
    /// Hash of the file the plan was computed from.
    pub base_hash: Option<String>,
    /// Binds "Apply" to exactly what the user reviewed.
    pub plan_hash: String,
    /// The JSON entries that would be written, for review.
    pub preview: String,
    /// Things worth saying about this agent (e.g. it asks for approval itself).
    pub notes: Vec<String>,
    #[serde(skip)]
    new_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncRun {
    pub id: String,
    pub agent_id: String,
    pub config_path: String,
    pub summary: String,
    pub created_at: String,
    pub rolled_back_at: Option<String>,
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The config file and layout Naurix manages for an agent, if M6 verified one. Which file and
/// layout belong to which agent is the adapter's business (ADR 0025); this only enforces the gate.
fn target(agent_id: &str, workspace_root: &Path, home: &Path) -> Result<McpTarget, McpError> {
    let a = adapter_for(agent_id)
        .ok_or_else(|| McpError::Sync(format!("{agent_id} has no adapter.")))?;
    if !a.info().acts_on(Feature::McpConfig) {
        return Err(McpError::Sync(
            "Syncing MCP servers to this agent is not verified yet (see docs/AGENT_ADAPTERS.md)."
                .into(),
        ));
    }
    a.mcp_target(workspace_root, home)
        .ok_or_else(|| McpError::Sync(format!("No MCP target for {agent_id}.")))
}

/// The config file Naurix manages for an agent, if M6 verified one.
pub fn target_path(
    agent_id: &str,
    workspace_root: &Path,
    home: &Path,
) -> Result<PathBuf, McpError> {
    target(agent_id, workspace_root, home).map(|t| t.path)
}

fn servers_key(s: Shape) -> &'static str {
    match s {
        Shape::Claude => "mcpServers",
        Shape::OpenCode => "mcp",
    }
}

/// The entry Naurix would write for a server, in the agent's verified format.
fn render_entry(s: &McpServer, shape: Shape) -> Value {
    let reference = |text: &str| -> String {
        expand(text, &|n| match shape {
            Shape::Claude => format!("${{{}}}", secret_env_var(&s.name, n)),
            Shape::OpenCode => format!("{{env:{}}}", secret_env_var(&s.name, n)),
        })
    };
    let pairs = |v: &[super::model::Pair]| -> Value {
        Value::Object(
            v.iter()
                .map(|p| (p.name.clone(), Value::String(reference(&p.value))))
                .collect(),
        )
    };
    let args: Vec<String> = s.args.iter().map(|a| reference(a)).collect();
    match (shape, s.transport) {
        (Shape::Claude, Transport::Stdio) => {
            let mut m = Map::new();
            m.insert("type".into(), json!("stdio"));
            m.insert("command".into(), json!(s.command));
            m.insert("args".into(), json!(args));
            if !s.env.is_empty() {
                m.insert("env".into(), pairs(&s.env));
            }
            Value::Object(m)
        }
        (Shape::Claude, Transport::Http) => {
            let mut m = Map::new();
            m.insert("type".into(), json!("http"));
            m.insert("url".into(), json!(s.url));
            if !s.headers.is_empty() {
                m.insert("headers".into(), pairs(&s.headers));
            }
            Value::Object(m)
        }
        (Shape::OpenCode, Transport::Stdio) => {
            let mut m = Map::new();
            m.insert("type".into(), json!("local"));
            let mut cmd = vec![s.command.clone().unwrap_or_default()];
            cmd.extend(args);
            m.insert("command".into(), json!(cmd));
            if !s.env.is_empty() {
                m.insert("environment".into(), pairs(&s.env));
            }
            Value::Object(m)
        }
        (Shape::OpenCode, Transport::Http) => {
            let mut m = Map::new();
            m.insert("type".into(), json!("remote"));
            m.insert("url".into(), json!(s.url));
            if !s.headers.is_empty() {
                m.insert("headers".into(), pairs(&s.headers));
            }
            Value::Object(m)
        }
    }
}

fn entry_hash(v: &Value) -> String {
    sha(serde_json::to_string(v).expect("serializable").as_bytes())
}

struct Owned {
    server_id: String,
    hash: String,
}

fn owned_entries(
    store: &McpStore<'_>,
    agent: &str,
    path: &str,
) -> Result<std::collections::HashMap<String, Owned>, McpError> {
    let c = store.db.c();
    let mut st = c.prepare("SELECT entry_key, server_id, entry_hash FROM mcp_sync_entries WHERE agent_id=?1 AND config_path=?2")?;
    let rows = st
        .query_map(params![agent, path], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Owned {
                    server_id: r.get(1)?,
                    hash: r.get(2)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

/// Compute (never write) what syncing would change.
pub fn plan(
    store: &McpStore<'_>,
    agent_id: &str,
    workspace_root: &Path,
    home: &Path,
) -> Result<SyncPlan, McpError> {
    let t = target(agent_id, workspace_root, home)?;
    let (path, shape) = (t.path, t.format);
    let key = servers_key(shape);
    let path_s = path.to_string_lossy().into_owned();
    let mut notes: Vec<String> = t.note.map(str::to_string).into_iter().collect();
    notes.push("Secrets are not written to the file. It refers to environment variables that Naurix provides when it launches the agent, so start the agent from Naurix's Agents panel.".into());

    let mut p = SyncPlan {
        agent_id: agent_id.into(),
        file: path_s.clone(),
        file_exists: path.exists(),
        blocked: None,
        changes: vec![],
        base_hash: None,
        plan_hash: String::new(),
        preview: String::new(),
        notes,
        new_text: None,
    };

    // Read and parse the existing file (strict JSON only).
    let mut root = Value::Object(Map::new());
    if p.file_exists {
        let meta = std::fs::metadata(&path).map_err(|e| McpError::Sync(e.to_string()))?;
        if !meta.is_file() || meta.len() > MAX_FILE {
            p.blocked = Some("The config path is not a regular file under 1 MB.".into());
            return Ok(finish(p));
        }
        let bytes = std::fs::read(&path).map_err(|e| McpError::Sync(e.to_string()))?;
        p.base_hash = Some(sha(&bytes));
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(v) if v.is_object() => root = v,
            Ok(_) => {
                p.blocked = Some("The file is JSON but not an object.".into());
                return Ok(finish(p));
            }
            Err(_) => {
                p.blocked = Some("This file contains comments or other non-standard JSON. Naurix will not rewrite it (your comments would be lost). Add the entries below by hand.".into());
                p.preview = desired_preview(store, shape)?;
                return Ok(finish(p));
            }
        }
    } else if shape == Shape::OpenCode {
        root["$schema"] = json!("https://opencode.ai/config.json");
    }
    if shape == Shape::OpenCode && root.get("mcp").is_some_and(|m| m.get("servers").is_some()) {
        p.blocked = Some("This OpenCode file uses the newer `mcp.servers` layout, which has not been verified. Naurix only edits the flat `mcp.<name>` layout checked against OpenCode 1.18.10.".into());
        return Ok(finish(p));
    }
    if root.get(key).is_some_and(|m| !m.is_object()) {
        p.blocked = Some(format!("`{key}` in this file is not an object."));
        return Ok(finish(p));
    }

    let owned = owned_entries(store, agent_id, &path_s)?;
    let servers = store.list()?;
    let mut table: Map<String, Value> = root
        .get(key)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let mut previews = Map::new();
    let mut changed = false;

    // Desired entries: enabled and approved servers.
    for s in servers.iter().filter(|s| s.enabled && s.is_approved()) {
        let want = render_entry(s, shape);
        let want_hash = entry_hash(&want);
        previews.insert(s.name.clone(), want.clone());
        let change = |action, detail: &str| PlannedChange {
            entry_key: s.name.clone(),
            action,
            detail: detail.into(),
            server_id: Some(s.id.clone()),
            new_hash: Some(want_hash.clone()),
        };
        match table.get(&s.name) {
            None => {
                table.insert(s.name.clone(), want);
                changed = true;
                p.changes.push(change(Action::Add, "New entry."));
            }
            Some(cur) => match owned.get(&s.name) {
                Some(o) if o.server_id == s.id && o.hash == entry_hash(cur) => {
                    if o.hash == want_hash {
                        p.changes.push(change(Action::Unchanged, "Already up to date."));
                    } else {
                        table.insert(s.name.clone(), want);
                        changed = true;
                        p.changes.push(change(Action::Update, "Naurix wrote this entry earlier; updating it."));
                    }
                }
                Some(_) => p.changes.push(PlannedChange { action: Action::Conflict, detail: "You edited this entry after Naurix wrote it. Left untouched.".into(), new_hash: None, ..change(Action::Conflict, "") }),
                None => p.changes.push(PlannedChange { action: Action::Conflict, detail: "An entry with this name already exists and is not managed by Naurix. Left untouched.".into(), new_hash: None, ..change(Action::Conflict, "") }),
            },
        }
    }
    // Entries Naurix wrote that are no longer wanted (server deleted, disabled or edited-unapproved).
    let wanted: std::collections::HashSet<&str> = servers
        .iter()
        .filter(|s| s.enabled && s.is_approved())
        .map(|s| s.name.as_str())
        .collect();
    let mut keys: Vec<_> = owned.keys().cloned().collect();
    keys.sort();
    for k in keys.into_iter().filter(|k| !wanted.contains(k.as_str())) {
        let o = &owned[&k];
        let mk = |action, detail: &str| PlannedChange {
            entry_key: k.clone(),
            action,
            detail: detail.into(),
            server_id: Some(o.server_id.clone()),
            new_hash: None,
        };
        match table.get(&k) {
            None => p
                .changes
                .push(mk(Action::Unchanged, "Already removed from the file.")),
            Some(cur) if entry_hash(cur) == o.hash => {
                table.shift_remove(&k);
                changed = true;
                p.changes.push(mk(
                    Action::Remove,
                    "Server is deleted, disabled or needs approval again.",
                ));
            }
            Some(_) => p.changes.push(mk(
                Action::Conflict,
                "You edited this entry after Naurix wrote it. Left untouched.",
            )),
        }
    }

    p.preview = serde_json::to_string_pretty(&Value::Object(previews)).unwrap_or_default();
    if changed {
        root.as_object_mut()
            .expect("object")
            .insert(key.into(), Value::Object(table));
        let mut text = serde_json::to_string_pretty(&root).expect("serializable");
        text.push('\n');
        p.new_text = Some(text);
    }
    Ok(finish(p))
}

fn desired_preview(store: &McpStore<'_>, shape: Shape) -> Result<String, McpError> {
    let m: Map<String, Value> = store
        .list()?
        .iter()
        .filter(|s| s.enabled && s.is_approved())
        .map(|s| (s.name.clone(), render_entry(s, shape)))
        .collect();
    Ok(serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_default())
}

fn finish(mut p: SyncPlan) -> SyncPlan {
    let digest = sha(format!(
        "{:?}|{:?}|{}|{:?}",
        p.base_hash,
        p.blocked,
        p.new_text.as_deref().unwrap_or(""),
        p.changes
            .iter()
            .map(|c| (&c.entry_key, c.action as u8))
            .collect::<Vec<_>>()
    )
    .as_bytes());
    p.plan_hash = digest;
    p
}

impl SyncPlan {
    pub fn has_writes(&self) -> bool {
        self.blocked.is_none() && self.new_text.is_some()
    }
}

fn write_atomic(path: &Path, text: &str, like: Option<&Path>) -> Result<(), McpError> {
    let dir = path
        .parent()
        .ok_or_else(|| McpError::Sync("no parent folder".into()))?;
    std::fs::create_dir_all(dir).map_err(|e| McpError::Sync(e.to_string()))?;
    let tmp = dir.join(format!(".naurix-tmp-{}", new_id()));
    std::fs::write(&tmp, text).map_err(|e| McpError::Sync(e.to_string()))?;
    if let Some(src) = like {
        if let Ok(m) = std::fs::metadata(src) {
            let _ = std::fs::set_permissions(&tmp, m.permissions());
        }
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        McpError::Sync(e.to_string())
    })
}

/// Apply a plan the user approved. The plan is recomputed and must match `plan_hash`; the original
/// file is backed up first; the write is atomic. Returns the run id (for rollback).
pub fn apply(
    store: &McpStore<'_>,
    agent_id: &str,
    workspace_root: &Path,
    home: &Path,
    plan_hash: &str,
    backup_dir: &Path,
) -> Result<SyncRun, McpError> {
    let p = plan(store, agent_id, workspace_root, home)?;
    if p.plan_hash != plan_hash {
        return Err(McpError::Sync(
            "The file or your servers changed since you reviewed the plan. Review it again.".into(),
        ));
    }
    if let Some(b) = &p.blocked {
        return Err(McpError::Sync(b.clone()));
    }
    let Some(new_text) = p.new_text.as_deref() else {
        return Err(McpError::Sync("There is nothing to change.".into()));
    };
    let path = PathBuf::from(&p.file);
    let before_exists = path.exists();

    std::fs::create_dir_all(backup_dir).map_err(|e| McpError::Sync(e.to_string()))?;
    let run_id = new_id();
    let backup = if before_exists {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config".into());
        let b = backup_dir.join(format!("{}-{agent_id}-{name}", run_id));
        std::fs::copy(&path, &b)
            .map_err(|e| McpError::Sync(format!("Could not back up the file: {e}")))?;
        Some(b)
    } else {
        None
    };

    // Snapshot ownership rows so rollback can restore them exactly.
    let entries_before: Vec<(String, String, String)> = {
        let c = store.db.c();
        let mut st = c.prepare("SELECT entry_key, server_id, entry_hash FROM mcp_sync_entries WHERE agent_id=?1 AND config_path=?2")?;
        let rows = st
            .query_map(params![agent_id, p.file], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    write_atomic(&path, new_text, before_exists.then_some(path.as_path()))?;
    let after_hash = sha(new_text.as_bytes());
    let now = iso8601_utc_now();
    let mut counts = (0, 0, 0);
    {
        let c = store.db.c();
        for ch in &p.changes {
            match ch.action {
                Action::Add | Action::Update => {
                    c.execute(
                        "INSERT INTO mcp_sync_entries (server_id, agent_id, config_path, entry_key, entry_hash, created_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(agent_id, config_path, entry_key) DO UPDATE SET server_id=?1, entry_hash=?5",
                        params![ch.server_id, agent_id, p.file, ch.entry_key, ch.new_hash, now],
                    )?;
                    if ch.action == Action::Add {
                        counts.0 += 1
                    } else {
                        counts.1 += 1
                    }
                }
                Action::Remove => {
                    c.execute("DELETE FROM mcp_sync_entries WHERE agent_id=?1 AND config_path=?2 AND entry_key=?3", params![agent_id, p.file, ch.entry_key])?;
                    counts.2 += 1;
                }
                Action::Unchanged if ch.detail.starts_with("Already removed") => {
                    c.execute("DELETE FROM mcp_sync_entries WHERE agent_id=?1 AND config_path=?2 AND entry_key=?3", params![agent_id, p.file, ch.entry_key])?;
                }
                _ => {}
            }
        }
    }
    let summary = format!(
        "{} added, {} updated, {} removed",
        counts.0, counts.1, counts.2
    );
    store.db.c().execute(
        "INSERT INTO mcp_sync_runs (id, agent_id, config_path, backup_path, before_exists, before_hash, after_hash, entries_before_json, summary, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![run_id, agent_id, p.file, backup.as_ref().map(|b| b.to_string_lossy().into_owned()), before_exists as i64, p.base_hash, after_hash, serde_json::to_string(&entries_before).expect("serializable"), summary, now],
    )?;
    Ok(SyncRun {
        id: run_id,
        agent_id: agent_id.into(),
        config_path: p.file,
        summary,
        created_at: now,
        rolled_back_at: None,
    })
}

pub fn runs(store: &McpStore<'_>, agent_id: &str, limit: u32) -> Result<Vec<SyncRun>, McpError> {
    let c = store.db.c();
    let mut st = c.prepare("SELECT id, agent_id, config_path, summary, created_at, rolled_back_at FROM mcp_sync_runs WHERE agent_id=?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2")?;
    let rows = st
        .query_map(params![agent_id, limit], |r| {
            Ok(SyncRun {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                config_path: r.get(2)?,
                summary: r.get(3)?,
                created_at: r.get(4)?,
                rolled_back_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Undo a run: restore the backed-up bytes (or delete a file the run created) and the ownership rows.
/// Only the newest run of a file can be undone, and only if the file is exactly as the run left it.
pub fn rollback(store: &McpStore<'_>, run_id: &str) -> Result<SyncRun, McpError> {
    let (agent, path_s, backup, before_exists, after_hash, entries_json, summary, created, rolled): (String, String, Option<String>, i64, String, String, String, String, Option<String>) = store
        .db
        .c()
        .query_row("SELECT agent_id, config_path, backup_path, before_exists, after_hash, entries_before_json, summary, created_at, rolled_back_at FROM mcp_sync_runs WHERE id=?1", [run_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
        })
        .map_err(|_| McpError::NotFound)?;
    if rolled.is_some() {
        return Err(McpError::Sync("This sync was already undone.".into()));
    }
    let newer: i64 = store.db.c().query_row("SELECT COUNT(*) FROM mcp_sync_runs WHERE agent_id=?1 AND config_path=?2 AND rolled_back_at IS NULL AND (created_at > ?3 OR (created_at = ?3 AND rowid > (SELECT rowid FROM mcp_sync_runs WHERE id=?4)))", params![agent, path_s, created, run_id], |r| r.get(0))?;
    if newer > 0 {
        return Err(McpError::Sync(
            "A newer sync exists for this file. Undo that one first.".into(),
        ));
    }
    let path = PathBuf::from(&path_s);
    let current = std::fs::read(&path).ok().map(|b| sha(&b));
    if current.as_deref() != Some(after_hash.as_str()) {
        return Err(McpError::Sync("The file was changed after this sync, so restoring the backup would lose your edits. Nothing was changed.".into()));
    }
    if before_exists != 0 {
        let b = backup.ok_or_else(|| McpError::Sync("The backup is missing.".into()))?;
        let bytes = std::fs::read(&b)
            .map_err(|e| McpError::Sync(format!("Cannot read the backup: {e}")))?;
        write_atomic(&path, &String::from_utf8_lossy(&bytes), Some(&path))?;
    } else {
        std::fs::remove_file(&path).map_err(|e| McpError::Sync(e.to_string()))?;
    }
    let before: Vec<(String, String, String)> =
        serde_json::from_str(&entries_json).unwrap_or_default();
    {
        let c = store.db.c();
        c.execute(
            "DELETE FROM mcp_sync_entries WHERE agent_id=?1 AND config_path=?2",
            params![agent, path_s],
        )?;
        for (k, sid, h) in before {
            c.execute("INSERT INTO mcp_sync_entries (server_id, agent_id, config_path, entry_key, entry_hash, created_at) VALUES (?1,?2,?3,?4,?5,?6)", params![sid, agent, path_s, k, h, iso8601_utc_now()])?;
        }
        c.execute(
            "UPDATE mcp_sync_runs SET rolled_back_at=?2 WHERE id=?1",
            params![run_id, iso8601_utc_now()],
        )?;
    }
    Ok(SyncRun {
        id: run_id.into(),
        agent_id: agent,
        config_path: path_s,
        summary,
        created_at: created,
        rolled_back_at: Some(iso8601_utc_now()),
    })
}
