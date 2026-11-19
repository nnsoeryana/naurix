use super::McpError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Stdio,
    Http,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "stdio" => Some(Self::Stdio),
            "http" => Some(Self::Http),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pair {
    pub name: String,
    /// Plain text, or text containing `${secret:NAME}` placeholders. Never a secret value.
    pub value: String,
}

/// What the user types into the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpInput {
    pub id: Option<String>,
    pub name: String,
    pub transport: Transport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env: Vec<Pair>,
    pub headers: Vec<Pair>,
    /// Names of secrets whose values are kept in the Keychain and referenced as `${secret:NAME}`.
    pub secrets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub transport: Transport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env: Vec<Pair>,
    pub headers: Vec<Pair>,
    pub secrets: Vec<String>,
    pub enabled: bool,
    pub approved_fingerprint: Option<String>,
    pub approved_at: Option<String>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub last_tool_count: Option<i64>,
    pub last_tested_at: Option<String>,
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
const PLACEHOLDER_OPEN: &str = "${secret:";

fn bad<T>(m: impl Into<String>) -> Result<T, McpError> {
    Err(McpError::Invalid(m.into()))
}

/// Every `${secret:NAME}` in `s`, or an error for any other `${`.
pub fn placeholders(s: &str) -> Result<Vec<String>, McpError> {
    let mut out = vec![];
    let mut rest = s;
    while let Some(i) = rest.find("${") {
        let tail = &rest[i..];
        let Some(body) = tail.strip_prefix(PLACEHOLDER_OPEN) else {
            return bad("Only ${secret:NAME} references are supported (other ${…} would depend on your shell environment).");
        };
        let Some(end) = body.find('}') else {
            return bad("Unclosed ${secret:…} reference.");
        };
        out.push(body[..end].to_string());
        rest = &body[end + 1..];
    }
    Ok(out)
}

/// Replace each `${secret:NAME}` with `f(NAME)`.
pub fn expand(s: &str, f: &dyn Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(PLACEHOLDER_OPEN) {
        out.push_str(&rest[..i]);
        let body = &rest[i + PLACEHOLDER_OPEN.len()..];
        match body.find('}') {
            Some(end) => {
                out.push_str(&f(&body[..end]));
                rest = &body[end + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn looks_secret(name: &str) -> bool {
    let u = name.to_ascii_uppercase();
    SECRET_HINTS.iter().any(|h| u.contains(h))
}

/// `scheme://user:password@host` (a credential embedded in a URL or connection string).
fn has_embedded_credentials(s: &str) -> bool {
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        let after = &rest[i + 3..];
        let authority: &str = after.split(['/', '?', '#', ' ']).next().unwrap_or("");
        if let Some(at) = authority.rfind('@') {
            if authority[..at].contains(':') {
                return true;
            }
        }
        rest = after;
    }
    false
}

fn valid_env_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 100
        && n.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}

pub fn validate_input(i: &McpInput) -> Result<(), McpError> {
    let name = i.name.as_str();
    let name_ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !name_ok {
        return bad("Name: 1-64 letters, digits, '-' or '_', starting with a letter or digit (it becomes the key in agent config files).");
    }
    match i.transport {
        Transport::Stdio => {
            let cmd = i.command.as_deref().unwrap_or("");
            let bare = !cmd.is_empty()
                && cmd
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            let abs = cmd.starts_with('/') && !cmd.contains('\0') && cmd.len() <= 1000;
            if !(bare || abs) || cmd == "." || cmd == ".." {
                return bad("Command must be a command name (like `npx`) or an absolute path.");
            }
            if i.url.as_deref().is_some_and(|u| !u.is_empty()) || !i.headers.is_empty() {
                return bad("A stdio server has no URL or headers.");
            }
        }
        Transport::Http => {
            let url = i.url.as_deref().unwrap_or("");
            let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
                .iter()
                .any(|p| url.starts_with(p));
            if !(url.starts_with("https://") || local)
                || url.len() > 2000
                || url.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return bad(
                    "URL must start with https:// (http:// is allowed only for localhost).",
                );
            }
            if has_embedded_credentials(url) {
                return bad("The URL contains a username and password. Use a header with a ${secret:NAME} reference instead.");
            }
            if i.command.as_deref().is_some_and(|c| !c.is_empty())
                || !i.args.is_empty()
                || !i.env.is_empty()
            {
                return bad("An HTTP server has no command, arguments or environment.");
            }
        }
    }
    if i.args.len() > 64 || i.args.iter().any(|a| a.len() > 2000 || a.contains('\0')) {
        return bad("Too many or too long arguments.");
    }
    if i.env.len() > 32 || i.headers.len() > 16 {
        return bad("Too many environment variables or headers.");
    }
    for e in &i.env {
        if !valid_env_name(&e.name) {
            return bad(format!("Invalid environment variable name '{}'.", e.name));
        }
        let up = e.name.to_ascii_uppercase();
        if up.starts_with("DYLD_") || up.starts_with("LD_") {
            return bad(
                "DYLD_*/LD_* variables are not allowed (they inject code into the process).",
            );
        }
        if e.value.len() > 4096 || e.value.contains('\0') {
            return bad("Environment value too long or contains NUL.");
        }
    }
    for h in &i.headers {
        if h.name.is_empty()
            || h.name.len() > 100
            || !h
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return bad(format!("Invalid header name '{}'.", h.name));
        }
        if h.value.len() > 4096 || h.value.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return bad("Header value too long or contains a line break.");
        }
    }
    // Secrets: declared names, every reference declared, every declaration used.
    if i.secrets.len() > 16 {
        return bad("At most 16 secrets per server.");
    }
    let mut seen = std::collections::HashSet::new();
    for s in &i.secrets {
        let ok = !s.is_empty()
            && s.len() <= 64
            && s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && s.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !ok {
            return bad(format!(
                "Secret name '{s}': use UPPER_CASE letters, digits and '_'."
            ));
        }
        if !seen.insert(s.clone()) {
            return bad(format!("Secret '{s}' is declared twice."));
        }
    }
    let texts = i
        .args
        .iter()
        .chain(i.env.iter().map(|e| &e.value))
        .chain(i.headers.iter().map(|h| &h.value));
    let mut used = std::collections::HashSet::new();
    for t in texts {
        for n in placeholders(t)? {
            if !seen.contains(&n) {
                return bad(format!(
                    "${{secret:{n}}} is used but not declared as a secret."
                ));
            }
            used.insert(n);
        }
    }
    if let Some(unused) = i.secrets.iter().find(|s| !used.contains(*s)) {
        return bad(format!("Secret '{unused}' is declared but never used."));
    }
    // Plain text must not carry credentials: those belong in the Keychain.
    for e in &i.env {
        if looks_secret(&e.name) && !e.value.contains(PLACEHOLDER_OPEN) {
            return bad(format!("'{}' looks like a secret. Make it a secret value (kept in the Keychain) instead of plain text.", e.name));
        }
    }
    for h in &i.headers {
        if looks_secret(&h.name) && !h.value.contains(PLACEHOLDER_OPEN) {
            return bad(format!("Header '{}' looks like it carries a credential. Use a ${{secret:NAME}} reference instead of plain text.", h.name));
        }
    }
    for t in i
        .args
        .iter()
        .chain(i.env.iter().map(|e| &e.value))
        .chain(i.headers.iter().map(|h| &h.value))
    {
        if has_embedded_credentials(t) {
            return bad("A value contains a password inside a URL or connection string. Put the whole value in a secret and reference it as ${secret:NAME}.");
        }
    }
    Ok(())
}

impl McpServer {
    pub fn as_input(&self) -> McpInput {
        McpInput {
            id: Some(self.id.clone()),
            name: self.name.clone(),
            transport: self.transport,
            command: self.command.clone(),
            args: self.args.clone(),
            url: self.url.clone(),
            env: self.env.clone(),
            headers: self.headers.clone(),
            secrets: self.secrets.clone(),
        }
    }

    /// SHA-256 over the canonical definition (templates, never secret values). Approval is bound to
    /// it: any change to what would run (command, arguments, URL, environment or headers) requires
    /// approving again; changing a secret's value does not.
    pub fn fingerprint(&self) -> String {
        let mut secrets = self.secrets.clone();
        secrets.sort();
        let canon = serde_json::json!({
            "transport": self.transport.as_str(),
            "command": self.command,
            "args": self.args,
            "url": self.url,
            "env": self.env,
            "headers": self.headers,
            "secrets": secrets,
        });
        let digest = Sha256::digest(serde_json::to_vec(&canon).expect("serializable"));
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn is_approved(&self) -> bool {
        self.approved_fingerprint.as_deref() == Some(&self.fingerprint())
    }
}

/// Environment variable through which a secret reaches an agent launched by Naurix. Agent config
/// files contain only this *name*, never the value.
pub fn secret_env_var(server_name: &str, secret: &str) -> String {
    let slug: String = server_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("NAURIX_MCP_{slug}_{secret}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(n: &str, v: &str) -> Pair {
        Pair {
            name: n.into(),
            value: v.into(),
        }
    }

    fn stdio() -> McpInput {
        McpInput {
            id: None,
            name: "fs".into(),
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "pkg".into()],
            url: None,
            env: vec![],
            headers: vec![],
            secrets: vec![],
        }
    }

    fn http() -> McpInput {
        McpInput {
            id: None,
            name: "gh".into(),
            transport: Transport::Http,
            command: None,
            args: vec![],
            url: Some("https://api.example.com/mcp".into()),
            env: vec![],
            headers: vec![pair("Authorization", "Bearer ${secret:TOKEN}")],
            secrets: vec!["TOKEN".into()],
        }
    }

    #[test]
    fn accepts_reasonable_servers() {
        assert!(validate_input(&stdio()).is_ok());
        assert!(validate_input(&http()).is_ok());
        let mut local = http();
        local.url = Some("http://localhost:8080/mcp".into());
        assert!(validate_input(&local).is_ok());
    }

    #[test]
    fn rejects_bad_names_commands_and_urls() {
        let t = |f: &dyn Fn(&mut McpInput)| {
            let mut i = stdio();
            f(&mut i);
            assert!(validate_input(&i).is_err());
        };
        t(&|i| i.name = "".into());
        t(&|i| i.name = "has space".into());
        t(&|i| i.name = "-lead".into());
        t(&|i| i.command = Some("./rel".into()));
        t(&|i| i.command = None);
        t(&|i| i.url = Some("https://x.io".into()));
        t(&|i| i.args = vec!["a\0b".into()]);
        for u in [
            "ftp://x",
            "http://example.com/mcp",
            "https://u:pw@host/mcp",
            "https://bad host/",
            "",
        ] {
            let mut h = http();
            h.url = Some(u.into());
            assert!(validate_input(&h).is_err(), "{u:?}");
        }
    }

    #[test]
    fn secrets_must_be_declared_used_and_never_plain() {
        // undeclared reference
        let mut h = http();
        h.secrets.clear();
        assert!(validate_input(&h).is_err());
        // declared but unused
        let mut h = http();
        h.secrets.push("EXTRA".into());
        assert!(validate_input(&h).is_err());
        // plain-text credential header / env
        let mut h = http();
        h.headers = vec![pair("Authorization", "Bearer abc123")];
        h.secrets.clear();
        assert!(validate_input(&h).is_err());
        let mut s = stdio();
        s.env = vec![pair("API_TOKEN", "abc")];
        assert!(validate_input(&s).is_err());
        s.env = vec![pair("API_TOKEN", "${secret:API_TOKEN}")];
        s.secrets = vec!["API_TOKEN".into()];
        assert!(validate_input(&s).is_ok());
        // credentials embedded in an argument
        let mut s = stdio();
        s.args = vec!["postgresql://user:pw@localhost/db".into()];
        assert!(validate_input(&s).is_err());
        s.args = vec!["${secret:DATABASE_URL}".into()];
        s.secrets = vec!["DATABASE_URL".into()];
        assert!(validate_input(&s).is_ok());
        // a connection string with a username but no password is fine
        let mut s = stdio();
        s.args = vec!["postgresql://localhost/mydb".into()];
        assert!(validate_input(&s).is_ok());
        // other ${...} forms are refused (they would depend on the user's shell)
        let mut s = stdio();
        s.args = vec!["${HOME}/x".into()];
        assert!(validate_input(&s).is_err());
        // bad secret names, DYLD, header injection
        let mut s = stdio();
        s.secrets = vec!["lower".into()];
        assert!(validate_input(&s).is_err());
        let mut s = stdio();
        s.env = vec![pair("DYLD_INSERT_LIBRARIES", "/x")];
        assert!(validate_input(&s).is_err());
        let mut h = http();
        h.headers.push(pair("X-A", "v\r\nX-Evil: 1"));
        assert!(validate_input(&h).is_err());
    }

    #[test]
    fn expand_and_placeholders() {
        assert_eq!(
            placeholders("a ${secret:X} b ${secret:Y_1}").unwrap(),
            ["X", "Y_1"]
        );
        assert!(placeholders("${nope}").is_err());
        assert!(placeholders("${secret:X").is_err());
        assert_eq!(
            expand("Bearer ${secret:T}!", &|n| format!("<{n}>")),
            "Bearer <T>!"
        );
        assert_eq!(
            expand("no placeholders", &|_| "x".into()),
            "no placeholders"
        );
    }

    #[test]
    fn fingerprint_changes_with_what_runs_but_not_with_secret_values() {
        let mk = |i: &McpInput| McpServer {
            id: "id".into(),
            name: i.name.clone(),
            transport: i.transport,
            command: i.command.clone(),
            args: i.args.clone(),
            url: i.url.clone(),
            env: i.env.clone(),
            headers: i.headers.clone(),
            secrets: i.secrets.clone(),
            enabled: true,
            approved_fingerprint: None,
            approved_at: None,
            last_status: None,
            last_error: None,
            last_tool_count: None,
            last_tested_at: None,
        };
        let a = mk(&stdio());
        assert_eq!(a.fingerprint(), mk(&stdio()).fingerprint(), "deterministic");
        assert_eq!(a.fingerprint().len(), 64);
        let mut changed = stdio();
        changed.args.push("--extra".into());
        assert_ne!(a.fingerprint(), mk(&changed).fingerprint());
        let mut renamed = stdio();
        renamed.name = "other".into();
        assert_eq!(
            a.fingerprint(),
            mk(&renamed).fingerprint(),
            "the display name does not change what runs"
        );
        let mut ap = a.clone();
        ap.approved_fingerprint = Some(ap.fingerprint());
        assert!(ap.is_approved());
        ap.args.push("x".into());
        assert!(!ap.is_approved(), "editing invalidates approval");
    }

    #[test]
    fn secret_env_var_names_are_namespaced() {
        assert_eq!(
            secret_env_var("my-gh.server", "TOKEN"),
            "NAURIX_MCP_MY_GH_SERVER_TOKEN"
        );
    }
}
