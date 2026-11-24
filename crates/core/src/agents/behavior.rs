//! Per-agent *behaviour* behind a trait (ADR 0025). Facts and verification levels stay in
//! `adapter.rs`; this module holds what differs per agent when Naurix acts, so shared code (MCP
//! sync, skill checks) never switches on an agent id.
//!
//! Only behaviours that exist today are here. Structured events, headless run, status and resume
//! are added when their first capability is verified (ADR 0014, ADR 0022), not before.

use super::adapter::{
    AdapterInfo, Feature, ANTIGRAVITY_CLI, CLAUDE_CODE, CODEX, GEMINI_CLI, OPENCODE,
};
use std::path::{Path, PathBuf};

/// File layouts Naurix has verified for MCP config, named after the agent each was captured from.
/// Another agent that uses the same layout reuses the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpFileFormat {
    /// `{"mcpServers": {"<name>": {...}}}` (Claude Code `.mcp.json`).
    Claude,
    /// Flat `mcp.<name>` entries (OpenCode 1.18.10).
    OpenCode,
}

/// The config file Naurix may write for an agent, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTarget {
    pub path: PathBuf,
    pub format: McpFileFormat,
    /// Shown with the sync plan (something worth knowing about this agent).
    pub note: Option<&'static str>,
}

/// What Naurix learned from a skill's front matter, independent of any agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillFacts {
    pub has_name: bool,
    pub has_description: bool,
}

/// `status` is one of `ok`, `needs_fix`, `unknown`, `unsupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillCompat {
    pub status: &'static str,
    pub note: &'static str,
}

pub trait AgentAdapter: Sync {
    /// The verified facts for this agent.
    fn info(&self) -> &'static AdapterInfo;

    /// Where and in which layout MCP servers may be written. `None` when there is no such target.
    /// Callers must still check `info().acts_on(Feature::McpConfig)` first; returning a path does
    /// not by itself allow writing.
    fn mcp_target(&self, _workspace_root: &Path, _home: &Path) -> Option<McpTarget> {
        None
    }

    fn skill_compat(&self, facts: SkillFacts) -> SkillCompat;

    /// Launch arguments that make this agent report its status (ADR 0029), with the spool path
    /// still a placeholder (see `signals::SPOOL_PLACEHOLDER`). `None` unless the capability is
    /// Verified for this agent and supported on this platform; callers must also have the user's
    /// opt-in. Nothing is written to the agent's own configuration.
    fn status_args(&self) -> Option<Vec<String>> {
        None
    }
}

struct ClaudeCode;
struct Codex;
struct GeminiCli;
struct AntigravityCli;
struct OpenCode;

impl AgentAdapter for ClaudeCode {
    fn info(&self) -> &'static AdapterInfo {
        &CLAUDE_CODE
    }
    fn status_args(&self) -> Option<Vec<String>> {
        // The hook command is a shell `printf`, so this is Unix only until Windows is verified.
        (cfg!(unix) && self.info().acts_on(Feature::StatusSignals)).then(|| {
            vec![
                "--settings".to_string(),
                crate::signals::claude_hook_settings(),
            ]
        })
    }
    fn mcp_target(&self, workspace_root: &Path, _home: &Path) -> Option<McpTarget> {
        Some(McpTarget {
            path: workspace_root.join(".mcp.json"),
            format: McpFileFormat::Claude,
            note: Some("Claude Code will ask you to approve project servers the first time it starts in this folder."),
        })
    }
    fn skill_compat(&self, f: SkillFacts) -> SkillCompat {
        SkillCompat {
            status: "ok",
            note: if f.has_description {
                "Name defaults to the folder; description is used to decide when to apply it."
            } else {
                "No description: Claude falls back to the first line of the file."
            },
        }
    }
}

impl AgentAdapter for Codex {
    fn info(&self) -> &'static AdapterInfo {
        &CODEX
    }
    fn skill_compat(&self, f: SkillFacts) -> SkillCompat {
        if f.has_name && f.has_description {
            SkillCompat {
                status: "ok",
                note: "Has the required `name` and `description`.",
            }
        } else {
            SkillCompat {
                status: "needs_fix",
                note: "Codex requires `name` and `description` in the front matter.",
            }
        }
    }
}

impl AgentAdapter for GeminiCli {
    fn info(&self) -> &'static AdapterInfo {
        &GEMINI_CLI
    }
    fn skill_compat(&self, _: SkillFacts) -> SkillCompat {
        SkillCompat {
            status: "unsupported",
            note: "No documented skills folder for Gemini CLI itself.",
        }
    }
}

impl AgentAdapter for AntigravityCli {
    fn info(&self) -> &'static AdapterInfo {
        &ANTIGRAVITY_CLI
    }
    fn skill_compat(&self, _: SkillFacts) -> SkillCompat {
        SkillCompat {
            status: "unknown",
            note: "Antigravity's skill requirements were not found in the documentation read.",
        }
    }
}

impl AgentAdapter for OpenCode {
    fn info(&self) -> &'static AdapterInfo {
        &OPENCODE
    }
    fn mcp_target(&self, _workspace_root: &Path, home: &Path) -> Option<McpTarget> {
        let dir = home.join(".config/opencode");
        let path = ["opencode.jsonc", "opencode.json"]
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.exists())
            .unwrap_or_else(|| dir.join("opencode.jsonc"));
        Some(McpTarget {
            path,
            format: McpFileFormat::OpenCode,
            note: None,
        })
    }
    fn skill_compat(&self, f: SkillFacts) -> SkillCompat {
        if f.has_description {
            SkillCompat {
                status: "ok",
                note: "Has a description, so it is advertised to the model.",
            }
        } else {
            SkillCompat {
                status: "needs_fix",
                note: "Without a description OpenCode does not advertise the skill to the model.",
            }
        }
    }
}

/// Registry order is the order skill compatibility is shown in.
static REGISTRY: [&dyn AgentAdapter; 5] =
    [&ClaudeCode, &Codex, &OpenCode, &AntigravityCli, &GeminiCli];

/// Behaviour for a built-in preset. Custom agents have none.
pub fn adapter_for(agent_id: &str) -> Option<&'static dyn AgentAdapter> {
    REGISTRY
        .iter()
        .copied()
        .find(|a| a.info().agent_id == agent_id)
}

pub fn agent_adapters() -> &'static [&'static dyn AgentAdapter] {
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{all_adapters, Feature, PRESETS};

    #[test]
    fn both_registries_list_the_same_agents() {
        let mut a: Vec<_> = agent_adapters().iter().map(|x| x.info().agent_id).collect();
        let mut b: Vec<_> = all_adapters().iter().map(|x| x.agent_id).collect();
        let mut c: Vec<_> = PRESETS.iter().map(|p| p.0).collect();
        a.sort_unstable();
        b.sort_unstable();
        c.sort_unstable();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert!(adapter_for("custom-1").is_none());
    }

    #[test]
    fn an_mcp_target_exists_exactly_when_mcp_config_is_acted_on() {
        let (root, home) = (Path::new("/w"), Path::new("/h"));
        for a in agent_adapters() {
            assert_eq!(
                a.mcp_target(root, home).is_some(),
                a.info().acts_on(Feature::McpConfig),
                "{}",
                a.info().agent_id
            );
        }
    }

    #[test]
    fn skill_compat_order_is_stable() {
        let ids: Vec<_> = agent_adapters().iter().map(|a| a.info().agent_id).collect();
        assert_eq!(
            ids,
            [
                "claude-code",
                "codex",
                "opencode",
                "antigravity-cli",
                "gemini-cli"
            ]
        );
    }
}
