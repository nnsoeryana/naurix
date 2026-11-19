//! Finding, inspecting, hashing and copying skills. Everything here treats the skill as untrusted
//! data: files are only read or copied, **never executed**, and symlinks are never followed.

use super::frontmatter::{parse, Frontmatter};
use super::SkillError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

pub const MAX_FILES: usize = 2000;
pub const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_SKILL_MD: u64 = 256 * 1024;
const MAX_CANDIDATES: usize = 100;
const MAX_DEPTH: usize = 3;

const SCRIPT_EXT: &[&str] = &[
    "sh", "bash", "zsh", "fish", "py", "js", "mjs", "cjs", "ts", "rb", "pl", "php", "ps1",
    "command", "lua",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Flag {
    /// `shell_injection`, `allows_shell_tools`, `defines_hooks`, `scripts`, `binary_files`, `symlinks_skipped`.
    pub kind: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Compat {
    pub agent_id: &'static str,
    /// `ok`, `needs_fix`, `unknown`, `unsupported`.
    pub status: &'static str,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    /// Path of the skill folder relative to the source root (`""` = the root itself).
    pub rel_path: String,
    pub dir_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub scripts: Vec<String>,
    pub flags: Vec<Flag>,
    pub compat: Vec<Compat>,
    pub tree_hash: String,
}

fn is_hidden_or_junk(name: &str) -> bool {
    name == ".git" || name == "node_modules" || name == ".DS_Store"
}

/// Folders (relative to `root`) that contain a `SKILL.md`, searched to a bounded depth. A skill folder
/// is not searched further.
pub fn find_skill_dirs(root: &Path) -> Result<Vec<PathBuf>, SkillError> {
    fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if out.len() >= MAX_CANDIDATES {
            return;
        }
        if dir.join("SKILL.md").is_file() {
            out.push(dir.strip_prefix(root).unwrap_or(dir).to_path_buf());
            return;
        }
        if depth >= MAX_DEPTH {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut subs: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && !is_hidden_or_junk(&e.file_name().to_string_lossy())
            })
            .map(|e| e.path())
            .collect();
        subs.sort();
        for s in subs {
            walk(root, &s, depth + 1, out);
        }
    }
    if !root.is_dir() {
        return Err(SkillError::Invalid("The source is not a folder.".into()));
    }
    let mut out = vec![];
    walk(root, root, 0, &mut out);
    Ok(out)
}

struct FileEntry {
    rel: PathBuf,
    size: u64,
    mode: u32,
}

/// Regular files under `dir` (sorted), enforcing the size limits and counting skipped symlinks.
fn list_files(dir: &Path) -> Result<(Vec<FileEntry>, usize), SkillError> {
    let mut files = vec![];
    let mut symlinks = 0usize;
    let mut total = 0u64;
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(dir.join(&rel))
            .map_err(|e| SkillError::Io(e.to_string()))?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name();
            let name_s = name.to_string_lossy();
            if is_hidden_or_junk(&name_s) {
                continue;
            }
            let rel_child = rel.join(&name);
            let md =
                std::fs::symlink_metadata(e.path()).map_err(|e| SkillError::Io(e.to_string()))?;
            if md.file_type().is_symlink() {
                symlinks += 1;
            } else if md.is_dir() {
                stack.push(rel_child);
            } else if md.is_file() {
                if md.len() > MAX_FILE_BYTES {
                    return Err(SkillError::Invalid(format!(
                        "{} is larger than {} MB.",
                        rel_child.display(),
                        MAX_FILE_BYTES >> 20
                    )));
                }
                total += md.len();
                if files.len() + 1 > MAX_FILES || total > MAX_TOTAL_BYTES {
                    return Err(SkillError::Invalid(format!(
                        "The skill is too large (limit {MAX_FILES} files / {} MB).",
                        MAX_TOTAL_BYTES >> 20
                    )));
                }
                files.push(FileEntry {
                    rel: rel_child,
                    size: md.len(),
                    mode: crate::platform::file_mode(&md),
                });
            }
        }
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok((files, symlinks))
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Stable hash of a skill folder: relative paths + file bytes (not timestamps).
pub fn tree_hash(dir: &Path) -> Result<String, SkillError> {
    let (files, _) = list_files(dir)?;
    let mut h = Sha256::new();
    for f in &files {
        h.update(f.rel.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(std::fs::read(dir.join(&f.rel)).map_err(|e| SkillError::Io(e.to_string()))?);
        h.update([0]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Relative paths of every file in a skill folder (for update diffs).
pub fn file_hashes(dir: &Path) -> Result<std::collections::BTreeMap<String, String>, SkillError> {
    let (files, _) = list_files(dir)?;
    files
        .into_iter()
        .map(|f| {
            Ok((
                f.rel.to_string_lossy().into_owned(),
                sha(&std::fs::read(dir.join(&f.rel)).map_err(|e| SkillError::Io(e.to_string()))?),
            ))
        })
        .collect()
}

fn looks_like_script(f: &FileEntry, head: &[u8]) -> bool {
    let ext = f
        .rel
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    f.mode & 0o111 != 0
        || head.starts_with(b"#!")
        || SCRIPT_EXT.contains(&ext.as_str())
        || f.rel
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "scripts")
}

pub(crate) fn compat(fm: &Frontmatter) -> Vec<Compat> {
    let (name, desc) = (fm.get("name").is_some(), fm.get("description").is_some());
    let c = |agent_id, status, note: &str| Compat {
        agent_id,
        status,
        note: note.into(),
    };
    vec![
        c(
            "claude-code",
            "ok",
            if desc {
                "Name defaults to the folder; description is used to decide when to apply it."
            } else {
                "No description: Claude falls back to the first line of the file."
            },
        ),
        if name && desc {
            c("codex", "ok", "Has the required `name` and `description`.")
        } else {
            c(
                "codex",
                "needs_fix",
                "Codex requires `name` and `description` in the front matter.",
            )
        },
        if desc {
            c(
                "opencode",
                "ok",
                "Has a description, so it is advertised to the model.",
            )
        } else {
            c(
                "opencode",
                "needs_fix",
                "Without a description OpenCode does not advertise the skill to the model.",
            )
        },
        c(
            "antigravity-cli",
            "unknown",
            "Antigravity's skill requirements were not found in the documentation read.",
        ),
        c(
            "gemini-cli",
            "unsupported",
            "No documented skills folder for Gemini CLI itself.",
        ),
    ]
}

/// Inspect one skill folder without modifying or executing anything.
pub fn inspect_candidate(root: &Path, rel: &Path) -> Result<Candidate, SkillError> {
    let dir = root.join(rel);
    let md_path = dir.join("SKILL.md");
    let md = std::fs::metadata(&md_path)
        .map_err(|_| SkillError::Invalid("SKILL.md not found.".into()))?;
    if md.len() > MAX_SKILL_MD {
        return Err(SkillError::Invalid(
            "SKILL.md is larger than 256 KB.".into(),
        ));
    }
    let text = String::from_utf8_lossy(
        &std::fs::read(&md_path).map_err(|e| SkillError::Io(e.to_string()))?,
    )
    .into_owned();
    let fm = parse(&text);
    let (files, symlinks) = list_files(&dir)?;
    let mut scripts = vec![];
    let mut binaries = 0;
    for f in &files {
        let mut head = [0u8; 512];
        let n = std::fs::File::open(dir.join(&f.rel))
            .and_then(|mut h| std::io::Read::read(&mut h, &mut head))
            .unwrap_or(0);
        let head = &head[..n];
        if looks_like_script(f, head) {
            scripts.push(f.rel.to_string_lossy().into_owned());
        } else if head.contains(&0) {
            binaries += 1;
        }
    }
    let mut flags = vec![];
    if text.contains("!`") || text.contains("```!") {
        flags.push(Flag { kind: "shell_injection", detail: "SKILL.md contains command-injection syntax (!`…` or ```!). The agent would run those shell commands when the skill is used.".into() });
    }
    if fm.get("allowed-tools").is_some_and(|t| t.contains("Bash")) {
        flags.push(Flag {
            kind: "allows_shell_tools",
            detail: "The skill pre-approves shell commands (allowed-tools).".into(),
        });
    }
    if fm.fields.contains_key("hooks") {
        flags.push(Flag {
            kind: "defines_hooks",
            detail: "The skill defines hooks that run commands.".into(),
        });
    }
    if !scripts.is_empty() {
        flags.push(Flag { kind: "scripts", detail: format!("Contains {} script or executable file(s). Naurix never runs them, but an agent might.", scripts.len()) });
    }
    if binaries > 0 {
        flags.push(Flag {
            kind: "binary_files",
            detail: format!("Contains {binaries} binary file(s)."),
        });
    }
    if symlinks > 0 {
        flags.push(Flag {
            kind: "symlinks_skipped",
            detail: format!("{symlinks} symbolic link(s) are not copied."),
        });
    }
    Ok(Candidate {
        rel_path: rel.to_string_lossy().into_owned(),
        dir_name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "skill".into()),
        name: fm.get("name").map(str::to_string),
        description: fm.get("description").map(|d| d.chars().take(400).collect()),
        file_count: files.len(),
        total_bytes: files.iter().map(|f| f.size).sum(),
        scripts,
        flags,
        compat: compat(&fm),
        tree_hash: tree_hash(&dir)?,
    })
}

/// Copy a skill folder (regular files only) to `dest`, which must not exist. Symlinks are skipped, never followed.
pub fn copy_skill(src: &Path, dest: &Path) -> Result<usize, SkillError> {
    if dest.exists() {
        return Err(SkillError::Invalid(
            "The destination already exists.".into(),
        ));
    }
    let (files, _) = list_files(src)?;
    std::fs::create_dir_all(dest).map_err(|e| SkillError::Io(e.to_string()))?;
    for f in &files {
        // `rel` came from read_dir, but make traversal impossible regardless.
        if f.rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            let _ = std::fs::remove_dir_all(dest);
            return Err(SkillError::Invalid("Unsafe path in the skill.".into()));
        }
        let to = dest.join(&f.rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SkillError::Io(e.to_string()))?;
        }
        std::fs::copy(src.join(&f.rel), &to).map_err(|e| {
            let _ = std::fs::remove_dir_all(dest);
            SkillError::Io(e.to_string())
        })?;
    }
    Ok(files.len())
}

/// A name usable as a folder in agent skill directories: lowercase letters, digits, `.`, `_`, `-`.
pub fn slugify(s: &str) -> String {
    let mut out: String = s
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out
        .trim_matches(|c| c == '-' || c == '.')
        .chars()
        .take(64)
        .collect::<String>();
    if out.is_empty() {
        "skill".into()
    } else {
        out
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "at-scan-{tag}-{}-{}",
            std::process::id(),
            crate::persistence::new_id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn finds_single_and_nested_skills_and_stops_at_a_skill_folder() {
        let r = tmp("find");
        write(&r.join("skills/a/SKILL.md"), "---\nname: a\n---\n");
        write(&r.join("skills/b/SKILL.md"), "x");
        write(
            &r.join("skills/b/inner/SKILL.md"),
            "nested inside a skill: not a separate candidate",
        );
        write(&r.join("node_modules/z/SKILL.md"), "ignored");
        write(&r.join(".git/x/SKILL.md"), "ignored");
        let found: Vec<String> = find_skill_dirs(&r)
            .unwrap()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(found, ["skills/a", "skills/b"]);
        let one = tmp("one");
        write(&one.join("SKILL.md"), "x");
        assert_eq!(find_skill_dirs(&one).unwrap(), [PathBuf::new()]);
        assert!(find_skill_dirs(&r.join("missing")).is_err());
    }

    #[test]
    fn inspection_flags_scripts_injection_hooks_and_computes_compatibility() {
        let r = tmp("inspect");
        let s = r.join("danger");
        write(&s.join("SKILL.md"), "---\nname: danger\nallowed-tools: Bash(rm *)\nhooks:\n  x: y\n---\nRun !`whoami` now\n");
        write(&s.join("scripts/go.py"), "print(1)");
        write(&s.join("plain.md"), "docs");
        std::fs::write(s.join("bin.dat"), [0u8, 1, 2, 0]).unwrap();
        symlink("/etc/passwd", s.join("link")).unwrap();
        let c = inspect_candidate(&r, Path::new("danger")).unwrap();
        let kinds: Vec<_> = c.flags.iter().map(|f| f.kind).collect();
        for k in [
            "shell_injection",
            "allows_shell_tools",
            "defines_hooks",
            "scripts",
            "binary_files",
            "symlinks_skipped",
        ] {
            assert!(kinds.contains(&k), "missing flag {k}: {kinds:?}");
        }
        assert_eq!(c.scripts, ["scripts/go.py"]);
        assert_eq!(c.file_count, 4, "symlinks are not counted as files");
        let codex = c.compat.iter().find(|x| x.agent_id == "codex").unwrap();
        assert_eq!(codex.status, "needs_fix", "no description");
        assert_eq!(
            c.compat
                .iter()
                .find(|x| x.agent_id == "claude-code")
                .unwrap()
                .status,
            "ok"
        );
    }

    #[test]
    fn a_clean_skill_has_no_flags_and_is_compatible() {
        let r = tmp("clean");
        write(
            &r.join("s/SKILL.md"),
            "---\nname: s\ndescription: Does a thing\n---\nHello",
        );
        let c = inspect_candidate(&r, Path::new("s")).unwrap();
        assert!(c.flags.is_empty(), "{:?}", c.flags);
        assert!(["claude-code", "codex", "opencode"].iter().all(|a| c
            .compat
            .iter()
            .find(|x| x.agent_id == *a)
            .unwrap()
            .status
            == "ok"));
        assert_eq!(c.name.as_deref(), Some("s"));
    }

    #[test]
    fn copy_skips_symlinks_never_follows_them_and_refuses_existing_destinations() {
        let r = tmp("copy");
        let outside = tmp("secret");
        write(&outside.join("secret.txt"), "TOP-SECRET");
        let s = r.join("s");
        write(&s.join("SKILL.md"), "x");
        write(&s.join("sub/f.txt"), "f");
        symlink(&outside, s.join("evil-dir")).unwrap();
        symlink(outside.join("secret.txt"), s.join("evil-file")).unwrap();
        let dest = r.join("out");
        assert_eq!(copy_skill(&s, &dest).unwrap(), 2);
        assert!(dest.join("SKILL.md").is_file() && dest.join("sub/f.txt").is_file());
        assert!(
            !dest.join("evil-dir").exists() && !dest.join("evil-file").exists(),
            "symlinks must not be copied or followed"
        );
        assert!(copy_skill(&s, &dest).is_err(), "destination exists");
    }

    #[test]
    fn limits_are_enforced() {
        let r = tmp("limits");
        let s = r.join("s");
        write(&s.join("SKILL.md"), "x");
        std::fs::write(s.join("big.bin"), vec![1u8; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        assert!(inspect_candidate(&r, Path::new("s")).is_err());
        let many = r.join("many");
        write(&many.join("SKILL.md"), "x");
        for i in 0..=MAX_FILES {
            std::fs::write(many.join(format!("f{i}")), "x").unwrap();
        }
        assert!(copy_skill(&many, &r.join("o")).is_err());
        assert!(
            !r.join("o").exists()
                || std::fs::read_dir(r.join("o"))
                    .map(|d| d.count())
                    .unwrap_or(0)
                    == 0
        );
    }

    #[test]
    fn tree_hash_depends_on_content_and_paths_not_timestamps() {
        let r = tmp("hash");
        write(&r.join("a/SKILL.md"), "one");
        write(&r.join("b/SKILL.md"), "one");
        assert_eq!(
            tree_hash(&r.join("a")).unwrap(),
            tree_hash(&r.join("b")).unwrap()
        );
        write(&r.join("b/SKILL.md"), "two");
        assert_ne!(
            tree_hash(&r.join("a")).unwrap(),
            tree_hash(&r.join("b")).unwrap()
        );
        write(&r.join("a/extra.md"), "x");
        write(&r.join("b/SKILL.md"), "one");
        assert_ne!(
            tree_hash(&r.join("a")).unwrap(),
            tree_hash(&r.join("b")).unwrap()
        );
    }

    #[test]
    fn slugify_is_safe_for_folder_names() {
        assert_eq!(slugify("My Cool Skill!"), "my-cool-skill");
        assert_eq!(slugify("../../etc"), "etc");
        assert_eq!(slugify("..."), "skill");
        assert_eq!(slugify("a/b\\c"), "a-b-c");
        assert!(slugify(&"x".repeat(200)).len() <= 64);
    }
}
