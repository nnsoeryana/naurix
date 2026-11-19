//! Heuristic check for values that look like credentials (ADR 0019).
//!
//! Naurix stores no credentials. Anything the user types into a field that is persisted in SQLite is
//! checked here and refused if it looks like a token, key or password. This is a best-effort guard, not
//! a guarantee: a credential in a harmless-looking shape can still pass, so the UI also tells users not
//! to enter secrets.

/// Well-known token prefixes (checked case-sensitively at the start of the value or after `Bearer `).
const PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "sk-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "xoxa-",
    "xoxb-",
    "xoxp-",
    "xoxr-",
    "xoxs-",
    "AKIA",
    "ASIA",
    "AIza",
    "npm_",
    "pypi-",
    "SG.",
    "shpat_",
    "dop_v1_",
    "hf_",
];

/// `scheme://user:password@host`.
fn has_url_password(s: &str) -> bool {
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        let after = &rest[i + 3..];
        let authority = after.split(['/', '?', '#', ' ']).next().unwrap_or("");
        if let Some(at) = authority.rfind('@') {
            if authority[..at].contains(':') {
                return true;
            }
        }
        rest = after;
    }
    false
}

fn entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let n = s.chars().count() as f64;
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum()
}

fn token_shaped(s: &str) -> bool {
    s.len() >= 32
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '=' | '.'))
        && s.chars().any(|c| c.is_ascii_digit())
        && s.chars().any(|c| c.is_ascii_alphabetic())
        && entropy(s) >= 3.5
}

/// True if `value` looks like a credential: a known token prefix, a private key block, a JWT, a password
/// inside a URL, or a long random-looking token.
pub fn looks_like_credential(value: &str) -> bool {
    let v = value.trim();
    let v = v
        .strip_prefix("Bearer ")
        .or_else(|| v.strip_prefix("Basic "))
        .unwrap_or(v);
    if v.contains("-----BEGIN") && v.contains("PRIVATE KEY") {
        return true;
    }
    if has_url_password(v) {
        return true;
    }
    if v.starts_with("eyJ") && v.matches('.').count() == 2 && v.len() > 30 {
        return true;
    }
    if PREFIXES.iter().any(|p| v.starts_with(p)) && v.len() >= p_min(v) {
        return true;
    }
    // Each whitespace-separated piece or `key=value` tail can carry a token (e.g. `--token=abc…`).
    v.split(|c: char| c.is_whitespace() || c == '=' || c == ':')
        .any(token_shaped)
}

fn p_min(v: &str) -> usize {
    // `sk-` and `hf_` are short; require a plausible length so ordinary words do not match.
    if v.starts_with("sk-")
        || v.starts_with("hf_")
        || v.starts_with("SG.")
        || v.starts_with("AKIA")
        || v.starts_with("ASIA")
    {
        16
    } else {
        10
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_credential_shapes() {
        for v in [
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "github_pat_11ABCDEFG0abcdefghijkl",
            "sk-proj-abcdefghijklmnop",
            "xoxb-123456789012-abcdefghijkl",
            "AKIAIOSFODNN7EXAMPLE",
            "Bearer abcdEFGH1234ijklMNOP5678qrstUVWX90",
            "postgres://user:hunter2@db.example.com/app",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abc123def456",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc",
            "--token=Zq8dK2mP9xV4nB7cL1wR6tY3uH5jF0aS",
        ] {
            assert!(looks_like_credential(v), "should flag: {v}");
        }
    }

    #[test]
    fn allows_ordinary_values() {
        for v in [
            "1",
            "true",
            "en_US.UTF-8",
            "/Users/me/projects/my-app",
            "https://example.com/mcp",
            "--model",
            "gpt-4o",
            "development",
            "/opt/homebrew/bin:/usr/local/bin",
            "sk-learn",
            "https://user@example.com/repo.git",
            "my-very-long-but-plain-directory-name-here",
        ] {
            assert!(!looks_like_credential(v), "should allow: {v}");
        }
    }
}
