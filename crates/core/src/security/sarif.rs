//! Bounded SARIF 2.1.0 reader (ADR 0032 §4). The report is untrusted: it is size- and count-limited,
//! every string is cleaned, snippets and code flows are never read, and a path is kept only when it is
//! lexically inside the workspace. The file system is never consulted for paths, so symlinks in a
//! report cannot point the view anywhere.

use super::finding::{
    clean_text, fingerprint, Finding, Level, Location, Severity, MAX_MESSAGE_CHARS, MAX_PATH_CHARS,
    MAX_RULE_CHARS, MAX_TOOL_CHARS,
};
use super::SecurityError;
use crate::memory::secrets;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub const MAX_REPORT_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_RESULTS: usize = 5_000;
const MAX_PARTIAL_FINGERPRINTS: usize = 16;
const MAX_FINGERPRINT_PART_CHARS: usize = 200;

/// A report as shown. Lives only as long as the view that asked for it; never stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Tool names in the order they appear.
    pub tools: Vec<String>,
    pub findings: Vec<Finding>,
    /// Results beyond [`MAX_RESULTS`]: counted, not kept.
    pub not_shown: usize,
    /// Results whose path was outside the workspace or unusable. They are kept, without a location.
    pub paths_dropped: usize,
    /// Messages replaced because they looked like they held a secret.
    pub messages_hidden: usize,
}

/// Read and parse the report at `path` (a file the user chose). The size is checked before reading
/// and enforced while reading.
pub fn read_report(path: &Path, workspace_root: &Path) -> Result<Report, SecurityError> {
    let io = |e: std::io::Error| SecurityError::Io(e.kind().to_string());
    let file = std::fs::File::open(path).map_err(io)?;
    if file.metadata().map_err(io)?.len() > MAX_REPORT_BYTES {
        return Err(SecurityError::TooLarge);
    }
    let mut buf = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(io)?;
    parse_report(&buf, workspace_root)
}

pub fn parse_report(bytes: &[u8], workspace_root: &Path) -> Result<Report, SecurityError> {
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(SecurityError::TooLarge);
    }
    // serde_json limits nesting depth, so a deeply nested document fails here instead of overflowing.
    let doc: Value = serde_json::from_slice(bytes)
        .map_err(|_| SecurityError::NotSarif("it is not valid JSON"))?;
    let top = doc
        .as_object()
        .ok_or(SecurityError::NotSarif("the top level is not an object"))?;
    match top.get("version") {
        None => {}
        Some(Value::String(v)) if v == "2.1.0" => {}
        Some(_) => return Err(SecurityError::UnsupportedVersion),
    }
    let runs = top
        .get("runs")
        .and_then(Value::as_array)
        .ok_or(SecurityError::NotSarif("it has no list of runs"))?;

    let root = components(&workspace_root.to_string_lossy());
    let mut report = Report::default();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for run in runs {
        let tool = run
            .pointer("/tool/driver/name")
            .and_then(Value::as_str)
            .map(|s| clean_text(s, MAX_TOOL_CHARS))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown tool".to_string());
        if !report.tools.contains(&tool) {
            report.tools.push(tool.clone());
        }
        let rules: &[Value] = run
            .pointer("/tool/driver/rules")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let by_id: HashMap<&str, &Value> = rules
            .iter()
            .filter_map(|r| Some((r.get("id")?.as_str()?, r)))
            .collect();
        let Some(results) = run.get("results").and_then(Value::as_array) else {
            continue;
        };
        for result in results {
            if report.findings.len() >= MAX_RESULTS {
                report.not_shown += 1;
                continue;
            }
            let ctx = Ctx {
                tool: &tool,
                rules,
                by_id: &by_id,
                root: root.as_deref(),
            };
            let f = finding(&ctx, result, &mut report, &mut seen);
            report.findings.push(f);
        }
    }
    Ok(report)
}

struct Ctx<'a> {
    tool: &'a str,
    rules: &'a [Value],
    by_id: &'a HashMap<&'a str, &'a Value>,
    root: Option<&'a [String]>,
}

fn finding(
    ctx: &Ctx<'_>,
    result: &Value,
    report: &mut Report,
    seen: &mut HashMap<String, usize>,
) -> Finding {
    let id_given = result
        .get("ruleId")
        .and_then(Value::as_str)
        .or_else(|| str_at(result, "/rule/id"));
    let rule = result
        .get("ruleIndex")
        .or_else(|| result.pointer("/rule/index"))
        .and_then(Value::as_u64)
        .and_then(|i| ctx.rules.get(usize::try_from(i).ok()?))
        .or_else(|| id_given.and_then(|id| ctx.by_id.get(id).copied()));
    let rule_id = clean_text(
        id_given
            .or_else(|| rule.and_then(|r| str_at(r, "/id")))
            .unwrap_or(""),
        MAX_RULE_CHARS,
    );
    let rule_id = if rule_id.is_empty() {
        "unknown rule".to_string()
    } else {
        rule_id
    };

    let level = result
        .get("level")
        .and_then(Value::as_str)
        .and_then(Level::parse)
        .or_else(|| {
            rule.and_then(|r| str_at(r, "/defaultConfiguration/level"))
                .and_then(Level::parse)
        })
        .unwrap_or(Level::Warning);
    let severity = score(result)
        .or_else(|| rule.and_then(score))
        .unwrap_or_else(|| level.severity());

    let raw_message = str_at(result, "/message/text")
        .or_else(|| rule.and_then(|r| str_at(r, "/shortDescription/text")))
        .unwrap_or("");
    let message = match secrets::scan(raw_message) {
        Some(found) => {
            report.messages_hidden += 1;
            format!("Message hidden: it looks like {}.", found.kind)
        }
        None => {
            let m = clean_text(raw_message, MAX_MESSAGE_CHARS);
            if m.is_empty() {
                "(no message)".to_string()
            } else {
                m
            }
        }
    };

    let physical = result.pointer("/locations/0/physicalLocation");
    let uri = physical.and_then(|p| str_at(p, "/artifactLocation/uri"));
    let location = uri.and_then(|u| match confine(u, ctx.root) {
        Some(path) => Some(Location {
            path,
            line: physical
                .and_then(|p| p.pointer("/region/startLine"))
                .and_then(Value::as_u64)
                .filter(|l| *l >= 1)
                .and_then(|l| u32::try_from(l).ok()),
        }),
        None => {
            report.paths_dropped += 1;
            None
        }
    });

    let path = location.as_ref().map_or("", |l| l.path.as_str());
    let partial = partial_fingerprints(result);
    let fp = if partial.is_empty() {
        // No tool fingerprint: tell identical findings in one file apart by their order.
        let base = fingerprint(&["m", ctx.tool, &rule_id, path, &message]);
        let n = seen.entry(base).or_insert(0);
        *n += 1;
        fingerprint(&["m", ctx.tool, &rule_id, path, &message, &n.to_string()])
    } else {
        let mut parts = vec!["p", ctx.tool, rule_id.as_str(), path];
        parts.extend(partial.iter().map(String::as_str));
        fingerprint(&parts)
    };

    Finding {
        tool: ctx.tool.to_string(),
        rule_id,
        severity,
        level,
        location,
        message,
        fingerprint: fp,
    }
}

fn str_at<'v>(v: &'v Value, pointer: &str) -> Option<&'v str> {
    v.pointer(pointer).and_then(Value::as_str)
}

/// `properties.security-severity` as a number or a numeric string.
fn score(v: &Value) -> Option<Severity> {
    let s = v.pointer("/properties/security-severity")?;
    let n = s
        .as_f64()
        .or_else(|| s.as_str().and_then(|t| t.trim().parse().ok()))?;
    Severity::from_score(n)
}

fn partial_fingerprints(result: &Value) -> Vec<String> {
    let Some(map) = result.get("partialFingerprints").and_then(Value::as_object) else {
        return vec![];
    };
    let mut v: Vec<String> = map
        .iter()
        .take(MAX_PARTIAL_FINGERPRINTS)
        .filter_map(|(k, v)| {
            Some(format!(
                "{}={}",
                clean_text(k, MAX_FINGERPRINT_PART_CHARS),
                clean_text(v.as_str()?, MAX_FINGERPRINT_PART_CHARS)
            ))
        })
        .collect();
    v.sort();
    v
}

/// The workspace-relative, `/`-separated path for a SARIF artifact URI, or `None` when it is outside
/// the workspace, uses another scheme or host, climbs with `..`, or is otherwise unusable. Lexical only.
fn confine(uri: &str, root: Option<&[String]>) -> Option<String> {
    if uri.len() > MAX_PATH_CHARS * 4 {
        return None;
    }
    let decoded = percent_decode(uri);
    let (rest, from_file_uri) = match decoded.strip_prefix("file://") {
        Some(r) => {
            let r = r.strip_prefix("localhost").unwrap_or(r);
            if !r.starts_with('/') {
                return None; // another host
            }
            // `file:///C:/x` names the Windows path `C:/x`.
            let r = match r.get(1..) {
                Some(t) if has_drive(t) => t,
                _ => r,
            };
            (r.to_string(), true)
        }
        None if has_scheme(&decoded) => return None,
        None => (decoded, false),
    };
    let parts = components(&rest)?;
    let rel = if from_file_uri || is_absolute(&rest) {
        let root = root?;
        if parts.len() <= root.len() || !root.iter().zip(&parts).all(|(a, b)| same_part(a, b)) {
            return None;
        }
        parts[root.len()..].to_vec()
    } else {
        parts
    };
    if rel.is_empty() {
        return None;
    }
    let s = rel.join("/");
    if s.chars().count() > MAX_PATH_CHARS || s.chars().any(char::is_control) {
        return None;
    }
    Some(s)
}

/// Path parts split on `/` and `\`, without empty and `.` parts. `None` if any part is `..`.
fn components(s: &str) -> Option<Vec<String>> {
    let mut out = vec![];
    for p in s.split(['/', '\\']) {
        match p {
            "" | "." => {}
            ".." => return None,
            p => out.push(p.to_string()),
        }
    }
    Some(out)
}

fn has_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn is_absolute(s: &str) -> bool {
    s.starts_with('/') || s.starts_with('\\') || has_drive(s)
}

/// A URI scheme such as `https:`. At least two characters, so a drive letter is not a scheme.
fn has_scheme(s: &str) -> bool {
    match s.find(':') {
        Some(i) if i >= 2 => {
            let scheme = &s[..i];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        }
        _ => false,
    }
}

/// Equal parts; a drive letter compares without case.
fn same_part(a: &str, b: &str) -> bool {
    a == b || (a.len() == 2 && has_drive(a) && a.eq_ignore_ascii_case(b))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> Vec<String> {
        components("/home/u/proj").unwrap()
    }

    fn c(uri: &str) -> Option<String> {
        confine(uri, Some(&root()))
    }

    #[test]
    fn relative_paths_are_kept_and_normalised() {
        assert_eq!(c("src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(c("./src//a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(c("src/a%20b.rs").as_deref(), Some("src/a b.rs"));
    }

    #[test]
    fn climbing_and_foreign_paths_are_dropped() {
        for bad in [
            "../etc/passwd",
            "src/../../x",
            "%2e%2e/x",
            "src\\..\\..\\x",
            "/etc/passwd",
            "/home/u/projX/a.rs",
            "/home/u/proj",
            "file:///etc/passwd",
            "file://evil.example/home/u/proj/a.rs",
            "https://example.com/a.rs",
            "C:\\Windows\\x",
            "src/a%00b.rs",
            "",
        ] {
            assert_eq!(c(bad), None, "{bad}");
        }
    }

    #[test]
    fn absolute_paths_inside_the_root_become_relative() {
        assert_eq!(c("/home/u/proj/src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(
            c("file:///home/u/proj/src/a.rs").as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(
            c("file://localhost/home/u/proj/a.rs").as_deref(),
            Some("a.rs")
        );
        let win = components("C:\\work\\proj").unwrap();
        assert_eq!(
            confine("file:///c:/work/proj/src/a.rs", Some(&win)).as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(confine("/home/u/proj/a.rs", None), None);
    }

    #[test]
    fn percent_decode_leaves_bad_escapes() {
        assert_eq!(percent_decode("a%2Fb%zz%4"), "a/b%zz%4");
    }
}
