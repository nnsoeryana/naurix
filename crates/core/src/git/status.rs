//! Parser for `git status --porcelain=v2 -z --branch --show-stash`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Changed,
    Renamed,
    Unmerged,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    pub path: String,
    /// Original path for renames/copies.
    pub orig_path: Option<String>,
    pub kind: Kind,
    /// Status letter in the index (`M A D R C T U`), if the index differs from HEAD.
    pub index: Option<char>,
    /// Status letter in the working tree, if it differs from the index.
    pub worktree: Option<char>,
    pub submodule: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Parsed {
    pub head_oid: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub stash: u32,
    pub entries: Vec<FileEntry>,
}

fn letter(c: char) -> Option<char> {
    if c == '.' {
        None
    } else {
        Some(c)
    }
}

pub fn parse(out: &[u8]) -> Parsed {
    let mut p = Parsed::default();
    let mut toks = out
        .split(|b| *b == 0)
        .filter(|t| !t.is_empty())
        .map(|t| String::from_utf8_lossy(t).into_owned());
    while let Some(t) = toks.next() {
        let mut c = t.chars();
        match c.next() {
            Some('#') => {
                let rest = t[1..].trim_start();
                if let Some(v) = rest.strip_prefix("branch.oid ") {
                    p.head_oid = (v != "(initial)").then(|| v.to_string());
                } else if let Some(v) = rest.strip_prefix("branch.head ") {
                    p.detached = v == "(detached)";
                    p.branch = (!p.detached).then(|| v.to_string());
                } else if let Some(v) = rest.strip_prefix("branch.upstream ") {
                    p.upstream = Some(v.to_string());
                } else if let Some(v) = rest.strip_prefix("branch.ab ") {
                    let mut it = v.split_whitespace();
                    p.ahead = it
                        .next()
                        .and_then(|a| a.trim_start_matches('+').parse().ok())
                        .unwrap_or(0);
                    p.behind = it
                        .next()
                        .and_then(|b| b.trim_start_matches('-').parse().ok())
                        .unwrap_or(0);
                } else if let Some(v) = rest.strip_prefix("stash ") {
                    p.stash = v.trim().parse().unwrap_or(0);
                }
            }
            Some('1') => {
                // 1 XY sub mH mI mW hH hI path
                let f: Vec<&str> = t.splitn(9, ' ').collect();
                if f.len() == 9 {
                    p.entries.push(entry(f[8], None, f[1], f[2], Kind::Changed));
                }
            }
            Some('2') => {
                // 2 XY sub mH mI mW hH hI Xscore path  (then the original path as the next token)
                let f: Vec<&str> = t.splitn(10, ' ').collect();
                let orig = toks.next();
                if f.len() == 10 {
                    p.entries.push(entry(f[9], orig, f[1], f[2], Kind::Renamed));
                }
            }
            Some('u') => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let f: Vec<&str> = t.splitn(11, ' ').collect();
                if f.len() == 11 {
                    p.entries
                        .push(entry(f[10], None, f[1], f[2], Kind::Unmerged));
                }
            }
            Some('?') => p.entries.push(FileEntry {
                path: t[2..].to_string(),
                orig_path: None,
                kind: Kind::Untracked,
                index: None,
                worktree: Some('?'),
                submodule: false,
            }),
            _ => {} // ignored files ('!') or anything unknown
        }
    }
    p
}

fn entry(path: &str, orig: Option<String>, xy: &str, sub: &str, kind: Kind) -> FileEntry {
    let mut c = xy.chars();
    let (x, y) = (c.next().unwrap_or('.'), c.next().unwrap_or('.'));
    FileEntry {
        path: path.to_string(),
        orig_path: orig,
        kind,
        index: letter(x),
        worktree: letter(y),
        submodule: sub.starts_with('S'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(parts: &[&str]) -> Vec<u8> {
        let mut v = parts.join("\0").into_bytes();
        v.push(0);
        v
    }

    #[test]
    fn branch_header_with_upstream_and_stash() {
        let p = parse(&z(&[
            "# branch.oid abc123",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -5",
            "# stash 3",
        ]));
        assert_eq!(
            (
                p.head_oid.as_deref(),
                p.branch.as_deref(),
                p.upstream.as_deref(),
                p.ahead,
                p.behind,
                p.stash,
                p.detached
            ),
            (
                Some("abc123"),
                Some("main"),
                Some("origin/main"),
                2,
                5,
                3,
                false
            )
        );
    }

    #[test]
    fn detached_and_initial_commit() {
        let p = parse(&z(&["# branch.oid (initial)", "# branch.head (detached)"]));
        assert!(p.detached && p.branch.is_none() && p.head_oid.is_none());
        let p = parse(&z(&["# branch.oid (initial)", "# branch.head main"]));
        assert_eq!(
            (p.branch.as_deref(), p.head_oid.clone()),
            (Some("main"), None),
            "a new repo with no commits"
        );
    }

    #[test]
    fn ordinary_renamed_unmerged_and_untracked_entries() {
        let h = "0000000000000000000000000000000000000000";
        let one = format!("1 MM N... 100644 100644 100644 {h} {h} src/a b.rs");
        let two = format!("2 R. N... 100644 100644 100644 {h} {h} R100 new name.rs");
        let un = format!("u UU N... 100644 100644 100644 100644 {h} {h} {h} conflict.txt");
        let sub = format!("1 .M S.M. 160000 160000 160000 {h} {h} vendor/lib");
        let p = parse(&z(&[
            "# branch.head main",
            &one,
            &two,
            "old name.rs",
            &un,
            &sub,
            "? new.txt",
            "? dir/",
            "! ignored.log",
        ]));
        assert_eq!(p.entries.len(), 6);
        assert_eq!(
            p.entries[0],
            FileEntry {
                path: "src/a b.rs".into(),
                orig_path: None,
                kind: Kind::Changed,
                index: Some('M'),
                worktree: Some('M'),
                submodule: false
            }
        );
        assert_eq!(
            (
                p.entries[1].path.as_str(),
                p.entries[1].orig_path.as_deref(),
                p.entries[1].kind,
                p.entries[1].index,
                p.entries[1].worktree
            ),
            (
                "new name.rs",
                Some("old name.rs"),
                Kind::Renamed,
                Some('R'),
                None
            )
        );
        assert_eq!(
            (p.entries[2].kind, p.entries[2].index, p.entries[2].worktree),
            (Kind::Unmerged, Some('U'), Some('U'))
        );
        assert!(
            p.entries[3].submodule
                && p.entries[3].index.is_none()
                && p.entries[3].worktree == Some('M')
        );
        assert_eq!(
            (p.entries[4].path.as_str(), p.entries[4].kind),
            ("new.txt", Kind::Untracked)
        );
        assert_eq!(p.entries[5].path, "dir/");
    }

    #[test]
    fn hostile_names_and_garbage_do_not_break_parsing() {
        let h = "0".repeat(40);
        let weird = format!("1 .M N... 100644 100644 100644 {h} {h} we\nird \"quoted\" ü名.txt");
        let p = parse(&z(&[
            &weird,
            "1 too short",
            "2 R. N... a b",
            "garbage",
            "? ",
        ]));
        assert_eq!(p.entries[0].path, "we\nird \"quoted\" ü名.txt");
        assert!(parse(b"").entries.is_empty());
        assert!(parse(&[0, 0, 0]).entries.is_empty());
    }
}
