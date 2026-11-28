//! Security Suite core (ADR 0032): the SARIF reader treats a report as hostile input, nothing is kept
//! that the report did not need to show, and comparison is by fingerprint.
use naurix_core::security::{
    compare, parse_report, read_report, Level, SecurityError, Severity, MAX_REPORT_BYTES,
    MAX_RESULTS,
};
use serde_json::{json, Value};
use std::path::Path;

const ROOT: &str = "/home/u/proj";

fn sarif(results: Value, rules: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": { "name": "scanner", "rules": rules } },
            "results": results
        }]
    }))
    .unwrap()
}

fn result(rule: &str, uri: &str, line: u64, msg: &str) -> Value {
    json!({
        "ruleId": rule,
        "message": { "text": msg },
        "locations": [{ "physicalLocation": {
            "artifactLocation": { "uri": uri },
            "region": { "startLine": line, "snippet": { "text": "PLANTED-SNIPPET-MARKER" } }
        }}]
    })
}

fn parse(bytes: &[u8]) -> naurix_core::security::Report {
    parse_report(bytes, Path::new(ROOT)).unwrap()
}

#[test]
fn reads_findings_with_relative_locations_and_no_snippet() {
    let r = parse(&sarif(
        json!([result("R1", "src/a.rs", 12, "Use of eval")]),
        json!([]),
    ));
    assert_eq!(r.tools, vec!["scanner"]);
    let f = &r.findings[0];
    assert_eq!(f.rule_id, "R1");
    assert_eq!(f.message, "Use of eval");
    let loc = f.location.as_ref().unwrap();
    assert_eq!((loc.path.as_str(), loc.line), ("src/a.rs", Some(12)));
    let shown = serde_json::to_string(&r).unwrap();
    assert!(
        !shown.contains("PLANTED-SNIPPET-MARKER"),
        "snippets are never kept"
    );
}

#[test]
fn severity_comes_from_score_then_rule_then_level() {
    let rules = json!([
        { "id": "SCORED", "properties": { "security-severity": "9.1" } },
        { "id": "RULE_ERR", "defaultConfiguration": { "level": "error" } }
    ]);
    let mut own = result("X", "a.rs", 1, "m");
    own["properties"] = json!({ "security-severity": 3.0 });
    let mut note = result("N", "a.rs", 1, "m");
    note["level"] = json!("note");
    let r = parse(&sarif(
        json!([
            result("SCORED", "a.rs", 1, "m"),
            result("RULE_ERR", "a.rs", 1, "m"),
            own,
            note,
            result("PLAIN", "a.rs", 1, "m"),
        ]),
        rules,
    ));
    let got: Vec<(Severity, Level)> = r.findings.iter().map(|f| (f.severity, f.level)).collect();
    assert_eq!(
        got,
        vec![
            (Severity::Critical, Level::Warning),
            (Severity::High, Level::Error),
            (Severity::Low, Level::Warning),
            (Severity::Low, Level::Note),
            (Severity::Medium, Level::Warning),
        ]
    );
}

#[test]
fn rule_index_and_short_description_fill_gaps() {
    let rules = json!([{ "id": "IDX", "shortDescription": { "text": "From the rule" } }]);
    let r = parse(&sarif(json!([{ "ruleIndex": 0 }]), rules));
    assert_eq!(r.findings[0].rule_id, "IDX");
    assert_eq!(r.findings[0].message, "From the rule");
    assert!(r.findings[0].location.is_none());
    assert_eq!(r.paths_dropped, 0, "no location is not a dropped path");
}

#[test]
fn hostile_paths_are_dropped_but_the_finding_is_kept() {
    let r = parse(&sarif(
        json!([
            result("A", "../../etc/passwd", 1, "m"),
            result("B", "file:///etc/shadow", 1, "m"),
            result("C", "https://evil.example/x", 1, "m"),
            result("D", "/home/u/proj/ok.rs", 4, "m"),
        ]),
        json!([]),
    ));
    assert_eq!(r.findings.len(), 4);
    assert_eq!(r.paths_dropped, 3);
    assert_eq!(r.findings[3].location.as_ref().unwrap().path, "ok.rs");
}

#[test]
fn control_characters_are_removed_and_long_text_is_cut() {
    let long = "x".repeat(2_000);
    let r = parse(&sarif(
        json!([result(
            "R\u{1b}[2J",
            "a.rs",
            1,
            &format!("bad\u{7}\u{1b}]0;title\u{7}{long}")
        )]),
        json!([]),
    ));
    let f = &r.findings[0];
    assert_eq!(f.rule_id, "R[2J");
    assert!(!f.message.chars().any(|c| c.is_control()));
    assert_eq!(f.message.chars().count(), 500);
    assert!(f.message.ends_with('…'));
}

#[test]
fn a_secret_in_a_message_is_hidden_and_never_returned() {
    let key = "AKIAIOSFODNN7EXAMPLE";
    let r = parse(&sarif(
        json!([result(
            "secret",
            "a.env",
            1,
            &format!("Found key {key} here")
        )]),
        json!([]),
    ));
    assert_eq!(r.messages_hidden, 1);
    assert_eq!(
        r.findings[0].message,
        "Message hidden: it looks like an AWS access key."
    );
    assert!(!serde_json::to_string(&r).unwrap().contains(key));
}

#[test]
fn results_over_the_limit_are_counted_not_kept() {
    let many: Vec<Value> = (0..MAX_RESULTS + 7)
        .map(|i| result("R", "a.rs", 1, &format!("m{i}")))
        .collect();
    let r = parse(&sarif(Value::Array(many), json!([])));
    assert_eq!(r.findings.len(), MAX_RESULTS);
    assert_eq!(r.not_shown, 7);
}

#[test]
fn wrong_version_shape_and_json_are_refused_without_quoting_content() {
    let err = |b: &[u8]| parse_report(b, Path::new(ROOT)).unwrap_err();
    assert!(matches!(
        err(br#"{"version":"1.0.0-SECRETVERSION","runs":[]}"#),
        SecurityError::UnsupportedVersion
    ));
    assert!(matches!(
        err(br#"{"version":"2.1.0"}"#),
        SecurityError::NotSarif(_)
    ));
    assert!(matches!(err(b"[1,2]"), SecurityError::NotSarif(_)));
    let e = err(b"not json PLANTED");
    assert!(!e.to_string().contains("PLANTED"));
    let deep = "[".repeat(100_000);
    assert!(matches!(err(deep.as_bytes()), SecurityError::NotSarif(_)));
}

#[test]
fn a_missing_version_with_runs_is_accepted() {
    let r = parse(br#"{"runs":[{"tool":{"driver":{}},"results":[{}]}]}"#);
    assert_eq!(r.tools, vec!["unknown tool"]);
    assert_eq!(r.findings[0].rule_id, "unknown rule");
    assert_eq!(r.findings[0].message, "(no message)");
}

#[test]
fn oversized_files_are_refused_before_parsing() {
    let dir = std::env::temp_dir().join(format!(
        "nx-sec-{}-{}",
        std::process::id(),
        naurix_core::persistence::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let big = dir.join("big.sarif");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(MAX_REPORT_BYTES + 1).unwrap();
    assert!(matches!(
        read_report(&big, Path::new(ROOT)),
        Err(SecurityError::TooLarge)
    ));
    let ok = dir.join("ok.sarif");
    std::fs::write(&ok, sarif(json!([result("R", "a.rs", 1, "m")]), json!([]))).unwrap();
    assert_eq!(read_report(&ok, Path::new(ROOT)).unwrap().findings.len(), 1);
    assert!(matches!(
        read_report(&dir.join("missing.sarif"), Path::new(ROOT)),
        Err(SecurityError::Io(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fingerprints_survive_a_line_shift_and_separate_duplicates() {
    let before = parse(&sarif(
        json!([result("R", "a.rs", 10, "m"), result("R", "a.rs", 20, "m")]),
        json!([]),
    ));
    let after = parse(&sarif(
        json!([result("R", "a.rs", 15, "m"), result("R", "a.rs", 25, "m")]),
        json!([]),
    ));
    let fps = |r: &naurix_core::security::Report| {
        r.findings
            .iter()
            .map(|f| f.fingerprint.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(fps(&before), fps(&after));
    assert_ne!(
        before.findings[0].fingerprint,
        before.findings[1].fingerprint
    );
}

#[test]
fn partial_fingerprints_are_used_when_present() {
    let mut a = result("R", "a.rs", 1, "message one");
    a["partialFingerprints"] = json!({ "primaryLocationLineHash": "abc:1" });
    let mut b = result("R", "a.rs", 9, "message two");
    b["partialFingerprints"] = json!({ "primaryLocationLineHash": "abc:1" });
    let r = parse(&sarif(json!([a, b]), json!([])));
    assert_eq!(r.findings[0].fingerprint, r.findings[1].fingerprint);
}

#[test]
fn compare_sorts_new_still_present_and_no_longer_reported() {
    let base = parse(&sarif(
        json!([
            result("KEEP", "a.rs", 1, "m"),
            result("GONE", "b.rs", 1, "m"),
            result("DUP", "c.rs", 1, "m"),
            result("DUP", "c.rs", 2, "m"),
        ]),
        json!([]),
    ));
    let cur = parse(&sarif(
        json!([
            result("KEEP", "a.rs", 7, "m"),
            result("DUP", "c.rs", 3, "m"),
            result("NEW", "d.rs", 1, "m"),
        ]),
        json!([]),
    ));
    let c = compare(&base, &cur);
    fn ids(v: &[naurix_core::security::Finding]) -> Vec<&str> {
        v.iter().map(|f| f.rule_id.as_str()).collect()
    }
    assert_eq!(ids(&c.new), vec!["NEW"]);
    assert_eq!(ids(&c.still_present), vec!["KEEP", "DUP"]);
    assert_eq!(ids(&c.no_longer_reported), vec!["GONE", "DUP"]);
    assert!(!c.incomplete);

    let empty = parse(&sarif(json!([]), json!([])));
    let c = compare(&empty, &cur);
    assert_eq!(
        (
            c.new.len(),
            c.still_present.len(),
            c.no_longer_reported.len()
        ),
        (3, 0, 0)
    );
    let c = compare(&cur, &cur);
    assert_eq!(
        (
            c.new.len(),
            c.still_present.len(),
            c.no_longer_reported.len()
        ),
        (0, 3, 0)
    );
}
