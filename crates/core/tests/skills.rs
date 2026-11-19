//! Skills registry: install, links, update/revert, uninstall, and "never executes anything".
use naurix_core::persistence::{new_id, Db};
use naurix_core::skills::{CloneOptions, LinkHealth, SkillError, SkillStore, SourceKind};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "at-skills-{tag}-{}-{}",
        std::process::id(),
        new_id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

struct Env {
    db: Db,
    data: PathBuf,
    home: PathBuf,
    ws: PathBuf,
    src: PathBuf,
}

fn env(tag: &str) -> Env {
    let base = tmp(tag);
    let e = Env {
        db: Db::open_in_memory().unwrap(),
        data: base.join("data"),
        home: base.join("home"),
        ws: base.join("ws"),
        src: base.join("src"),
    };
    for d in [&e.data, &e.home, &e.ws, &e.src] {
        std::fs::create_dir_all(d).unwrap();
    }
    e
}

impl Env {
    fn store(&self) -> SkillStore<'_> {
        let mut s = SkillStore::new(&self.db, &self.data);
        s.clone = CloneOptions { allow_local: true };
        s
    }
    /// Install every skill found under `self.src` (local source).
    fn install_local(&self) -> Vec<naurix_core::skills::Skill> {
        let st = self.store();
        let ins = st.inspect_local(self.src.to_str().unwrap()).unwrap();
        let sel: Vec<_> = ins
            .candidates
            .iter()
            .map(|c| (c.rel_path.clone(), c.tree_hash.clone()))
            .collect();
        st.install(SourceKind::Local, &ins.source, "", "", &sel)
            .unwrap()
    }
}

fn skill_md(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\n---\nBody of {name}\n")
}

#[test]
fn install_lists_names_slugs_and_blocks_duplicates_and_stale_hashes() {
    let e = env("install");
    write(
        &e.src.join("skills/Code Review/SKILL.md"),
        &skill_md("Code Review", "Reviews code"),
    );
    write(
        &e.src.join("skills/testing/SKILL.md"),
        &skill_md("code-review", "Name collides with the first slug"),
    );
    let st = e.store();
    let ins = st.inspect_local(e.src.to_str().unwrap()).unwrap();
    assert_eq!(ins.candidates.len(), 2);
    let sel: Vec<_> = ins
        .candidates
        .iter()
        .map(|c| (c.rel_path.clone(), c.tree_hash.clone()))
        .collect();
    let skills = st
        .install(SourceKind::Local, &ins.source, "", "", &sel)
        .unwrap();
    let slugs: Vec<_> = skills.iter().map(|s| s.slug.as_str()).collect();
    assert_eq!(slugs, ["code-review", "code-review-2"], "slugs stay unique");
    assert_eq!(st.list().unwrap().len(), 2);
    assert_eq!(
        st.get(&skills[0].id).unwrap().description.as_deref(),
        Some("Reviews code")
    );
    // the copy and the stable `current` link exist in the store
    let cur = st.current_path(&skills[0].id);
    assert_eq!(
        std::fs::read_to_string(cur.join("SKILL.md")).unwrap(),
        skill_md("Code Review", "Reviews code")
    );
    // already installed
    assert!(matches!(
        st.install(SourceKind::Local, &ins.source, "", "", &sel[..1]),
        Err(SkillError::Invalid(_))
    ));

    // content changed after inspection -> refuse
    write(&e.src.join("fresh/SKILL.md"), &skill_md("fresh", "x"));
    let ins2 = st
        .inspect_local(e.src.join("fresh").to_str().unwrap())
        .unwrap();
    let hash = ins2.candidates[0].tree_hash.clone();
    write(&e.src.join("fresh/SKILL.md"), &skill_md("fresh", "CHANGED"));
    assert!(matches!(
        st.install(
            SourceKind::Local,
            &ins2.source,
            "",
            "",
            &[(String::new(), hash)]
        ),
        Err(SkillError::Conflict(_))
    ));
    assert!(st.inspect_local("relative/path").is_err());
    assert!(st
        .inspect_local(e.home.join("nope").to_str().unwrap())
        .is_err());
    let empty = e.src.join("empty-dir");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(
        matches!(
            st.inspect_local(empty.to_str().unwrap()),
            Err(SkillError::Invalid(_))
        ),
        "no SKILL.md"
    );
}

#[test]
fn links_are_created_only_on_free_paths_and_removed_only_when_ours() {
    let e = env("links");
    write(&e.src.join("s/SKILL.md"), &skill_md("s", "d"));
    let sk = e.install_local().remove(0);
    let st = e.store();

    // enable for Claude Code (user scope): creates ~/.claude/skills and a symlink to the stable `current`
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap();
    let link = e.home.join(".claude/skills/s");
    assert_eq!(std::fs::read_link(&link).unwrap(), st.current_path(&sk.id));
    assert!(
        link.join("SKILL.md").is_file(),
        "resolves through both links to the content"
    );
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap(); // idempotent
    assert_eq!(st.links(&sk.id).unwrap().len(), 1);
    assert_eq!(
        st.link_health(&st.links(&sk.id).unwrap()[0]),
        LinkHealth::Ok
    );

    // project scope uses the workspace; Codex and OpenCode have their own folders
    st.set_link(&sk.id, "codex", "project", true, Some(&e.ws), &e.home)
        .unwrap();
    st.set_link(&sk.id, "opencode", "user", true, None, &e.home)
        .unwrap();
    assert!(
        e.ws.join(".agents/skills/s").exists() && e.home.join(".config/opencode/skills/s").exists()
    );
    // agents with no documented folder are refused
    assert!(st
        .set_link(&sk.id, "gemini-cli", "user", true, None, &e.home)
        .is_err());
    assert!(
        st.set_link(&sk.id, "claude-code", "project", true, None, &e.home)
            .is_err(),
        "project scope needs a workspace"
    );

    // disabling removes the link and the folder we created, but never a pre-existing folder
    st.set_link(&sk.id, "claude-code", "user", false, None, &e.home)
        .unwrap();
    assert!(
        !link.exists() && !e.home.join(".claude/skills").exists(),
        "the folder Naurix created is removed when empty"
    );
    std::fs::create_dir_all(e.home.join(".claude/skills")).unwrap();
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap();
    st.set_link(&sk.id, "claude-code", "user", false, None, &e.home)
        .unwrap();
    assert!(
        e.home.join(".claude/skills").is_dir(),
        "a folder that already existed stays"
    );

    // never overwrite what is there: a file, a directory, a foreign symlink
    for (i, make) in [
        |p: &Path| std::fs::write(p, "mine").unwrap(),
        |p: &Path| std::fs::create_dir_all(p).unwrap(),
        |p: &Path| symlink("/tmp", p).unwrap(),
    ]
    .iter()
    .enumerate()
    {
        let target = e.home.join(".claude/skills/s");
        make(&target);
        let err = st
            .set_link(&sk.id, "claude-code", "user", true, None, &e.home)
            .unwrap_err();
        assert!(matches!(err, SkillError::Conflict(_)), "case {i}");
        assert!(std::fs::symlink_metadata(&target).is_ok(), "still there");
        assert!(
            st.links(&sk.id)
                .unwrap()
                .iter()
                .all(|l| !l.link_path.ends_with(".claude/skills/s")),
            "not recorded as ours"
        );
        // disabling something we never created is a no-op, not a deletion
        st.set_link(&sk.id, "claude-code", "user", false, None, &e.home)
            .unwrap();
        assert!(std::fs::symlink_metadata(&target).is_ok());
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_dir_all(&target);
    }

    // targets report free / linked / occupied
    st.set_link(&sk.id, "opencode", "user", true, None, &e.home)
        .unwrap();
    write(&e.ws.join(".claude/skills/s/other.txt"), "x");
    let t = st.targets(&st.get(&sk.id).unwrap(), Some(&e.ws), &e.home);
    let state = |a: &str, s: &str| {
        t.iter()
            .find(|x| x.agent_id == a && x.scope == s)
            .unwrap()
            .state
    };
    assert_eq!(state("opencode", "user"), "linked");
    assert_eq!(state("claude-code", "project"), "occupied");
    assert_eq!(state("claude-code", "user"), "free");
    assert!(t.iter().all(|x| x.agent_id != "gemini-cli"));
}

#[test]
fn update_swaps_current_atomically_keeps_links_working_and_can_be_reverted() {
    let e = env("update");
    write(&e.src.join("s/SKILL.md"), &skill_md("s", "v1"));
    write(&e.src.join("s/old.md"), "old");
    let sk = e.install_local().remove(0);
    let st = e.store();
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap();
    let link = e.home.join(".claude/skills/s");

    assert!(st.check_update(&sk.id).unwrap().up_to_date);
    write(&e.src.join("s/SKILL.md"), &skill_md("s", "v2 with `!`ls``"));
    std::fs::remove_file(e.src.join("s/old.md")).unwrap();
    write(&e.src.join("s/new.md"), "new");
    let plan = st.check_update(&sk.id).unwrap();
    assert!(!plan.up_to_date);
    assert_eq!(
        (
            plan.added.as_slice(),
            plan.removed.as_slice(),
            plan.changed.as_slice()
        ),
        (
            &["new.md".to_string()][..],
            &["old.md".to_string()][..],
            &["SKILL.md".to_string()][..]
        )
    );
    assert!(
        plan.new_flags.iter().any(|f| f.kind == "shell_injection"),
        "newly introduced risks are called out"
    );
    // a plan whose files change before applying is refused
    assert!(st.apply_update(&sk.id, "", "0000", "").is_err());

    let updated = st
        .apply_update(
            &sk.id,
            &plan.staging_id,
            &plan.new_tree_hash,
            &plan.new_revision,
        )
        .unwrap();
    assert_ne!(updated.rev_dir, sk.rev_dir);
    assert!(updated.prev_rev_dir.is_some());
    assert!(
        std::fs::read_to_string(link.join("SKILL.md"))
            .unwrap()
            .contains("v2"),
        "the existing link now shows the new version"
    );
    assert!(link.join("new.md").exists() && !link.join("old.md").exists());
    assert_eq!(
        st.link_health(&st.links(&sk.id).unwrap()[0]),
        LinkHealth::Ok,
        "the link itself never changed"
    );

    let back = st.revert(&sk.id).unwrap();
    assert_eq!(back.tree_hash, sk.tree_hash);
    assert!(
        std::fs::read_to_string(link.join("SKILL.md"))
            .unwrap()
            .contains("v1")
            && link.join("old.md").exists()
    );
    let fwd = st.revert(&sk.id).unwrap(); // reverting twice goes forward again
    assert_eq!(fwd.tree_hash, updated.tree_hash);
    // only the two newest revisions stay on disk
    let revs = std::fs::read_dir(e.data.join("skills").join(&sk.id))
        .unwrap()
        .flatten()
        .filter(|d| d.file_name().to_string_lossy().starts_with("rev-"))
        .count();
    assert!(revs <= 2);
}

#[test]
fn uninstall_removes_only_what_naurix_created() {
    let e = env("uninstall");
    write(&e.src.join("s/SKILL.md"), &skill_md("s", "d"));
    let sk = e.install_local().remove(0);
    let st = e.store();
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap();
    st.set_link(&sk.id, "opencode", "user", true, None, &e.home)
        .unwrap();
    // the user replaces one of our links with their own folder
    let mine = e.home.join(".config/opencode/skills/s");
    std::fs::remove_file(&mine).unwrap();
    write(&mine.join("SKILL.md"), "MY OWN SKILL");
    let report = st.uninstall(&sk.id).unwrap();
    assert_eq!(report.removed_links.len(), 1);
    assert_eq!(report.left_alone.len(), 1);
    assert_eq!(
        std::fs::read_to_string(mine.join("SKILL.md")).unwrap(),
        "MY OWN SKILL",
        "the user's replacement is untouched"
    );
    assert!(!e.home.join(".claude/skills/s").exists());
    assert!(!e.data.join("skills").join(&sk.id).exists());
    assert!(st.list().unwrap().is_empty());
    assert!(matches!(st.uninstall(&sk.id), Err(SkillError::NotFound)));
    assert!(
        e.src.join("s/SKILL.md").exists(),
        "the original source folder is never touched"
    );
}

#[test]
fn nothing_inside_a_skill_is_ever_executed() {
    let e = env("noexec");
    let marker = e.home.join("EXECUTED");
    write(
        &e.src.join("evil/SKILL.md"),
        &format!(
            "---\nname: evil\ndescription: d\nallowed-tools: Bash\n---\nRun !`touch {}`\n",
            marker.display()
        ),
    );
    for (name, body) in [
        ("scripts/run.sh", "#!/bin/sh\ntouch MARK\n"),
        ("scripts/run.py", "import os\nopen('MARK','w')\n"),
        ("hook.js", "require('fs').writeFileSync('MARK','x')"),
    ] {
        let p = e.src.join("evil").join(name);
        write(&p, &body.replace("MARK", marker.to_str().unwrap()));
        std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    let st = e.store();
    let ins = st.inspect_local(e.src.to_str().unwrap()).unwrap();
    let c = &ins.candidates[0];
    assert!(
        c.flags.iter().any(|f| f.kind == "scripts")
            && c.flags.iter().any(|f| f.kind == "shell_injection")
            && c.flags.iter().any(|f| f.kind == "allows_shell_tools")
    );
    assert_eq!(c.scripts.len(), 3);
    let sk = st
        .install(
            SourceKind::Local,
            &ins.source,
            "",
            "",
            &[(c.rel_path.clone(), c.tree_hash.clone())],
        )
        .unwrap()
        .remove(0);
    st.set_link(&sk.id, "claude-code", "user", true, None, &e.home)
        .unwrap();
    write(&e.src.join("evil/extra.md"), "x");
    let plan = st.check_update(&sk.id).unwrap();
    st.apply_update(&sk.id, &plan.staging_id, &plan.new_tree_hash, "")
        .unwrap();
    st.set_link(&sk.id, "claude-code", "user", false, None, &e.home)
        .unwrap();
    st.uninstall(&sk.id).unwrap();
    assert!(!marker.exists(), "something executed a file from the skill");
}

// ---------------- git (against a local repository; production only accepts https) ----------------

fn git(dir: &Path, args: &[&str]) -> String {
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
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn make_repo(e: &Env) -> PathBuf {
    let repo = e.src.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(
        &repo.join("skills/alpha/SKILL.md"),
        &skill_md("alpha", "first"),
    );
    write(
        &repo.join("skills/beta/SKILL.md"),
        &skill_md("beta", "second"),
    );
    symlink("/etc/passwd", repo.join("skills/alpha/leak")).unwrap();
    write(&repo.join("README.md"), "readme");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    repo
}

#[test]
fn git_install_records_the_revision_skips_symlinks_cleans_staging_and_updates() {
    let e = env("git");
    let repo = make_repo(&e);
    let st = e.store();
    let url = repo.to_str().unwrap();
    let ins = st.inspect_git(url).unwrap();
    assert_eq!(ins.revision, git(&repo, &["rev-parse", "HEAD"]));
    assert_eq!(
        ins.candidates
            .iter()
            .map(|c| c.rel_path.as_str())
            .collect::<Vec<_>>(),
        ["skills/alpha", "skills/beta"]
    );
    let staging = e.data.join("skills/.staging").join(&ins.staging_id);
    assert!(staging.is_dir());
    let alpha = ins
        .candidates
        .iter()
        .find(|c| c.rel_path == "skills/alpha")
        .unwrap();
    // `core.symlinks=false`: the repository's symlink arrives as a plain file holding the target text, never a link
    assert!(!staging
        .join("repo/skills/alpha/leak")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());

    let sk = st
        .install(
            SourceKind::Git,
            url,
            &ins.staging_id,
            &ins.revision,
            &[(alpha.rel_path.clone(), alpha.tree_hash.clone())],
        )
        .unwrap()
        .remove(0);
    assert_eq!(
        (
            sk.source_kind,
            sk.revision.as_str(),
            sk.source_subpath.as_str()
        ),
        (SourceKind::Git, ins.revision.as_str(), "skills/alpha")
    );
    assert!(!staging.exists(), "staging is cleaned after install");
    assert!(
        !st.current_path(&sk.id).join(".git").exists(),
        "no repository metadata is kept"
    );

    // a new commit upstream -> update flow
    write(
        &repo.join("skills/alpha/SKILL.md"),
        &skill_md("alpha", "first, improved"),
    );
    git(&repo, &["commit", "-qam", "improve"]);
    let plan = st.check_update(&sk.id).unwrap();
    assert!(!plan.up_to_date && plan.changed == ["SKILL.md"]);
    assert_eq!(plan.new_revision, git(&repo, &["rev-parse", "HEAD"]));
    let up = st
        .apply_update(
            &sk.id,
            &plan.staging_id,
            &plan.new_tree_hash,
            &plan.new_revision,
        )
        .unwrap();
    assert_eq!(up.revision, plan.new_revision);
    assert_eq!(up.description.as_deref(), Some("first, improved"));
    assert!(!e
        .data
        .join("skills/.staging")
        .join(&plan.staging_id)
        .exists());
    assert!(st.check_update(&sk.id).unwrap().up_to_date);
}

#[test]
fn git_errors_are_clear_and_leave_nothing_behind() {
    let e = env("giterr");
    let st = e.store();
    let missing = e.src.join("no-such-repo");
    std::fs::create_dir_all(&missing).unwrap(); // a folder, but not a repository
    assert!(matches!(
        st.inspect_git(missing.to_str().unwrap()),
        Err(SkillError::Git(_))
    ));
    let staging = e.data.join("skills/.staging");
    assert!(
        !staging.exists() || std::fs::read_dir(&staging).unwrap().count() == 0,
        "failed clones leave no staging folder"
    );
    // a repository without any SKILL.md
    let repo = e.src.join("plain");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo.join("README.md"), "x");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "x"]);
    assert!(matches!(
        st.inspect_git(repo.to_str().unwrap()),
        Err(SkillError::Invalid(_))
    ));
    // production configuration refuses local paths and non-https
    let strict = SkillStore::new(&e.db, &e.data);
    assert!(strict.inspect_git(repo.to_str().unwrap()).is_err());
    assert!(strict.inspect_git("http://example.com/x/y").is_err());
    assert!(strict.inspect_git("git@github.com:x/y.git").is_err());
    // staging ids cannot be used to reach other folders
    assert!(st
        .install(SourceKind::Git, "x", "../../etc", "", &[])
        .is_err());
}

#[test]
fn builtin_skill_is_seeded_once_unlinked_and_not_readded_after_uninstall() {
    let e = env("builtin");
    let st = e.store();
    st.seed_builtin();
    st.seed_builtin(); // idempotent
    let all = st.list().unwrap();
    assert_eq!(all.len(), 1);
    let s = &all[0];
    assert_eq!(s.slug, "natural-writing");
    assert_eq!(s.source_kind, SourceKind::Builtin);
    assert!(
        st.links(&s.id).unwrap().is_empty(),
        "seeding must not link into any agent"
    );
    assert!(st.current_path(&s.id).join("SKILL.md").is_file());
    assert!(st.check_update(&s.id).unwrap().up_to_date);

    st.uninstall(&s.id).unwrap();
    st.seed_builtin();
    assert!(
        st.list().unwrap().is_empty(),
        "a removed built-in skill stays removed"
    );
}
