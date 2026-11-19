//! A small static catalog of well-known MCP servers (no marketplace). Every entry is a *template*
//! the user reviews and approves; nothing here runs by itself. Facts were taken from each project's
//! own documentation on 2026-11-19 (see `source`).

use super::model::{McpInput, Pair, Transport};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SecretHint {
    pub name: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub transport: Transport,
    pub command: Option<&'static str>,
    /// `{workspace}` is replaced with the workspace folder when the entry is added.
    pub args: &'static [&'static str],
    pub url: Option<&'static str>,
    /// (header name, value template)
    pub headers: &'static [(&'static str, &'static str)],
    pub secrets: &'static [SecretHint],
    /// Shown before adding.
    pub notes: &'static [&'static str],
}

pub static CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "filesystem",
        title: "Filesystem",
        description: "Read and write files in directories you allow.",
        source: "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem",
        transport: Transport::Stdio,
        command: Some("npx"),
        args: &["-y", "@modelcontextprotocol/server-filesystem", "{workspace}"],
        url: None,
        headers: &[],
        secrets: &[],
        notes: &["Allowed directories are the arguments after the package name. The default is your workspace folder.", "Runs a package downloaded by npx. The README states Node.js 24 or later is required."],
    },
    CatalogEntry {
        id: "github",
        title: "GitHub",
        description: "GitHub's official hosted MCP server.",
        source: "https://github.com/github/github-mcp-server",
        transport: Transport::Http,
        command: None,
        args: &[],
        url: Some("https://api.githubcopilot.com/mcp/"),
        headers: &[("Authorization", "Bearer ${secret:GITHUB_PAT}")],
        secrets: &[SecretHint { name: "GITHUB_PAT", label: "GitHub personal access token" }],
        notes: &["Uses a personal access token. GitHub also supports OAuth, which the agent handles itself."],
    },
    CatalogEntry {
        id: "postgres",
        title: "PostgreSQL (archived)",
        description: "Read-only access to a PostgreSQL database.",
        source: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/postgres",
        transport: Transport::Stdio,
        command: Some("npx"),
        args: &["-y", "@modelcontextprotocol/server-postgres", "${secret:DATABASE_URL}"],
        url: None,
        headers: &[],
        secrets: &[SecretHint { name: "DATABASE_URL", label: "Connection string (postgresql://user:password@host/db)" }],
        notes: &["This reference server was archived by its owner on 2025-05-29 and is no longer maintained.", "Queries run in a READ ONLY transaction. The connection string contains a password, so it is kept in the Keychain."],
    },
    CatalogEntry {
        id: "playwright",
        title: "Playwright",
        description: "Browser automation for agents.",
        source: "https://github.com/microsoft/playwright-mcp",
        transport: Transport::Stdio,
        command: Some("npx"),
        args: &["@playwright/mcp@latest"],
        url: None,
        headers: &[],
        secrets: &[],
        notes: &["Requires Node.js 20 or newer. Add `--headless` as an argument to run without a visible browser.", "Runs a package downloaded by npx."],
    },
    CatalogEntry {
        id: "context7",
        title: "Context7",
        description: "Up-to-date library documentation for agents.",
        source: "https://github.com/upstash/context7",
        transport: Transport::Http,
        command: None,
        args: &[],
        url: Some("https://mcp.context7.com/mcp"),
        headers: &[],
        secrets: &[],
        notes: &["An API key is optional and gives higher rate limits. To use one, add the header `Authorization: Bearer ${secret:CONTEXT7_API_KEY}` and declare that secret."],
    },
    CatalogEntry {
        id: "sentry",
        title: "Sentry",
        description: "Sentry's hosted MCP server.",
        source: "https://mcp.sentry.dev/",
        transport: Transport::Http,
        command: None,
        args: &[],
        url: Some("https://mcp.sentry.dev/mcp"),
        headers: &[],
        secrets: &[],
        notes: &["Sign-in uses OAuth, which the agent performs on first use. Naurix cannot test OAuth servers."],
    },
];

impl CatalogEntry {
    /// An editor-ready definition for this entry.
    pub fn to_input(&self, workspace_root: &str) -> McpInput {
        McpInput {
            id: None,
            name: self.id.to_string(),
            transport: self.transport,
            command: self.command.map(str::to_string),
            args: self
                .args
                .iter()
                .map(|a| a.replace("{workspace}", workspace_root))
                .collect(),
            url: self.url.map(str::to_string),
            env: vec![],
            headers: self
                .headers
                .iter()
                .map(|(n, v)| Pair {
                    name: (*n).into(),
                    value: (*v).into(),
                })
                .collect(),
            secrets: self.secrets.iter().map(|s| s.name.to_string()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::validate_input;

    #[test]
    fn every_entry_is_valid_documented_and_free_of_plaintext_secrets() {
        let ids: Vec<_> = CATALOG.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            [
                "filesystem",
                "github",
                "postgres",
                "playwright",
                "context7",
                "sentry"
            ]
        );
        for e in CATALOG {
            assert!(e.source.starts_with("https://"), "{}", e.id);
            assert!(!e.description.is_empty() && !e.notes.is_empty(), "{}", e.id);
            validate_input(&e.to_input("/Users/me/project"))
                .unwrap_or_else(|err| panic!("{}: {err}", e.id));
        }
    }

    #[test]
    fn archived_postgres_server_is_flagged() {
        let pg = CATALOG.iter().find(|e| e.id == "postgres").unwrap();
        assert!(pg.title.contains("archived") && pg.notes.iter().any(|n| n.contains("archived")));
    }

    #[test]
    fn workspace_placeholder_is_filled_in() {
        let fs = CATALOG.iter().find(|e| e.id == "filesystem").unwrap();
        assert!(fs.to_input("/p/w").args.contains(&"/p/w".to_string()));
    }
}
