//! Read-only Git status/diff against real repositories, including hostile ones.
use naurix_core::git::{self, Area, Kind, RepoStatus, StatusResult};
use naurix_core::persistence::{new_id, Db};
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("at-git-{tag}-{}-{}", std::process::id(), new_id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

/// Test-setup git: isolated from the developer's own global config.
fn g(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "protocol.file.allow=always",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn repo(tag: &str) -> PathBuf {
    let r = tmp(tag);
    g(&r, &["init", "-q", "-b", "main"]);
    write(&r.join("a.txt"), "one\n");
    write(&r.join("dir/b.txt"), "two\n");
    g(&r, &["add", "-A"]);
    g(&r, &["commit", "-q", "-m", "init"]);
    r
}

fn never(_: &Path) -> bool {
    false
}

fn st(dir: &Path) -> RepoStatus {
    match git::status(dir, &never).unwrap() {
        StatusResult::Repo(s) => *s,
        other => panic!("expected a repository, got {other:?}"),
    }
}

fn entry<'a>(s: &'a RepoStatus, path: &str) -> &'a git::FileEntry {
    s.entries
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| {
            panic!(
                "{path} not in {:?}",
                s.entries.iter().map(|e| &e.path).collect::<Vec<_>>()
            )
        })
}

#[test]
fn non_repositories_and_subfolders() {
    let plain = tmp("plain");
    assert!(matches!(
        git::status(&plain, &never).unwrap(),
        StatusResult::NotRepo
    ));
    assert_eq!(git::repo_root(&plain).unwrap(), None);
    let r = repo("sub");
    let s = st(&r.join("dir"));
    assert_eq!(
        s.root,
        r.to_string_lossy(),
        "a workspace inside a repository reports the repository root"
    );
    assert_eq!(git::repo_root(&r.join("dir")).unwrap(), Some(r.clone()));
    // a bare repository has no work tree
    let bare = tmp("bare");
    g(&bare, &["init", "-q", "--bare", "-b", "main"]);
    assert!(matches!(
        git::status(&bare, &never).unwrap(),
        StatusResult::NotRepo
    ));
}

#[test]
fn clean_repository_and_a_repository_with_no_commits() {
    let r = repo("clean");
    let s = st(&r);
    assert_eq!(
        (s.branch.as_deref(), s.detached, s.operation, s.stash),
        (Some("main"), false, None, 0)
    );
    assert!(s.entries.is_empty() && s.counts == git::Counts::default() && !s.truncated);
    assert_eq!(
        s.head_oid.as_deref(),
        Some(g(&r, &["rev-parse", "HEAD"]).as_str())
    );
    let empty = tmp("empty");
    g(&empty, &["init", "-q", "-b", "trunk"]);
    write(&empty.join("x"), "x");
    let s = st(&empty);
    assert_eq!(
        (s.branch.as_deref(), s.head_oid.clone()),
        (Some("trunk"), None)
    );
    assert_eq!(s.counts.untracked, 1);
}

#[test]
fn staged_unstaged_both_added_deleted_renamed_untracked_and_awkward_names() {
    let r = repo("states");
    write(&r.join("gone.txt"), "g\n");
    g(&r, &["add", "gone.txt"]);
    g(&r, &["commit", "-q", "-m", "add gone"]);
    write(&r.join("a.txt"), "one changed\n"); // unstaged
    write(&r.join("dir/b.txt"), "two changed\n");
    g(&r, &["add", "dir/b.txt"]); // staged
    write(&r.join("dir/b.txt"), "two changed again\n"); // staged + unstaged (MM)
    write(&r.join("new.txt"), "n\n");
    g(&r, &["add", "new.txt"]); // added
    std::fs::remove_file(r.join("gone.txt")).unwrap(); // deleted (unstaged)
    g(&r, &["mv", "a.txt", "renamed.txt"]); // staged rename
    write(&r.join("untracked dir/inner file.txt"), "u\n");
    write(&r.join("sp ace.txt"), "s\n");
    write(&r.join("ünï\u{540d}.txt"), "u\n");
    write(&r.join("new\nline.txt"), "nl\n");

    let s = st(&r);
    let b = entry(&s, "dir/b.txt");
    assert_eq!(
        (b.kind, b.index, b.worktree),
        (Kind::Changed, Some('M'), Some('M'))
    );
    let n = entry(&s, "new.txt");
    assert_eq!((n.index, n.worktree), (Some('A'), None));
    let d = entry(&s, "gone.txt");
    assert_eq!((d.index, d.worktree), (None, Some('D')));
    let rn = entry(&s, "renamed.txt");
    assert_eq!(
        (rn.kind, rn.orig_path.as_deref(), rn.index),
        (Kind::Renamed, Some("a.txt"), Some('R'))
    );
    assert_eq!(entry(&s, "untracked dir/").kind, Kind::Untracked);
    for name in ["sp ace.txt", "ünï\u{540d}.txt", "new\nline.txt"] {
        assert_eq!(entry(&s, name).kind, Kind::Untracked, "{name:?}");
    }
    assert_eq!(
        s.counts.untracked, 4,
        "the directory, the space name, the unicode name and the newline name"
    );
    assert_eq!(s.counts.staged, 3, "b.txt (index), new.txt, renamed.txt");
    assert_eq!(
        s.counts.unstaged, 3,
        "b.txt (worktree), gone.txt, and renamed.txt (renamed after being edited)"
    );
    let rn2 = entry(&s, "renamed.txt");
    assert_eq!((rn2.index, rn2.worktree), (Some('R'), Some('M')));
    assert_eq!(s.counts.conflicted, 0);
}

#[test]
fn detached_head_stash_and_upstream_ahead_behind() {
    let r = repo("detached");
    g(&r, &["checkout", "-q", "--detach"]);
    let s = st(&r);
    assert!(s.detached && s.branch.is_none() && s.head_oid.is_some());
    g(&r, &["checkout", "-q", "main"]);
    write(&r.join("a.txt"), "stash me\n");
    g(&r, &["stash", "-q"]);
    assert_eq!(st(&r).stash, 1);

    let remote = tmp("remote");
    g(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let a = tmp("clone-a");
    g(&a, &["clone", "-q", remote.to_str().unwrap(), "."]);
    write(&a.join("f"), "1");
    g(&a, &["add", "-A"]);
    g(&a, &["commit", "-q", "-m", "one"]);
    g(&a, &["push", "-q", "origin", "HEAD:main"]);
    g(
        &a,
        &["branch", "-q", "--set-upstream-to=origin/main", "main"],
    );
    let b = tmp("clone-b");
    g(&b, &["clone", "-q", remote.to_str().unwrap(), "."]);
    write(&b.join("g"), "2");
    g(&b, &["add", "-A"]);
    g(&b, &["commit", "-q", "-m", "two"]);
    g(&b, &["push", "-q", "origin", "HEAD:main"]);
    write(&a.join("h"), "3");
    g(&a, &["add", "-A"]);
    g(&a, &["commit", "-q", "-m", "local"]);
    g(&a, &["fetch", "-q"]);
    let s = st(&a);
    assert_eq!(
        (s.upstream.as_deref(), s.ahead, s.behind),
        (Some("origin/main"), 1, 1)
    );
}

#[test]
fn merge_conflicts_and_rebase_in_progress_are_reported() {
    let r = repo("conflict");
    g(&r, &["checkout", "-q", "-b", "feature"]);
    write(&r.join("a.txt"), "feature side\n");
    g(&r, &["commit", "-qam", "feature"]);
    g(&r, &["checkout", "-q", "main"]);
    write(&r.join("a.txt"), "main side\n");
    g(&r, &["commit", "-qam", "main"]);
    let _ = Command::new("git")
        .arg("-C")
        .arg(&r)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.com",
            "merge",
            "feature",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    let s = st(&r);
    assert_eq!(s.operation, Some("merging"));
    assert_eq!(s.counts.conflicted, 1);
    let c = entry(&s, "a.txt");
    assert_eq!(
        (c.kind, c.index, c.worktree),
        (Kind::Unmerged, Some('U'), Some('U'))
    );
    g(&r, &["merge", "--abort"]);
    assert_eq!(st(&r).operation, None);

    g(&r, &["checkout", "-q", "feature"]);
    let _ = Command::new("git")
        .arg("-C")
        .arg(&r)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.com",
            "rebase",
            "main",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert_eq!(st(&r).operation, Some("rebasing"));
}

#[test]
fn very_large_status_is_capped_but_counted() {
    let r = repo("many");
    for i in 0..(git::MAX_ENTRIES + 50) {
        std::fs::write(r.join(format!("f{i:05}")), "x").unwrap();
    }
    let s = st(&r);
    assert_eq!(s.entries.len(), git::MAX_ENTRIES);
    assert_eq!(s.counts.untracked, git::MAX_ENTRIES + 50);
    assert!(s.truncated);
}

#[test]
fn diffs_for_every_area_binary_large_and_invalid_paths() {
    let r = repo("diff");
    write(&r.join("a.txt"), "one\nadded line\n");
    let d = git::diff(&r, "a.txt", None, Area::Unstaged, &never).unwrap();
    assert!(
        d.text.contains("+added line") && !d.binary && !d.truncated,
        "{}",
        d.text
    );
    g(&r, &["add", "a.txt"]);
    assert!(
        git::diff(&r, "a.txt", None, Area::Unstaged, &never)
            .unwrap()
            .text
            .is_empty(),
        "nothing unstaged any more"
    );
    assert!(git::diff(&r, "a.txt", None, Area::Staged, &never)
        .unwrap()
        .text
        .contains("+added line"));

    write(&r.join("fresh file.txt"), "brand new\n");
    let u = git::diff(&r, "fresh file.txt", None, Area::Untracked, &never).unwrap();
    assert!(
        u.text.contains("new file mode") && u.text.contains("+brand new"),
        "{}",
        u.text
    );

    g(&r, &["mv", "dir/b.txt", "dir/moved.txt"]);
    let rn = git::diff(&r, "dir/moved.txt", Some("dir/b.txt"), Area::Staged, &never).unwrap();
    assert!(
        rn.text.contains("rename from dir/b.txt") && rn.text.contains("rename to dir/moved.txt"),
        "{}",
        rn.text
    );

    std::fs::write(r.join("bin.dat"), [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
    let bin = git::diff(&r, "bin.dat", None, Area::Untracked, &never).unwrap();
    assert!(bin.binary, "{}", bin.text);

    std::fs::remove_file(r.join("a.txt")).unwrap_or_default();
    g(&r, &["reset", "-q", "--", "a.txt"]);
    let del = git::diff(&r, "a.txt", None, Area::Unstaged, &never).unwrap();
    assert!(del.text.contains("deleted file mode"), "{}", del.text);

    // bounded output and shortened lines
    let big: String = (0..40_000)
        .map(|i| format!("line number {i} of a very large change\n"))
        .collect();
    write(&r.join("big.txt"), &big);
    let b = git::diff(&r, "big.txt", None, Area::Untracked, &never).unwrap();
    assert!(
        b.truncated && b.text.len() <= git::MAX_DIFF_BYTES + 4096,
        "{} bytes",
        b.text.len()
    );
    write(&r.join("wide.txt"), &"x".repeat(50_000));
    let w = git::diff(&r, "wide.txt", None, Area::Untracked, &never).unwrap();
    assert!(w.text.contains("[line shortened]") && w.text.len() < 10_000);

    for bad in [
        "",
        "../etc/passwd",
        "/etc/passwd",
        "-p",
        "--output=/tmp/x",
        "a\0b",
        "dir/../../x",
    ] {
        assert!(
            git::diff(&r, bad, None, Area::Unstaged, &never).is_err(),
            "{bad:?}"
        );
    }
    assert!(git::diff(&r, "a.txt", Some("../x"), Area::Staged, &never).is_err());
    assert!(git::diff(&tmp("nogit"), "a.txt", None, Area::Unstaged, &never).is_err());
}

// ---------------- hostile repositories ----------------

#[cfg(unix)]
fn marker_script(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let marker = dir.join(format!("MARK-{name}"));
    let script = dir.join(format!("{name}.sh"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch '{}'\ncat \"$1\" 2>/dev/null\nexit 0\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    (script, marker)
}

#[cfg(unix)]
#[test]
fn repository_config_cannot_make_status_or_diff_run_commands() {
    let r = repo("evil");
    let tools = tmp("tools");
    let (fsmon, m_fsmon) = marker_script(&tools, "fsmon");
    let (ext, m_ext) = marker_script(&tools, "ext");
    let (pager, m_pager) = marker_script(&tools, "pager");
    let (textconv, m_textconv) = marker_script(&tools, "textconv");
    g(&r, &["config", "core.fsmonitor", fsmon.to_str().unwrap()]);
    g(&r, &["config", "diff.external", ext.to_str().unwrap()]);
    g(&r, &["config", "core.pager", pager.to_str().unwrap()]);
    g(
        &r,
        &["config", "diff.evil.textconv", textconv.to_str().unwrap()],
    );
    write(&r.join(".gitattributes"), "*.txt diff=evil\n");
    write(&r.join("a.txt"), "one changed\n");

    // Sanity: the same configuration DOES run commands when git is invoked naively.
    let _ = Command::new("git")
        .arg("-C")
        .arg(&r)
        .arg("status")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&r)
        .args(["--no-pager", "diff"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        m_fsmon.exists() && m_ext.exists(),
        "test setup: a naive git run should trigger fsmonitor and the external diff"
    );
    for m in [&m_fsmon, &m_ext, &m_pager, &m_textconv] {
        let _ = std::fs::remove_file(m);
    }

    let s = st(&r);
    assert_eq!(entry(&s, "a.txt").worktree, Some('M'), "status still works");
    for area in [Area::Unstaged, Area::Staged] {
        let _ = git::diff(&r, "a.txt", None, area, &never).unwrap();
    }
    // (staging is test setup, so it must itself avoid running the hostile commands)
    g(&r, &["-c", "core.fsmonitor=false", "add", "a.txt"]);
    let _ = git::diff(&r, "a.txt", None, Area::Staged, &never).unwrap();
    let _ = git::worktrees(&r, &never).unwrap();
    for (name, m) in [
        ("fsmonitor", &m_fsmon),
        ("external diff", &m_ext),
        ("pager", &m_pager),
        ("textconv", &m_textconv),
    ] {
        assert!(!m.exists(), "{name} command was executed by Naurix");
    }
}

#[cfg(unix)]
#[test]
fn a_repository_that_defines_filters_or_includes_is_refused_until_trusted() {
    let r = repo("filter");
    let tools = tmp("ftools");
    let (clean, marker) = marker_script(&tools, "clean");
    g(
        &r,
        &["config", "filter.evil.clean", clean.to_str().unwrap()],
    );
    write(&r.join(".gitattributes"), "*.txt filter=evil\n");
    write(&r.join("a.txt"), "changed\n");
    match git::status(&r, &never).unwrap() {
        StatusResult::Blocked { root, keys } => {
            assert_eq!(root, r.to_string_lossy());
            assert_eq!(keys, ["filter.evil.clean"]);
        }
        other => panic!("{other:?}"),
    }
    assert!(
        !marker.exists(),
        "git must not have been run in the untrusted repository"
    );
    assert!(git::diff(&r, "a.txt", None, Area::Unstaged, &never).is_err());
    assert!(git::worktrees(&r, &never).unwrap().is_empty());

    // include.path is refused too (it can pull in a file that defines more commands)
    let inc = repo("include");
    g(&inc, &["config", "include.path", "../elsewhere.cfg"]);
    assert!(matches!(
        git::status(&inc, &never).unwrap(),
        StatusResult::Blocked { .. }
    ));

    // after the user trusts it (remembered per repository), status runs
    let db = Db::open_in_memory().unwrap();
    assert!(!git::is_trusted(&db, &inc));
    git::trust(&db, &inc).unwrap();
    assert!(
        git::is_trusted(&db, &inc) && !git::is_trusted(&db, &r),
        "trust is per repository"
    );
    let trusted = |p: &Path| git::is_trusted(&db, p);
    assert!(matches!(
        git::status(&inc, &trusted).unwrap(),
        StatusResult::Repo(_)
    ));
    // a filter defined in the user's own *global* config (like Git LFS) never triggers the gate
    let clean_repo = repo("noglobalgate");
    assert!(matches!(
        git::status(&clean_repo, &never).unwrap(),
        StatusResult::Repo(_)
    ));
}

#[test]
fn status_does_not_take_the_index_lock_and_never_modifies_the_repository() {
    let r = repo("readonly");
    write(&r.join("a.txt"), "dirty\n");
    let before = g(&r, &["status", "--porcelain=v2", "-z"]);
    let index_mtime = std::fs::metadata(r.join(".git/index"))
        .unwrap()
        .modified()
        .unwrap();
    // hold the index lock like a concurrent user command would
    write(&r.join(".git/index.lock"), "");
    let s = st(&r);
    assert_eq!(
        entry(&s, "a.txt").worktree,
        Some('M'),
        "works even while the index is locked"
    );
    std::fs::remove_file(r.join(".git/index.lock")).unwrap();
    assert_eq!(
        std::fs::metadata(r.join(".git/index"))
            .unwrap()
            .modified()
            .unwrap(),
        index_mtime,
        "the index was not rewritten"
    );
    assert_eq!(g(&r, &["status", "--porcelain=v2", "-z"]), before);
}

#[test]
fn linked_worktrees_are_listed() {
    let r = repo("wt");
    let other = tmp("wt-other").join("linked");
    g(
        &r,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            other.to_str().unwrap(),
        ],
    );
    let list = git::worktrees(&r, &never).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].branch.as_deref(), Some("main"));
    assert_eq!(list[1].branch.as_deref(), Some("side"));
    assert!(list[1].path.ends_with("linked") && list[1].head.is_some());
    // status inside the linked worktree reports its own branch
    assert_eq!(st(&other).branch.as_deref(), Some("side"));
}

#[test]
fn trusted_repositories_can_be_listed_and_revoked() {
    let db = Db::open_in_memory().unwrap();
    let (a, b) = (repo("trust-a"), repo("trust-b"));
    git::trust(&db, &a).unwrap();
    git::trust(&db, &b).unwrap();
    let list = git::trusted_repos(&db).unwrap();
    assert_eq!(list.len(), 2);
    let mut paths: Vec<_> = list.iter().map(|t| t.path.clone()).collect();
    paths.sort();
    let mut want = vec![
        a.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    ];
    want.sort();
    assert_eq!(paths, want);
    git::untrust(
        &db,
        &list
            .iter()
            .find(|t| t.path == a.to_string_lossy())
            .unwrap()
            .key,
    )
    .unwrap();
    assert!(!git::is_trusted(&db, &a) && git::is_trusted(&db, &b));
    assert!(
        git::untrust(&db, "app.settings").is_err(),
        "only trust entries can be removed this way"
    );
    assert_eq!(db.get_setting("app.settings").unwrap(), None);
}
