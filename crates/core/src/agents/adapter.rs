//! Per-agent adapter facts (M6) with an explicit **verification level** for every capability.
//!
//! Rule (AGENTS.md, ADR 0014): Naurix only *acts* on a capability whose level is `Verified`.
//! `DocumentedOnly` means an official page describes it but nothing here has confirmed it against a
//! real installation; `Unsupported` means we have no basis at all. Nothing is assumed from memory.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    PtyLaunch,
    McpConfig,
    Skills,
    StatusSignals,
    SessionResume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum Level {
    /// Confirmed by running the real CLI (a fixture in `tests/fixtures/agents`) or by Naurix's own tests.
    Verified {
        evidence: &'static str,
    },
    /// Described by official documentation, not yet confirmed here. Naurix does not act on it.
    DocumentedOnly {
        source: &'static str,
    },
    /// Documented, not confirmed against the agent, but the maintainer decided on 2026-11-19 that Naurix
    /// may act on it **only through a reversible symlink** into the documented directory (ADR 0016).
    /// Whether the agent then discovers the skill is not verified.
    DocumentedAccepted {
        source: &'static str,
        basis: &'static str,
    },
    Unsupported {
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CapabilityInfo {
    pub feature: Feature,
    #[serde(flatten)]
    pub level: Level,
    /// What exactly is (or would be) covered, including important limits.
    pub detail: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    Project,
}

/// A file or directory an agent reads configuration from. Used only to report whether it exists;
/// contents are never read here.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ConfigPath {
    pub purpose: &'static str,
    pub scope: Scope,
    /// `~/…` (user home) or relative to the workspace root.
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AdapterInfo {
    pub agent_id: &'static str,
    pub capabilities: &'static [CapabilityInfo],
    pub config_paths: &'static [ConfigPath],
    /// Version the verified facts were captured with, if any.
    pub verified_with: Option<&'static str>,
    /// Something the user should know before launching (e.g. a deprecation).
    pub notice: Option<&'static str>,
}

impl AdapterInfo {
    pub fn level(&self, f: Feature) -> Level {
        self.capabilities
            .iter()
            .find(|c| c.feature == f)
            .map(|c| c.level)
            .unwrap_or(Level::Unsupported {
                reason: "not researched",
            })
    }

    /// True only for `Verified` or an explicitly accepted, reversible action (see [`Level`]).
    pub fn acts_on(&self, f: Feature) -> bool {
        matches!(
            self.level(f),
            Level::Verified { .. } | Level::DocumentedAccepted { .. }
        )
    }
}

const GENERIC_LAUNCH: Level = Level::Verified { evidence: "Generic PTY launch is covered by integration tests using a scripted agent. The real CLI is not started by the tests." };
const LAUNCH: CapabilityInfo = CapabilityInfo {
    feature: Feature::PtyLaunch,
    level: GENERIC_LAUNCH,
    detail: "Runs the executable with an argument array in a terminal.",
};

const fn doc(feature: Feature, source: &'static str, detail: &'static str) -> CapabilityInfo {
    CapabilityInfo {
        feature,
        level: Level::DocumentedOnly { source },
        detail,
    }
}
const ACCEPTED_BASIS: &str = "Maintainer decision 2026-11-19: a reversible symlink into the documented folder is acceptable without a live discovery test. Naurix never overwrites anything there.";

const fn accepted(feature: Feature, source: &'static str, detail: &'static str) -> CapabilityInfo {
    CapabilityInfo {
        feature,
        level: Level::DocumentedAccepted {
            source,
            basis: ACCEPTED_BASIS,
        },
        detail,
    }
}
const fn none(feature: Feature, reason: &'static str) -> CapabilityInfo {
    CapabilityInfo {
        feature,
        level: Level::Unsupported { reason },
        detail: "",
    }
}

pub static CLAUDE_CODE: AdapterInfo = AdapterInfo {
    agent_id: "claude-code",
    capabilities: &[
        LAUNCH,
        CapabilityInfo {
            feature: Feature::McpConfig,
            level: Level::Verified { evidence: "`.mcp.json` shape captured from Claude Code 2.1.322 (tests/fixtures/agents/claude-2.1.322-project.mcp.json)." },
            detail: "Project scope only: `.mcp.json` in the workspace, key `mcpServers`; stdio = command/args/env, http = type/url/headers. User and local scope live in `~/.claude.json`, which also holds unrelated state, so Naurix will not edit it. `${VAR}` references in `env`, `args` and `headers` are expanded from the environment (verified), so secrets can stay out of the file.",
        },
        accepted(Feature::Skills, "https://code.claude.com/docs/en/skills", "`~/.claude/skills/<name>/SKILL.md` and `.claude/skills/<name>/SKILL.md`; picked up live; skills may bundle scripts that Claude (not Naurix) runs."),
        CapabilityInfo {
            feature: Feature::StatusSignals,
            level: Level::Verified { evidence: "Run against Claude Code 2.1.329 on 2026-11-23 with hooks injected per launch through `--settings` (ADR 0029): `UserPromptSubmit`, `PostToolUse` (working), `PermissionRequest` (awaiting approval, with `--permission-mode default`), `Notification` matcher `idle_prompt` (about a minute after a turn) and `Stop` (turn done) all fired in the interactive TUI, `UserPromptSubmit`, `PermissionRequest` and `Stop` also with `-p`; hooks from `--settings` run alongside the project's own." },
            detail: "Opt-in per agent (Settings). Hooks come from `--settings` for that launch only; the user's settings.json is never written. Each hook appends one fixed word to a private file and never reads the payload (which contains prompts, answers and tool input). Not verified: a refused approval, hook failure or timeout, the cost per tool call, Windows and Linux. Only on Unix for now (the hook command is a shell `printf`).",
        },
        doc(Feature::SessionResume, "https://code.claude.com/docs/en/cli-reference", "`--continue`/`-c` and `--resume`/`-r`."),
    ],
    config_paths: &[
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: ".mcp.json" },
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.claude.json" },
        ConfigPath { purpose: "skills", scope: Scope::User, path: "~/.claude/skills" },
        ConfigPath { purpose: "skills", scope: Scope::Project, path: ".claude/skills" },
        ConfigPath { purpose: "settings", scope: Scope::User, path: "~/.claude/settings.json" },
    ],
    verified_with: Some("2.1.322"),
    notice: None,
};

pub static CODEX: AdapterInfo = AdapterInfo {
    agent_id: "codex",
    capabilities: &[
        LAUNCH,
        doc(Feature::McpConfig, "https://learn.chatgpt.com/docs/extend/mcp?surface=cli", "`~/.codex/config.toml`, tables `[mcp_servers.<name>]` (`command`, `args`, `env`, `url`, `bearer_token_env_var`, `enabled`); project `.codex/config.toml` only for trusted projects. Not verified: Codex is not installed on the development machine."),
        accepted(Feature::Skills, "https://learn.chatgpt.com/docs/build-skills", "`~/.agents/skills` and `.agents/skills` (`SKILL.md` with `name` and `description`); enable/disable with `[[skills.config]]` in config.toml."),
        doc(Feature::StatusSignals, "https://learn.chatgpt.com/docs/config-file/config-reference", "`[hooks]` events include `PermissionRequest` and `Stop` (needs `features.hooks`). The `notify` payload is not described on the reference page."),
        none(Feature::SessionResume, "not researched"),
    ],
    config_paths: &[
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.codex/config.toml" },
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: ".codex/config.toml" },
        ConfigPath { purpose: "skills", scope: Scope::User, path: "~/.agents/skills" },
        ConfigPath { purpose: "skills", scope: Scope::Project, path: ".agents/skills" },
    ],
    verified_with: None,
    notice: None,
};

pub static GEMINI_CLI: AdapterInfo = AdapterInfo {
    agent_id: "gemini-cli",
    capabilities: &[
        LAUNCH,
        doc(Feature::McpConfig, "https://geminicli.com/docs/tools/mcp-server/", "`~/.gemini/settings.json` and `.gemini/settings.json`, key `mcpServers` (`command`, `args`, `env`, `url`, `httpUrl`, `headers`, `trust`). Not verified: Gemini CLI is not installed on the development machine."),
        none(Feature::Skills, "not researched for Gemini CLI itself"),
        none(Feature::StatusSignals, "no documented signal found"),
        none(Feature::SessionResume, "not researched"),
    ],
    config_paths: &[
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.gemini/settings.json" },
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: ".gemini/settings.json" },
    ],
    verified_with: None,
    notice: Some("Google ended Gemini CLI for Google AI Pro/Ultra and free Gemini Code Assist accounts on 18 June 2026 (replaced by Antigravity CLI). It remains available with paid API keys and for Gemini Code Assist Standard/Enterprise."),
};

pub static ANTIGRAVITY_CLI: AdapterInfo = AdapterInfo {
    agent_id: "antigravity-cli",
    capabilities: &[
        LAUNCH,
        doc(Feature::McpConfig, "https://antigravity.google/docs/cli/gcli-migration", "`~/.gemini/config/mcp_config.json` and `.agents/mcp_config.json`, key `mcpServers`; remote servers use `serverUrl` (legacy `url`/`httpUrl` must be renamed). Not verified: not installed on the development machine."),
        accepted(Feature::Skills, "https://antigravity.google/docs/cli/gcli-migration", "`~/.gemini/skills/`, `~/.gemini/antigravity-cli/skills/` and `.agents/skills/`."),
        none(Feature::StatusSignals, "hooks are mentioned, but no event documentation was read"),
        none(Feature::SessionResume, "not researched"),
    ],
    config_paths: &[
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.gemini/config/mcp_config.json" },
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: ".agents/mcp_config.json" },
        ConfigPath { purpose: "skills", scope: Scope::User, path: "~/.gemini/skills" },
        ConfigPath { purpose: "skills", scope: Scope::Project, path: ".agents/skills" },
    ],
    verified_with: None,
    notice: Some("Successor to Gemini CLI (announced 19 May 2026); the command is `agy`. Launch is checked only as a generic terminal command."),
};

pub static OPENCODE: AdapterInfo = AdapterInfo {
    agent_id: "opencode",
    capabilities: &[
        LAUNCH,
        CapabilityInfo {
            feature: Feature::McpConfig,
            level: Level::Verified { evidence: "Flat `mcp.<name>` shape captured from OpenCode 1.18.10 (tests/fixtures/agents/opencode-1.18.10-global.opencode.jsonc)." },
            detail: "`opencode.json(c)`, entries `mcp.<name>` with `type` local/remote, `command` array, `environment`. Current online docs show a different `mcp.servers.<name>` shape; Naurix only acts when the existing file uses the flat shape verified for this version. `{env:VAR}` references in `environment`, the `command` array and `headers` are expanded (verified).",
        },
        accepted(Feature::Skills, "https://opencode.ai/docs/skills/", "`~/.config/opencode/skills`, `.opencode/skills`; it also reads `~/.claude/skills`, `.claude/skills`, `~/.agents/skills` and `.agents/skills`."),
        doc(Feature::StatusSignals, "https://opencode.ai/docs/server/", "`opencode serve` exposes `GET /api/event` with `session.idle`, `session.status`, `permission.asked` (documented as experimental/volatile). The installed CLI has `serve` and `attach`, but the event stream was not exercised."),
        none(Feature::SessionResume, "not researched"),
    ],
    config_paths: &[
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.config/opencode/opencode.jsonc" },
        ConfigPath { purpose: "mcp", scope: Scope::User, path: "~/.config/opencode/opencode.json" },
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: "opencode.json" },
        ConfigPath { purpose: "mcp", scope: Scope::Project, path: "opencode.jsonc" },
        ConfigPath { purpose: "skills", scope: Scope::User, path: "~/.config/opencode/skills" },
        ConfigPath { purpose: "skills", scope: Scope::Project, path: ".opencode/skills" },
    ],
    verified_with: Some("1.18.10"),
    notice: None,
};

static ALL: [&AdapterInfo; 5] = [
    &CLAUDE_CODE,
    &CODEX,
    &GEMINI_CLI,
    &ANTIGRAVITY_CLI,
    &OPENCODE,
];

/// Adapter facts for a built-in preset. Custom agents have none: they only get the generic launch.
pub fn adapter(agent_id: &str) -> Option<&'static AdapterInfo> {
    ALL.iter().copied().find(|a| a.agent_id == agent_id)
}

pub fn all_adapters() -> &'static [&'static AdapterInfo] {
    &ALL
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Presence {
    pub purpose: &'static str,
    pub scope: Scope,
    pub path: String,
    pub exists: bool,
}

/// Which of an adapter's config locations currently exist. Existence only: contents are never read.
pub fn detect_presence(info: &AdapterInfo, home: &Path, workspace_root: &Path) -> Vec<Presence> {
    info.config_paths
        .iter()
        .map(|c| {
            let full: PathBuf = match c.path.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => workspace_root.join(c.path),
            };
            Presence {
                purpose: c.purpose,
                scope: c.scope,
                path: full.to_string_lossy().into_owned(),
                exists: full.exists(),
            }
        })
        .collect()
}
