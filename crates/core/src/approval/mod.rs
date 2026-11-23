//! Central approval gate (ADR 0027). No Tauri dependency (ADR 0002).
//!
//! The backend builds an [`Action`] from resolved data (never from anything the UI sends) and asks
//! the gate to [`authorize`](ApprovalGate::authorize) it. If a live grant matches the action's
//! fingerprint it is allowed; otherwise the gate returns an [`ApprovalRequest`] with a
//! server-built summary. The UI answers with [`decide`](ApprovalGate::decide) naming only the
//! request id and a scope, then the caller retries, and the gate recomputes the fingerprint, so a
//! changed target can never ride on an earlier decision.
//!
//! Grants are metadata only (class, scope, fingerprint, an agent name and program path). No
//! arguments, environment values, prompts or terminal output are stored here.

use crate::persistence::{iso8601_utc_in, iso8601_utc_now, Db, DbError};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a pending request stays decidable, and how long a "once" grant stays usable.
pub const DEFAULT_TTL: Duration = Duration::from_secs(300);
const MAX_PENDING: usize = 32;
/// Consumed, revoked and expired grants kept as history.
const HISTORY_KEEP: i64 = 200;

/// What kind of thing is being approved. A class is added together with the feature that needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    LaunchAgent,
    /// Create a Git worktree (a Git mutation: once only, ADR 0028).
    CreateWorktree,
    /// Remove a worktree Naurix created (a Git mutation: once only, ADR 0028).
    RemoveWorktree,
}

impl ActionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LaunchAgent => "launch_agent",
            Self::CreateWorktree => "create_worktree",
            Self::RemoveWorktree => "remove_worktree",
        }
    }
}

/// How long a grant lasts. Session and run scopes are added when an action class needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Used for exactly one matching action, within the time limit.
    Once,
    /// Every matching action in one workspace.
    Workspace,
    /// Every matching action in any workspace. Only for classes that allow it (preset agent launch).
    App,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Workspace => "workspace",
            Self::App => "app",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("This approval request is no longer valid. Try the action again.")]
    UnknownRequest,
    #[error("This action cannot be approved for scope '{}'.", .0.as_str())]
    ScopeNotAllowed(Scope),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<DbError> for ApprovalError {
    fn from(e: DbError) -> Self {
        Self::Storage(e.to_string())
    }
}
impl From<rusqlite::Error> for ApprovalError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}

/// Length-prefixed hash of the class and the parts, so no two different part lists collide
/// (`["ab","c"]` and `["a","bc"]` differ).
pub fn fingerprint(class: ActionClass, parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for field in std::iter::once(class.as_str()).chain(parts.iter().copied()) {
        h.update((field.len() as u64).to_be_bytes());
        h.update(field.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// A thing the backend wants to do and needs approval for. Built only from resolved backend data.
#[derive(Debug, Clone)]
pub struct Action {
    pub class: ActionClass,
    pub workspace_id: String,
    pub fingerprint: String,
    /// Short and persisted with a grant: an agent name and program path only.
    pub summary: String,
    /// What the UI shows in the review. Never persisted.
    pub display: String,
    pub allowed_scopes: Vec<Scope>,
}

impl Action {
    /// The workspace this action belongs to, or `None` when there is none (for example a worktree whose
    /// workspace has been removed). Grants are stored with a NULL workspace in that case.
    pub fn workspace(&self) -> Option<&str> {
        Some(self.workspace_id.as_str()).filter(|w| !w.is_empty())
    }

    pub fn new(
        class: ActionClass,
        workspace_id: &str,
        fingerprint_parts: &[&str],
        summary: String,
        display: String,
        allowed_scopes: Vec<Scope>,
    ) -> Self {
        Self {
            class,
            workspace_id: workspace_id.to_string(),
            fingerprint: fingerprint(class, fingerprint_parts),
            summary,
            display,
            allowed_scopes,
        }
    }
}

/// What the UI needs to ask the user. It carries no fingerprint: the UI cannot name a target.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub class: ActionClass,
    pub workspace_id: String,
    pub summary: String,
    pub display: String,
    pub allowed_scopes: Vec<Scope>,
    pub expires_in_secs: u64,
}

#[derive(Debug, Clone)]
pub enum Verdict {
    /// Allowed by this grant (recorded on the session for traceability).
    Allowed {
        approval_id: String,
    },
    Required(ApprovalRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Deny,
    Allow(Scope),
}

/// A grant or a past grant, for Settings. Metadata only.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Approval {
    pub id: String,
    pub class: String,
    pub scope: String,
    pub summary: String,
    pub workspace_id: Option<String>,
    /// `active`, `consumed`, `revoked` or `expired`.
    pub state: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub ended_at: Option<String>,
}

struct Pending {
    action: Action,
    created: Instant,
}

pub struct ApprovalGate {
    db: Arc<Db>,
    ttl: Duration,
    pending: Mutex<HashMap<String, Pending>>,
    counter: AtomicU64,
    seed: std::collections::hash_map::RandomState,
}

const COLS: &str =
    "id, class, scope, summary, workspace_id, state, created_at, last_used_at, ended_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    Ok(Approval {
        id: r.get(0)?,
        class: r.get(1)?,
        scope: r.get(2)?,
        summary: r.get(3)?,
        workspace_id: r.get(4)?,
        state: r.get(5)?,
        created_at: r.get(6)?,
        last_used_at: r.get(7)?,
        ended_at: r.get(8)?,
    })
}

/// Mark grants whose time limit has passed.
fn expire(c: &Connection, now: &str) -> rusqlite::Result<()> {
    c.execute(
        "UPDATE approvals SET state='expired', ended_at=?1 WHERE state='active' AND expires_at IS NOT NULL AND expires_at <= ?1",
        [now],
    )?;
    Ok(())
}

/// Keep only the newest [`HISTORY_KEEP`] non-active rows. Active grants are never pruned.
fn prune(c: &Connection) -> rusqlite::Result<()> {
    c.execute(
        "DELETE FROM approvals WHERE state <> 'active' AND rowid NOT IN (
           SELECT rowid FROM approvals WHERE state <> 'active' ORDER BY created_at DESC, rowid DESC LIMIT ?1)",
        [HISTORY_KEEP],
    )?;
    Ok(())
}

impl ApprovalGate {
    pub fn new(db: Arc<Db>) -> Self {
        Self::with_ttl(db, DEFAULT_TTL)
    }

    /// `ttl` applies to pending requests and to "once" grants (tests use a short one).
    pub fn with_ttl(db: Arc<Db>, ttl: Duration) -> Self {
        Self {
            db,
            ttl,
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
            seed: Default::default(),
        }
    }

    fn new_request_id(&self) -> String {
        use std::hash::{BuildHasher, Hasher};
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut a = self.seed.build_hasher();
        a.write_u64(n);
        a.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        let mut b = self.seed.build_hasher();
        b.write_u64(!n);
        b.write_u64(a.finish());
        format!("{:016x}{:016x}", a.finish(), b.finish())
    }

    /// Allow `action` if a live grant matches, otherwise return a request for the user to decide.
    /// An allowing "once" grant is consumed atomically: it cannot authorize two actions.
    pub fn authorize(&self, action: &Action) -> Result<Verdict, ApprovalError> {
        self.check(action, true)
    }

    /// Like [`authorize`](Self::authorize) but a matching "once" grant is **not** spent. For asking
    /// "would this be allowed?" before the real action, so the UI can show the review first.
    pub fn preview(&self, action: &Action) -> Result<Verdict, ApprovalError> {
        self.check(action, false)
    }

    /// Whether a standing app-wide grant covers this exact action (workspace is irrelevant). Used for
    /// "this executable is already approved" decisions that must not spend anything.
    pub fn has_app_grant(&self, action: &Action) -> Result<bool, ApprovalError> {
        let c = self.db.c();
        expire(&c, &iso8601_utc_now())?;
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM approvals WHERE class=?1 AND fingerprint=?2 AND scope='app' AND state='active'",
            params![action.class.as_str(), action.fingerprint],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Record a standing grant the user already gave through an older mechanism (for example the
    /// pre-gate preset approvals). Never used for new decisions, and never for "once". Returns whether
    /// a grant was created (false if an equal one was already active).
    pub fn import_grant(&self, action: &Action, scope: Scope) -> Result<bool, ApprovalError> {
        if scope == Scope::Once || !action.allowed_scopes.contains(&scope) {
            return Err(ApprovalError::ScopeNotAllowed(scope));
        }
        let workspace = (scope != Scope::App).then(|| action.workspace()).flatten();
        let c = self.db.c();
        let exists: Option<String> = c
            .query_row(
                "SELECT id FROM approvals WHERE class=?1 AND scope=?2 AND fingerprint=?3 AND state='active' AND workspace_id IS ?4",
                params![action.class.as_str(), scope.as_str(), action.fingerprint, workspace],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Ok(false);
        }
        c.execute(
            "INSERT INTO approvals (id, class, scope, fingerprint, summary, workspace_id, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7)",
            params![self.new_request_id(), action.class.as_str(), scope.as_str(), action.fingerprint, action.summary, workspace, iso8601_utc_now()],
        )?;
        Ok(true)
    }

    fn check(&self, action: &Action, consume: bool) -> Result<Verdict, ApprovalError> {
        let now = iso8601_utc_now();
        {
            let c = self.db.c();
            expire(&c, &now)?;
            // Standing grants first, so a "once" grant is only spent when nothing broader applies.
            for _ in 0..3 {
                let hit: Option<(String, String)> = c
                    .query_row(
                        "SELECT id, scope FROM approvals
                         WHERE class=?1 AND fingerprint=?2 AND state='active'
                           AND (scope='app' OR (scope IN ('workspace','once') AND workspace_id IS ?3))
                         ORDER BY CASE scope WHEN 'app' THEN 0 WHEN 'workspace' THEN 1 ELSE 2 END, created_at
                         LIMIT 1",
                        params![action.class.as_str(), action.fingerprint, action.workspace()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((id, scope)) = hit else { break };
                if !consume {
                    return Ok(Verdict::Allowed { approval_id: id });
                }
                if scope == "once" {
                    let spent = c.execute(
                        "UPDATE approvals SET state='consumed', last_used_at=?2, ended_at=?2 WHERE id=?1 AND state='active'",
                        params![id, now],
                    )?;
                    if spent == 1 {
                        prune(&c)?;
                        return Ok(Verdict::Allowed { approval_id: id });
                    }
                } else {
                    c.execute(
                        "UPDATE approvals SET last_used_at=?2 WHERE id=?1",
                        params![id, now],
                    )?;
                    return Ok(Verdict::Allowed { approval_id: id });
                }
            }
        }
        Ok(Verdict::Required(self.open_request(action.clone())))
    }

    fn open_request(&self, action: Action) -> ApprovalRequest {
        let id = self.new_request_id();
        let req = ApprovalRequest {
            id: id.clone(),
            class: action.class,
            workspace_id: action.workspace_id.clone(),
            summary: action.summary.clone(),
            display: action.display.clone(),
            allowed_scopes: action.allowed_scopes.clone(),
            expires_in_secs: self.ttl.as_secs(),
        };
        let mut p = self.pending.lock().unwrap();
        let ttl = self.ttl;
        p.retain(|_, v| v.created.elapsed() < ttl);
        while p.len() >= MAX_PENDING {
            let Some(oldest) = p
                .iter()
                .min_by_key(|(_, v)| v.created)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            p.remove(&oldest);
        }
        p.insert(
            id,
            Pending {
                action,
                created: Instant::now(),
            },
        );
        req
    }

    /// Answer a request. The request is single-use: after a valid answer it is gone. A scope the
    /// action does not allow is an error and leaves the request open. Returns the new grant's id
    /// for an allow, `None` for a deny.
    pub fn decide(
        &self,
        request_id: &str,
        decision: Decision,
    ) -> Result<Option<String>, ApprovalError> {
        let action = {
            let mut p = self.pending.lock().unwrap();
            let Some(req) = p.get(request_id) else {
                return Err(ApprovalError::UnknownRequest);
            };
            if req.created.elapsed() >= self.ttl {
                p.remove(request_id);
                return Err(ApprovalError::UnknownRequest);
            }
            if let Decision::Allow(scope) = decision {
                if !req.action.allowed_scopes.contains(&scope) {
                    return Err(ApprovalError::ScopeNotAllowed(scope));
                }
            }
            p.remove(request_id).expect("present").action
        };
        let Decision::Allow(scope) = decision else {
            return Ok(None);
        };

        let workspace = (scope != Scope::App).then(|| action.workspace()).flatten();
        let c = self.db.c();
        let now = iso8601_utc_now();
        expire(&c, &now)?;
        if scope != Scope::Once {
            // Approving the same thing again does not pile up duplicate grants.
            let existing: Option<String> = c
                .query_row(
                    "SELECT id FROM approvals WHERE class=?1 AND scope=?2 AND fingerprint=?3 AND state='active' AND workspace_id IS ?4",
                    params![action.class.as_str(), scope.as_str(), action.fingerprint, workspace],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                return Ok(Some(id));
            }
        }
        let id = self.new_request_id();
        let expires = (scope == Scope::Once).then(|| iso8601_utc_in(self.ttl.as_secs()));
        c.execute(
            "INSERT INTO approvals (id, class, scope, fingerprint, summary, workspace_id, state, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8)",
            params![id, action.class.as_str(), scope.as_str(), action.fingerprint, action.summary, workspace, now, expires],
        )?;
        prune(&c)?;
        Ok(Some(id))
    }

    /// Grants that can currently authorize something, newest first.
    pub fn list_active(&self) -> Result<Vec<Approval>, ApprovalError> {
        let c = self.db.c();
        expire(&c, &iso8601_utc_now())?;
        let mut st = c.prepare(&format!(
            "SELECT {COLS} FROM approvals WHERE state='active' ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = st.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Consumed, revoked and expired grants, newest first (at most 200 are kept).
    pub fn history(&self, limit: u32) -> Result<Vec<Approval>, ApprovalError> {
        let c = self.db.c();
        expire(&c, &iso8601_utc_now())?;
        let mut st = c.prepare(&format!(
            "SELECT {COLS} FROM approvals WHERE state <> 'active' ORDER BY created_at DESC, rowid DESC LIMIT ?1"
        ))?;
        let rows = st.query_map([limit], row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// End one active grant. Returns whether something was revoked.
    pub fn revoke(&self, id: &str) -> Result<bool, ApprovalError> {
        let n = self.db.c().execute(
            "UPDATE approvals SET state='revoked', ended_at=?2 WHERE id=?1 AND state='active'",
            params![id, iso8601_utc_now()],
        )?;
        Ok(n == 1)
    }

    /// End every active grant. Returns how many.
    pub fn revoke_all(&self) -> Result<usize, ApprovalError> {
        Ok(self.db.c().execute(
            "UPDATE approvals SET state='revoked', ended_at=?1 WHERE state='active'",
            [iso8601_utc_now()],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_do_not_collide_across_field_boundaries_or_classes() {
        let a = fingerprint(ActionClass::LaunchAgent, &["ab", "c"]);
        assert_ne!(a, fingerprint(ActionClass::LaunchAgent, &["a", "bc"]));
        assert_ne!(a, fingerprint(ActionClass::LaunchAgent, &["abc"]));
        assert_ne!(a, fingerprint(ActionClass::LaunchAgent, &["ab", "c", ""]));
        assert_eq!(a, fingerprint(ActionClass::LaunchAgent, &["ab", "c"]));
        assert_eq!(a.len(), 64);
    }
}
