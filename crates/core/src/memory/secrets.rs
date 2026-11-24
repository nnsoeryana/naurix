//! A safety net against saving credentials as memory (ADR 0031 §4). It looks for well-known token
//! shapes and for secret-looking assignments, and reports the **line and kind only**, never the value.
//! It is heuristic: it can refuse ordinary text and it can miss a secret.

/// What looked like a secret, and where. Never holds the secret itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-based line number.
    pub line: usize,
    pub kind: &'static str,
}

const NAMES: [&str; 8] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "private_key",
];

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-+/=.".contains(c)
}

fn all(s: &str, f: impl Fn(char) -> bool) -> bool {
    s.chars().all(f)
}

/// A token shape from a well-known provider.
fn token_kind(w: &str) -> Option<&'static str> {
    let alnum = |c: char| c.is_ascii_alphanumeric();
    let url_safe = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if w.starts_with("AKIA")
        && w.len() >= 20
        && all(&w[4..20], |c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some("an AWS access key");
    }
    for p in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        if let Some(rest) = w.strip_prefix(p) {
            if rest.len() >= 36 && all(rest, alnum) {
                return Some("a GitHub token");
            }
        }
    }
    if w.strip_prefix("github_pat_")
        .is_some_and(|r| r.len() >= 22 && all(r, url_safe))
    {
        return Some("a GitHub token");
    }
    if w.strip_prefix("sk-")
        .is_some_and(|r| r.len() >= 20 && all(r, url_safe))
    {
        return Some("an API key");
    }
    if w.len() >= 14
        && w.starts_with("xox")
        && "baprs".contains(&w[3..4])
        && w[4..].starts_with('-')
    {
        return Some("a Slack token");
    }
    if w.strip_prefix("AIza")
        .is_some_and(|r| r.len() >= 35 && all(&r[..35], url_safe))
    {
        return Some("a Google API key");
    }
    let parts: Vec<&str> = w.split('.').collect();
    if parts.len() == 3
        && parts[0].starts_with("eyJ")
        && parts.iter().all(|p| p.len() >= 8 && all(p, url_safe))
    {
        return Some("a JSON web token");
    }
    None
}

/// A value that is long, has letters and digits and no spaces: the shape of a generated secret.
fn looks_random(v: &str) -> bool {
    v.len() >= 16
        && v.chars().any(|c| c.is_ascii_digit())
        && v.chars().any(|c| c.is_ascii_alphabetic())
        && !v.contains(['(', ')', '{', '}', ';'])
}

/// `name = value` or `name: value` where the name suggests a secret and the value looks generated.
fn assignment(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    for name in NAMES {
        let mut from = 0;
        while let Some(i) = lower[from..].find(name) {
            let end = from + i + name.len();
            from = end;
            let rest = lower[end..]
                .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let rest = rest.trim_start_matches(['"', '\'']).trim_start();
            let Some(after) = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')) else {
                continue;
            };
            // Use the original text for the value so case is kept (only its shape matters).
            let offset = line.len() - after.len();
            let value = line[offset..].trim().trim_start_matches(['"', '\'']);
            let value = value
                .split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
                .next()
                .unwrap_or("");
            if looks_random(value) {
                return true;
            }
        }
    }
    false
}

/// The first likely secret in `text`, or `None`.
pub fn scan(text: &str) -> Option<Finding> {
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
            return Some(Finding {
                line: n,
                kind: "a private key",
            });
        }
        for w in line.split(|c: char| !is_word_char(c)) {
            if let Some(kind) = token_kind(w.trim_matches(['.', '=', '/', '+'])) {
                return Some(Finding { line: n, kind });
            }
        }
        if assignment(line) {
            return Some(Finding {
                line: n,
                kind: "a password, token or key assignment",
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(t: &str) -> Option<&'static str> {
        scan(t).map(|f| f.kind)
    }

    #[test]
    fn well_known_shapes_are_found_with_their_line_and_never_their_value() {
        let cases = [
            ("-----BEGIN OPENSSH PRIVATE KEY-----", "a private key"),
            ("aws AKIAIOSFODNN7EXAMPLE here", "an AWS access key"),
            ("ghp_abcdefghijklmnopqrstuvwxyz0123456789", "a GitHub token"),
            (
                "github_pat_11ABCDEFG0123456789abcd_efghijk",
                "a GitHub token",
            ),
            ("key sk-ant-api03-abcdefghijklmnopqrstuvwx", "an API key"),
            ("xoxb-123456789012-abcdefghij", "a Slack token"),
            (
                "AIzaSyA-1234567890abcdefghijklmnopqrstuv",
                "a Google API key",
            ),
            (
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijk_lmnop",
                "a JSON web token",
            ),
        ];
        for (text, want) in cases {
            let f = scan(&format!("first line\n{text}")).unwrap_or_else(|| panic!("missed {want}"));
            assert_eq!((f.kind, f.line), (want, 2));
            assert!(
                !format!("{f:?}").contains(text.split(' ').next_back().unwrap()),
                "the finding must not hold the secret"
            );
        }
    }

    #[test]
    fn secret_assignments_are_found_in_common_spellings() {
        for t in [
            "password = Xk29fjA0sd9Qw8e7r6",
            "API_KEY: 'a8f5f167f44f4964e6c998dee827110c'",
            "export SECRET_TOKEN=\"9sd8f7g6h5j4k3l2m1n0\"",
            "db_password:   hunter2hunter2hunter2",
            "{\"apiKey\": \"abcd1234efgh5678ijkl\"}",
        ] {
            assert_eq!(kind(t), Some("a password, token or key assignment"), "{t}");
        }
    }

    #[test]
    fn ordinary_notes_are_not_refused() {
        for t in [
            "We rotate the password every 90 days; ask ops for the token.",
            "Use `let token = next_token();` to advance the lexer.",
            "const apiKeyName = \"api_key_name_for_header\";",
            "Deploy commit 3f786850e387550fdab836ed7e6dc881de23001b to staging.",
            "password: correct horse battery staple",
            "secret = short1",
            "Run `cargo test -p naurix-core` before every push.",
            "The key idea: keep the engine pure. Tokens: AKIA is a prefix, sk- too.",
            "## Decisions\n- Use SQLite\n- Search with FTS5\n",
        ] {
            assert_eq!(scan(t), None, "{t}");
        }
    }
}
