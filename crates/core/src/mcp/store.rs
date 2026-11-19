use super::model::{secret_env_var, validate_input, McpInput, McpServer, Pair, Transport};
use super::secrets::{account, SecretStore};
use super::McpError;
use crate::persistence::{iso8601_utc_now, new_id, Db};
use rusqlite::{params, OptionalExtension};

const COLS: &str = "id, name, transport, command, args_json, url, env_json, headers_json, secrets_json, enabled, approved_fingerprint, approved_at, last_status, last_error, last_tool_count, last_tested_at";
const MAX_SERVERS: usize = 50;
const MAX_SECRET_BYTES: usize = 16 * 1024;

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<McpServer> {
    let j = |i: usize| -> rusqlite::Result<String> { r.get(i) };
    let parse = |s: String| -> rusqlite::Result<serde_json::Value> {
        serde_json::from_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    let pairs =
        |v: serde_json::Value| -> Vec<Pair> { serde_json::from_value(v).unwrap_or_default() };
    Ok(McpServer {
        id: r.get(0)?,
        name: r.get(1)?,
        transport: Transport::parse(&j(2)?).unwrap_or(Transport::Stdio),
        command: r.get(3)?,
        args: serde_json::from_value(parse(j(4)?)?).unwrap_or_default(),
        url: r.get(5)?,
        env: pairs(parse(j(6)?)?),
        headers: pairs(parse(j(7)?)?),
        secrets: serde_json::from_value(parse(j(8)?)?).unwrap_or_default(),
        enabled: r.get::<_, i64>(9)? != 0,
        approved_fingerprint: r.get(10)?,
        approved_at: r.get(11)?,
        last_status: r.get(12)?,
        last_error: r.get(13)?,
        last_tool_count: r.get(14)?,
        last_tested_at: r.get(15)?,
    })
}

/// The MCP registry. Metadata lives in SQLite; secret values only in the injected [`SecretStore`].
pub struct McpStore<'a> {
    pub db: &'a Db,
    pub secrets: &'a dyn SecretStore,
}

impl<'a> McpStore<'a> {
    pub fn new(db: &'a Db, secrets: &'a dyn SecretStore) -> Self {
        Self { db, secrets }
    }

    pub fn list(&self) -> Result<Vec<McpServer>, McpError> {
        let c = self.db.c();
        let mut st = c.prepare(&format!(
            "SELECT {COLS} FROM mcp_servers ORDER BY name COLLATE NOCASE"
        ))?;
        let rows = st.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<McpServer, McpError> {
        self.db
            .c()
            .query_row(
                &format!("SELECT {COLS} FROM mcp_servers WHERE id = ?1"),
                [id],
                row,
            )
            .optional()?
            .ok_or(McpError::NotFound)
    }

    /// Create (no id) or update a server definition. New servers start **enabled but not approved**.
    pub fn save(&self, input: McpInput) -> Result<McpServer, McpError> {
        validate_input(&input)?;
        let now = iso8601_utc_now();
        let enc = |v: &dyn erased::Ser| v.to_json();
        let (args, env, headers, secrets) = (
            enc(&input.args),
            enc(&input.env),
            enc(&input.headers),
            enc(&input.secrets),
        );
        match input.id.as_deref().filter(|s| !s.is_empty()) {
            None => {
                if self.list()?.len() >= MAX_SERVERS {
                    return Err(McpError::Invalid(format!("At most {MAX_SERVERS} servers.")));
                }
                let id = new_id();
                self.db
                    .c()
                    .execute(
                        "INSERT INTO mcp_servers (id, name, transport, command, args_json, url, env_json, headers_json, secrets_json, enabled, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,1,?10,?10)",
                        params![id, input.name, input.transport.as_str(), input.command, args, input.url, env, headers, secrets, now],
                    )
                    .map_err(name_conflict)?;
                self.get(&id)
            }
            Some(id) => {
                let before = self.get(id)?;
                self.db
                    .c()
                    .execute(
                        "UPDATE mcp_servers SET name=?2, transport=?3, command=?4, args_json=?5, url=?6, env_json=?7, headers_json=?8, secrets_json=?9, updated_at=?10 WHERE id=?1",
                        params![id, input.name, input.transport.as_str(), input.command, args, input.url, env, headers, secrets, now],
                    )
                    .map_err(name_conflict)?;
                // A secret that is no longer declared must not linger in the Keychain.
                for gone in before.secrets.iter().filter(|s| !input.secrets.contains(s)) {
                    self.secrets.delete(&account(id, gone))?;
                }
                self.get(id)
            }
        }
    }

    pub fn delete(&self, id: &str) -> Result<(), McpError> {
        let s = self.get(id)?;
        for n in &s.secrets {
            self.secrets.delete(&account(id, n))?;
        }
        // mcp_sync_entries are kept on purpose: the next sync plan removes the entries we wrote.
        self.db
            .c()
            .execute("DELETE FROM mcp_servers WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), McpError> {
        let n = self.db.c().execute(
            "UPDATE mcp_servers SET enabled=?2, updated_at=?3 WHERE id=?1",
            params![id, enabled as i64, iso8601_utc_now()],
        )?;
        if n == 0 {
            return Err(McpError::NotFound);
        }
        Ok(())
    }

    /// Approve exactly what the user reviewed: `fingerprint` must equal the current definition's.
    pub fn approve(&self, id: &str, fingerprint: &str) -> Result<McpServer, McpError> {
        let s = self.get(id)?;
        if s.fingerprint() != fingerprint {
            return Err(McpError::ApprovalStale);
        }
        self.db.c().execute(
            "UPDATE mcp_servers SET approved_fingerprint=?2, approved_at=?3 WHERE id=?1",
            params![id, fingerprint, iso8601_utc_now()],
        )?;
        self.get(id)
    }

    pub fn revoke_approval(&self, id: &str) -> Result<(), McpError> {
        self.db.c().execute(
            "UPDATE mcp_servers SET approved_fingerprint=NULL, approved_at=NULL WHERE id=?1",
            [id],
        )?;
        Ok(())
    }

    /// Withdraw every approval (e.g. from the Security settings). Servers stay defined but inert until
    /// reviewed again. Returns how many were approved.
    pub fn revoke_all_approvals(&self) -> Result<usize, McpError> {
        Ok(self.db.c().execute("UPDATE mcp_servers SET approved_fingerprint=NULL, approved_at=NULL WHERE approved_fingerprint IS NOT NULL", [])?)
    }

    pub fn set_secret(&self, id: &str, name: &str, value: &str) -> Result<(), McpError> {
        let s = self.get(id)?;
        if !s.secrets.iter().any(|n| n == name) {
            return Err(McpError::Invalid(format!(
                "'{name}' is not a secret of this server."
            )));
        }
        if value.is_empty() || value.len() > MAX_SECRET_BYTES || value.contains('\0') {
            return Err(McpError::Invalid(
                "A secret must be 1 byte to 16 KB and contain no NUL.".into(),
            ));
        }
        self.secrets.set(&account(id, name), value)
    }

    pub fn clear_secret(&self, id: &str, name: &str) -> Result<(), McpError> {
        self.get(id)?;
        self.secrets.delete(&account(id, name))
    }

    /// For each declared secret: is a value stored? (Never returns values.)
    pub fn secret_status(&self, s: &McpServer) -> Result<Vec<(String, bool)>, McpError> {
        s.secrets
            .iter()
            .map(|n| Ok((n.clone(), self.secrets.get(&account(&s.id, n))?.is_some())))
            .collect()
    }

    /// Resolve `${secret:NAME}` placeholders with real values, in memory only, to run a server.
    /// Fails if any referenced secret has no value.
    pub fn resolve(&self, s: &McpServer, text: &str) -> Result<String, McpError> {
        let missing: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
        let out = super::model::expand(text, &|n| match self.secrets.get(&account(&s.id, n)) {
            Ok(Some(v)) => v,
            _ => {
                missing.borrow_mut().get_or_insert_with(|| n.to_string());
                String::new()
            }
        });
        match missing.into_inner() {
            Some(n) => Err(McpError::Invalid(format!(
                "The secret {n} has no value yet. Set it first."
            ))),
            None => Ok(out),
        }
    }

    /// `(ENV_VAR_NAME, value)` for every stored secret of every enabled, approved server. Added to the
    /// environment of agents launched from Naurix so agent config files can hold references only.
    pub fn launch_environment(&self) -> Result<Vec<(String, String)>, McpError> {
        let mut out = vec![];
        for s in self
            .list()?
            .into_iter()
            .filter(|s| s.enabled && s.is_approved())
        {
            for n in &s.secrets {
                if let Some(v) = self.secrets.get(&account(&s.id, n))? {
                    out.push((secret_env_var(&s.name, n), v));
                }
            }
        }
        Ok(out)
    }

    pub fn record_probe(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        tools: Option<i64>,
    ) -> Result<(), McpError> {
        self.db.c().execute(
            "UPDATE mcp_servers SET last_status=?2, last_error=?3, last_tool_count=?4, last_tested_at=?5 WHERE id=?1",
            params![id, status, error.map(|e| e.chars().take(500).collect::<String>()), tools, iso8601_utc_now()],
        )?;
        Ok(())
    }
}

fn name_conflict(e: rusqlite::Error) -> McpError {
    match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            McpError::Invalid("A server with this name already exists.".into())
        }
        other => other.into(),
    }
}

mod erased {
    pub trait Ser {
        fn to_json(&self) -> String;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> String {
            serde_json::to_string(self).expect("serializable")
        }
    }
}
