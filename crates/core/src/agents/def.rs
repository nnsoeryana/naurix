use super::search::find_executable;
use super::AgentError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Only capabilities that are implemented and tested may be listed (PRD §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    PtyLaunch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Preset,
    Custom,
}

/// Built-in launch presets: (id, display name, executable name). No default arguments are assumed.
pub const PRESETS: &[(&str, &str, &str)] = &[
    ("claude-code", "Claude Code", "claude"),
    ("codex", "OpenAI Codex CLI", "codex"),
    ("gemini-cli", "Gemini CLI", "gemini"),
    ("antigravity-cli", "Antigravity CLI", "agy"),
    ("opencode", "OpenCode", "opencode"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    pub kind: AgentKind,
    /// Bare command name (looked up in the search dirs) or an absolute path.
    pub executable: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl AgentDef {
    pub fn capabilities(&self) -> &'static [AgentCapability] {
        &[AgentCapability::PtyLaunch]
    }

    pub fn preset(id: &str) -> Option<Self> {
        PRESETS
            .iter()
            .find(|p| p.0 == id)
            .map(|(id, name, exe)| Self {
                id: (*id).into(),
                name: (*name).into(),
                kind: AgentKind::Preset,
                executable: (*exe).into(),
                args: vec![],
                env: vec![],
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// A user-defined CLI agent. Environment values are stored unencrypted, so secret-looking names
/// are rejected (secrets belong in the Keychain; ADR 0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAgent {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub env: Vec<EnvVar>,
}

impl From<&CustomAgent> for AgentDef {
    fn from(c: &CustomAgent) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            kind: AgentKind::Custom,
            executable: c.executable.clone(),
            args: c.args.clone(),
            env: c
                .env
                .iter()
                .map(|e| (e.name.clone(), e.value.clone()))
                .collect(),
        }
    }
}

const SECRET_HINTS: &[&str] = &[
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "AUTH",
];

pub fn validate_custom(c: &CustomAgent) -> Result<(), AgentError> {
    let bad = |m: &str| Err(AgentError::Invalid(m.to_string()));
    let name = c.name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return bad("Name must be 1-60 characters.");
    }
    let exe = c.executable.as_str();
    let bare = !exe.is_empty()
        && exe
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    let abs = exe.starts_with('/') && !exe.contains('\0');
    if !(bare || abs) || exe == "." || exe == ".." {
        return bad("Executable must be a command name (like `aider`) or an absolute path.");
    }
    if c.args.len() > 64 || c.args.iter().any(|a| a.len() > 1000 || a.contains('\0')) {
        return bad("Too many or too long arguments.");
    }
    if c.args
        .iter()
        .any(|a| crate::credential::looks_like_credential(a))
    {
        return bad("An argument looks like a token, key or password. Naurix does not store credentials; do not put them in arguments.");
    }
    if c.env.len() > 32 {
        return bad("Too many environment variables.");
    }
    for e in &c.env {
        let valid_name = !e.name.is_empty()
            && e.name.len() <= 100
            && e.name.chars().enumerate().all(|(i, ch)| {
                ch == '_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit())
            });
        if !valid_name {
            return bad(&format!("Invalid environment variable name '{}'.", e.name));
        }
        let upper = e.name.to_ascii_uppercase();
        if upper.starts_with("DYLD_") || upper.starts_with("LD_") {
            return bad(
                "DYLD_*/LD_* variables are not allowed (they inject code into the process).",
            );
        }
        if SECRET_HINTS.iter().any(|h| upper.contains(h)) {
            return bad(&format!("'{}' looks like a secret. Naurix does not store credentials; set it in your shell profile instead.", e.name));
        }
        if e.value.len() > 4096 || e.value.contains('\0') {
            return bad("Environment value too long or contains NUL.");
        }
        if crate::credential::looks_like_credential(&e.value) {
            return bad(&format!("The value of '{}' looks like a token, key or password. Naurix does not store credentials; set it in your shell profile instead.", e.name));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Environment additions (PATH for the search dirs, then the agent's own variables).
    pub env: Vec<(String, String)>,
    /// Human-readable command line for the review dialog. Display only: never executed.
    pub display: String,
}

fn is_exec(p: &Path) -> bool {
    crate::platform::is_executable_file(p)
}

fn quote_for_display(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Resolve an agent to a concrete launch. `path_override` (user-chosen executable) wins.
pub fn plan_launch(
    def: &AgentDef,
    path_override: Option<&str>,
    dirs: &[PathBuf],
) -> Result<LaunchPlan, AgentError> {
    let program = match path_override {
        Some(p) if is_exec(Path::new(p)) && crate::platform::is_absolute_str(p) => PathBuf::from(p),
        Some(p) => {
            return Err(AgentError::Invalid(format!(
                "'{p}' is not an executable file."
            )))
        }
        None if def.executable.starts_with('/') => {
            let p = PathBuf::from(&def.executable);
            if is_exec(&p) {
                p
            } else {
                return Err(AgentError::NotFound {
                    name: def.name.clone(),
                });
            }
        }
        None => find_executable(&def.executable, dirs).ok_or_else(|| AgentError::NotFound {
            name: def.name.clone(),
        })?,
    };
    let mut env = vec![(
        "PATH".to_string(),
        std::env::join_paths(dirs)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )];
    env.extend(def.env.iter().cloned());
    let display = std::iter::once(program.to_string_lossy().into_owned())
        .chain(def.args.iter().cloned())
        .map(|a| quote_for_display(&a))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(LaunchPlan {
        program,
        args: def.args.clone(),
        env,
        display,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(f: impl FnOnce(&mut CustomAgent)) -> CustomAgent {
        let mut c = CustomAgent {
            id: "custom-1".into(),
            name: "Aider".into(),
            executable: "aider".into(),
            args: vec!["--model".into(), "x y".into()],
            env: vec![EnvVar {
                name: "AIDER_MODE".into(),
                value: "fast".into(),
            }],
        };
        f(&mut c);
        c
    }

    #[test]
    fn presets_cover_the_agents_and_claim_only_pty_launch() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.0).collect();
        assert_eq!(
            ids,
            [
                "claude-code",
                "codex",
                "gemini-cli",
                "antigravity-cli",
                "opencode"
            ]
        );
        let d = AgentDef::preset("codex").unwrap();
        assert_eq!(d.capabilities(), &[AgentCapability::PtyLaunch]);
        assert!(d.args.is_empty(), "no invented default arguments");
        assert!(AgentDef::preset("nope").is_none());
    }

    #[test]
    fn accepts_a_reasonable_custom_agent() {
        assert!(validate_custom(&custom(|_| {})).is_ok());
        assert!(validate_custom(&custom(|c| c.executable = "/opt/tools/my-agent".into())).is_ok());
    }

    #[test]
    fn rejects_bad_custom_agents() {
        for f in [
            (|c: &mut CustomAgent| c.name = "  ".into()) as fn(&mut CustomAgent),
            |c| c.executable = "./relative".into(),
            |c| c.executable = "a b".into(),
            |c| c.executable = "..".into(),
            |c| c.args = vec!["a\0b".into()],
            |c| {
                c.env = vec![EnvVar {
                    name: "1BAD".into(),
                    value: "x".into(),
                }]
            },
            |c| {
                c.env = vec![EnvVar {
                    name: "A=B".into(),
                    value: "x".into(),
                }]
            },
            |c| {
                c.env = vec![EnvVar {
                    name: "DYLD_INSERT_LIBRARIES".into(),
                    value: "/x".into(),
                }]
            },
            |c| {
                c.env = vec![EnvVar {
                    name: "OPENAI_API_KEY".into(),
                    value: "sk-xxx".into(),
                }]
            },
            |c| {
                c.env = vec![EnvVar {
                    name: "gh_token".into(),
                    value: "x".into(),
                }]
            },
        ] {
            assert!(validate_custom(&custom(f)).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn plan_resolves_in_search_dirs_and_builds_a_reviewable_command() {
        let dir = std::env::temp_dir().join(format!("at-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let exe = dir.join("aider");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

        let def = AgentDef::from(&custom(|_| {}));
        let plan = plan_launch(&def, None, std::slice::from_ref(&dir)).unwrap();
        assert_eq!(plan.program, exe);
        assert_eq!(plan.args, ["--model", "x y"]);
        assert_eq!(plan.display, format!("{} --model 'x y'", exe.display()));
        assert_eq!(plan.env[0].0, "PATH");
        assert!(plan.env.contains(&("AIDER_MODE".into(), "fast".into())));

        assert!(matches!(
            plan_launch(
                &AgentDef::preset("codex").unwrap(),
                None,
                std::slice::from_ref(&dir)
            ),
            Err(AgentError::NotFound { .. })
        ));
        // override wins, and must be an absolute executable file
        let over = plan_launch(
            &AgentDef::preset("codex").unwrap(),
            Some(exe.to_str().unwrap()),
            &[],
        )
        .unwrap();
        assert_eq!(over.program, exe);
        assert!(matches!(
            plan_launch(&AgentDef::preset("codex").unwrap(), Some("/no/such"), &[]),
            Err(AgentError::Invalid(_))
        ));
        assert!(matches!(
            plan_launch(&AgentDef::preset("codex").unwrap(), Some("relative/x"), &[]),
            Err(AgentError::Invalid(_))
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn display_quoting_neutralizes_shell_metacharacters() {
        assert_eq!(quote_for_display("a b"), "'a b'");
        assert_eq!(quote_for_display("it's"), "'it'\\''s'");
        assert_eq!(quote_for_display("$(rm -rf ~)"), "'$(rm -rf ~)'");
        assert_eq!(quote_for_display(""), "''");
        assert_eq!(quote_for_display("--flag=value"), "--flag=value");
    }
}
