use super::git::{clone_shallow, CloneOptions};
use super::scan::{
    copy_skill, file_hashes, find_skill_dirs, inspect_candidate, slugify, Candidate, Flag,
};
use super::SkillError;
use crate::agents::{adapter, Feature, Scope};
use crate::persistence::{iso8601_utc_now, new_id, Db};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Git,
    Local,
    /// Ships inside the app (see `builtin.rs`).
    Builtin,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Local => "local",
            Self::Builtin => "builtin",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub source_kind: SourceKind,
    pub source_url: String,
    pub source_subpath: String,
    pub revision: String,
    pub tree_hash: String,
    pub rev_dir: String,
    pub prev_rev_dir: Option<String>,
    pub flags: Vec<Flag>,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillLink {
    pub skill_id: String,
    pub agent_id: String,
    pub scope: String,
    pub link_path: String,
    pub created_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkHealth {
    Ok,
    /// The link we created is gone.
    Missing,
    /// Something else is at the path now (or the link points elsewhere): left alone.
    Replaced,
}

/// Where an agent looks for skills, and whether enabling would collide with something.
#[derive(Debug, Clone, Serialize)]
pub struct LinkTarget {
    pub agent_id: String,
    pub scope: String,
    pub dir: String,
    /// `free`, `linked` (ours), or `occupied` (something else is there).
    pub state: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    /// Identifies the downloaded copy (git sources); empty for local folders.
    pub staging_id: String,
    pub source_kind: SourceKind,
    pub source: String,
    pub revision: String,
    /// Where installed skills are stored (shown before installing).
    pub install_root: String,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Serialize)]
pub struct UpdatePlan {
    pub skill_id: String,
    pub up_to_date: bool,
    pub staging_id: String,
    pub new_revision: String,
    pub new_tree_hash: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    /// Concerns the *new* version has that the installed one did not.
    pub new_flags: Vec<Flag>,
}

#[derive(Debug, Serialize, Default)]
pub struct UninstallReport {
    pub removed_links: Vec<String>,
    /// Links left alone because the path no longer points at Naurix's copy.
    pub left_alone: Vec<String>,
}

const COLS: &str = "id, name, slug, description, source_kind, source_url, source_subpath, revision, tree_hash, rev_dir, prev_rev_dir, flags_json, installed_at, updated_at";
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
    let kind: String = r.get(4)?;
    let flags: String = r.get(11)?;
    // Flags are stored as (kind, detail) pairs.
    let pairs: Vec<(String, String)> = serde_json::from_str(&flags).unwrap_or_default();
    Ok(Skill {
        id: r.get(0)?,
        name: r.get(1)?,
        slug: r.get(2)?,
        description: r.get(3)?,
        source_kind: match kind.as_str() {
            "git" => SourceKind::Git,
            "builtin" => SourceKind::Builtin,
            _ => SourceKind::Local,
        },
        source_url: r.get(5)?,
        source_subpath: r.get(6)?,
        revision: r.get(7)?,
        tree_hash: r.get(8)?,
        rev_dir: r.get(9)?,
        prev_rev_dir: r.get(10)?,
        flags: pairs
            .into_iter()
            .map(|(k, d)| Flag {
                kind: leak_kind(&k),
                detail: d,
            })
            .collect(),
        installed_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

fn leak_kind(k: &str) -> &'static str {
    [
        "shell_injection",
        "allows_shell_tools",
        "defines_hooks",
        "scripts",
        "binary_files",
        "symlinks_skipped",
    ]
    .into_iter()
    .find(|x| *x == k)
    .unwrap_or("other")
}

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The skills registry and its on-disk store under `<data dir>/skills`.
pub struct SkillStore<'a> {
    pub db: &'a Db,
    pub root: PathBuf,
    pub clone: CloneOptions,
}

impl<'a> SkillStore<'a> {
    pub fn new(db: &'a Db, data_dir: &Path) -> Self {
        Self {
            db,
            root: data_dir.join("skills"),
            clone: CloneOptions::default(),
        }
    }

    fn staging_dir(&self, id: &str) -> PathBuf {
        self.root.join(".staging").join(id)
    }

    fn skill_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// The stable path agents are linked to; it is re-pointed on update and revert.
    pub fn current_path(&self, id: &str) -> PathBuf {
        self.skill_dir(id).join("current")
    }

    /// Remove half-finished downloads (call at startup).
    pub fn cleanup_staging(&self) {
        let _ = std::fs::remove_dir_all(self.root.join(".staging"));
    }

    pub fn discard_staging(&self, staging_id: &str) {
        if safe_id(staging_id) {
            let _ = std::fs::remove_dir_all(self.staging_dir(staging_id));
        }
    }

    // ---------------------------------------------------------------- inspect

    /// Download a repository into staging and list the skills in it. Nothing is installed or run.
    pub fn inspect_git(&self, url: &str) -> Result<Inspection, SkillError> {
        let id = new_id();
        let dest = self.staging_dir(&id).join("repo");
        std::fs::create_dir_all(self.staging_dir(&id))
            .map_err(|e| SkillError::Io(e.to_string()))?;
        let revision = match clone_shallow(url, &dest, self.clone, GIT_TIMEOUT) {
            Ok(r) => r,
            Err(e) => {
                self.discard_staging(&id);
                return Err(e);
            }
        };
        match self.candidates(&dest) {
            Ok(candidates) => Ok(Inspection {
                install_root: self.root.to_string_lossy().into_owned(),
                staging_id: id,
                source_kind: SourceKind::Git,
                source: url.to_string(),
                revision,
                candidates,
            }),
            Err(e) => {
                self.discard_staging(&id);
                Err(e)
            }
        }
    }

    pub fn inspect_local(&self, path: &str) -> Result<Inspection, SkillError> {
        let root = canonical_dir(path)?;
        let candidates = self.candidates(&root)?;
        Ok(Inspection {
            install_root: self.root.to_string_lossy().into_owned(),
            staging_id: String::new(),
            source_kind: SourceKind::Local,
            source: root.to_string_lossy().into_owned(),
            revision: String::new(),
            candidates,
        })
    }

    fn candidates(&self, root: &Path) -> Result<Vec<Candidate>, SkillError> {
        let dirs = find_skill_dirs(root)?;
        if dirs.is_empty() {
            return Err(SkillError::Invalid(
                "No SKILL.md found. A skill is a folder containing a SKILL.md file.".into(),
            ));
        }
        dirs.iter().map(|d| inspect_candidate(root, d)).collect()
    }

    fn source_root(
        &self,
        kind: SourceKind,
        source: &str,
        staging_id: &str,
    ) -> Result<PathBuf, SkillError> {
        match kind {
            SourceKind::Local => canonical_dir(source),
            SourceKind::Git | SourceKind::Builtin => {
                if !safe_id(staging_id) {
                    return Err(SkillError::Invalid("Invalid download reference.".into()));
                }
                let p = self.staging_dir(staging_id).join("repo");
                if p.is_dir() {
                    Ok(p)
                } else {
                    Err(SkillError::Invalid(
                        "The downloaded copy is gone. Inspect the repository again.".into(),
                    ))
                }
            }
        }
    }

    /// Copy an embedded skill into a fresh staging folder; returns `(staging_id, source root)`.
    fn stage_builtin(&self, source: &str) -> Result<(String, PathBuf), SkillError> {
        let b = source
            .strip_prefix("builtin:")
            .and_then(super::builtin::find)
            .ok_or_else(|| SkillError::Invalid("Unknown built-in skill.".into()))?;
        let id = new_id();
        let root = self.staging_dir(&id).join("repo");
        if let Err(e) = super::builtin::materialize(b, &root) {
            self.discard_staging(&id);
            return Err(e);
        }
        Ok((id, root))
    }

    /// Add the built-in skills to the store once. Never links them into an agent, and never adds one
    /// back after the user removed it. Best effort: a failure for one skill does not stop the app.
    pub fn seed_builtin(&self) {
        for b in super::builtin::BUILTIN {
            let key = format!("skills.builtin_seeded.{}", b.slug);
            if self.db.get_setting(&key).ok().flatten().is_some() {
                continue;
            }
            let source = super::builtin::source_for(b.slug);
            let _ = self.seed_one(&source, &key);
        }
    }

    fn seed_one(&self, source: &str, key: &str) -> Result<(), SkillError> {
        let (staging_id, root) = self.stage_builtin(source)?;
        let result = (|| {
            let cands = self.candidates(&root)?;
            let sel: Vec<(String, String)> = cands
                .iter()
                .map(|c| (c.rel_path.clone(), c.tree_hash.clone()))
                .collect();
            self.install(SourceKind::Builtin, source, &staging_id, "", &sel)
        })();
        self.discard_staging(&staging_id);
        result?;
        self.db.set_setting(key, "true")?;
        Ok(())
    }

    // ---------------------------------------------------------------- registry

    pub fn list(&self) -> Result<Vec<Skill>, SkillError> {
        let c = self.db.c();
        let mut st = c.prepare(&format!(
            "SELECT {COLS} FROM skills ORDER BY name COLLATE NOCASE"
        ))?;
        let rows = st.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<Skill, SkillError> {
        self.db
            .c()
            .query_row(&format!("SELECT {COLS} FROM skills WHERE id=?1"), [id], row)
            .optional()?
            .ok_or(SkillError::NotFound)
    }

    fn unique_slug(&self, base: &str) -> Result<String, SkillError> {
        let taken: std::collections::HashSet<String> =
            self.list()?.into_iter().map(|s| s.slug).collect();
        if !taken.contains(base) {
            return Ok(base.to_string());
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|s| !taken.contains(s))
            .ok_or_else(|| SkillError::Invalid("no free name".into()))
    }

    /// Install the chosen skills. Each selection carries the `tree_hash` the user saw; if the files
    /// changed since, nothing is installed for that skill.
    pub fn install(
        &self,
        kind: SourceKind,
        source: &str,
        staging_id: &str,
        revision: &str,
        selections: &[(String, String)],
    ) -> Result<Vec<Skill>, SkillError> {
        let root = self.source_root(kind, source, staging_id)?;
        let mut done = vec![];
        for (rel, expected) in selections {
            let cand = inspect_candidate(&root, Path::new(rel))?;
            if &cand.tree_hash != expected {
                return Err(SkillError::Conflict(format!(
                    "“{}” changed after you inspected it. Inspect it again.",
                    cand.dir_name
                )));
            }
            if self
                .list()?
                .iter()
                .any(|s| s.source_url == source && s.source_subpath == *rel)
            {
                return Err(SkillError::Invalid(format!(
                    "“{}” is already installed. Use Update instead.",
                    cand.name.as_deref().unwrap_or(&cand.dir_name)
                )));
            }
            let display = cand.name.clone().unwrap_or_else(|| {
                if rel.is_empty() {
                    root.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    cand.dir_name.clone()
                }
            });
            let slug = self.unique_slug(&slugify(&display))?;
            let id = new_id();
            let rev_dir = format!("rev-{}", &cand.tree_hash[..12]);
            let dest = self.skill_dir(&id).join(&rev_dir);
            std::fs::create_dir_all(self.skill_dir(&id))
                .map_err(|e| SkillError::Io(e.to_string()))?;
            copy_skill(&root.join(rel), &dest)?;
            point_current(&self.skill_dir(&id), &rev_dir)?;
            let now = iso8601_utc_now();
            let flags = serde_json::to_string(
                &cand
                    .flags
                    .iter()
                    .map(|f| (f.kind, f.detail.clone()))
                    .collect::<Vec<_>>(),
            )
            .expect("serializable");
            self.db.c().execute(
                "INSERT INTO skills (id, name, slug, description, source_kind, source_url, source_subpath, revision, tree_hash, rev_dir, flags_json, installed_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
                params![id, display, slug, cand.description, kind.as_str(), source, rel, revision, cand.tree_hash, rev_dir, flags, now],
            )?;
            done.push(self.get(&id)?);
        }
        if kind != SourceKind::Local {
            self.discard_staging(staging_id);
        }
        Ok(done)
    }

    /// Per-agent compatibility of the installed copy, recomputed from its SKILL.md.
    pub fn compat(&self, id: &str) -> Result<Vec<super::scan::Compat>, SkillError> {
        self.get(id)?;
        let text = std::fs::read(self.current_path(id).join("SKILL.md"))
            .map_err(|e| SkillError::Io(e.to_string()))?;
        Ok(super::scan::compat(&super::frontmatter::parse(
            &String::from_utf8_lossy(&text),
        )))
    }

    // ---------------------------------------------------------------- links

    /// The folder an agent reads skills from, for a scope. Only for agents whose skills folder is documented and accepted.
    pub fn link_dir(
        &self,
        agent_id: &str,
        scope: &str,
        workspace_root: Option<&Path>,
        home: &Path,
    ) -> Result<PathBuf, SkillError> {
        let a = adapter(agent_id).ok_or_else(|| SkillError::Invalid("Unknown agent.".into()))?;
        if !a.acts_on(Feature::Skills) {
            return Err(SkillError::Invalid(
                "Naurix has no documented skills folder for this agent.".into(),
            ));
        }
        let want = if scope == "user" {
            Scope::User
        } else if scope == "project" {
            Scope::Project
        } else {
            return Err(SkillError::Invalid("Unknown scope.".into()));
        };
        let cp = a
            .config_paths
            .iter()
            .find(|c| c.purpose == "skills" && c.scope == want)
            .ok_or_else(|| SkillError::Invalid("No skills folder for that scope.".into()))?;
        Ok(match cp.path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => workspace_root
                .ok_or_else(|| SkillError::Invalid("Choose a project first.".into()))?
                .join(cp.path),
        })
    }

    pub fn links(&self, skill_id: &str) -> Result<Vec<SkillLink>, SkillError> {
        let c = self.db.c();
        let mut st = c.prepare("SELECT skill_id, agent_id, scope, link_path, created_dir FROM skill_links WHERE skill_id=?1 ORDER BY link_path")?;
        let rows = st
            .query_map([skill_id], |r| {
                Ok(SkillLink {
                    skill_id: r.get(0)?,
                    agent_id: r.get(1)?,
                    scope: r.get(2)?,
                    link_path: r.get(3)?,
                    created_dir: r.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn link_health(&self, link: &SkillLink) -> LinkHealth {
        let p = Path::new(&link.link_path);
        match std::fs::symlink_metadata(p) {
            Err(_) => LinkHealth::Missing,
            Ok(m)
                if m.file_type().is_symlink()
                    && std::fs::read_link(p).ok().as_deref()
                        == Some(self.current_path(&link.skill_id).as_path()) =>
            {
                LinkHealth::Ok
            }
            Ok(_) => LinkHealth::Replaced,
        }
    }

    /// For every supported agent and scope: where it would go and whether that spot is free.
    pub fn targets(
        &self,
        skill: &Skill,
        workspace_root: Option<&Path>,
        home: &Path,
    ) -> Vec<LinkTarget> {
        let mut out = vec![];
        for (agent_id, _, _) in crate::agents::PRESETS {
            for scope in ["user", "project"] {
                let Ok(dir) = self.link_dir(agent_id, scope, workspace_root, home) else {
                    continue;
                };
                let link = dir.join(&skill.slug);
                let ours = SkillLink {
                    skill_id: skill.id.clone(),
                    agent_id: (*agent_id).into(),
                    scope: scope.into(),
                    link_path: link.to_string_lossy().into_owned(),
                    created_dir: false,
                };
                let state = match std::fs::symlink_metadata(&link) {
                    Err(_) => "free",
                    Ok(_) if self.link_health(&ours) == LinkHealth::Ok => "linked",
                    Ok(_) => "occupied",
                };
                out.push(LinkTarget {
                    agent_id: (*agent_id).into(),
                    scope: scope.into(),
                    dir: dir.to_string_lossy().into_owned(),
                    state,
                });
            }
        }
        out
    }

    /// Create or remove one link. Never overwrites: an existing path that is not our link blocks it.
    pub fn set_link(
        &self,
        skill_id: &str,
        agent_id: &str,
        scope: &str,
        enabled: bool,
        workspace_root: Option<&Path>,
        home: &Path,
    ) -> Result<(), SkillError> {
        let skill = self.get(skill_id)?;
        let dir = self.link_dir(agent_id, scope, workspace_root, home)?;
        let link = dir.join(&skill.slug);
        let link_s = link.to_string_lossy().into_owned();
        let current = self.current_path(skill_id);
        let existing: Option<SkillLink> = self
            .links(skill_id)?
            .into_iter()
            .find(|l| l.link_path == link_s);
        if enabled {
            match std::fs::symlink_metadata(&link) {
                Ok(_) => {
                    let ours = SkillLink {
                        skill_id: skill_id.into(),
                        agent_id: agent_id.into(),
                        scope: scope.into(),
                        link_path: link_s.clone(),
                        created_dir: false,
                    };
                    if self.link_health(&ours) == LinkHealth::Ok {
                        if existing.is_none() {
                            self.record_link(skill_id, agent_id, scope, &link_s, false)?;
                        }
                        return Ok(());
                    }
                    return Err(SkillError::Conflict(format!("{link_s} already exists and is not managed by Naurix. Nothing was changed.")));
                }
                Err(_) => {
                    let created_dir = !dir.exists();
                    std::fs::create_dir_all(&dir).map_err(|e| SkillError::Io(e.to_string()))?;
                    crate::platform::symlink_dir(&current, &link).map_err(SkillError::Io)?;
                    self.record_link(skill_id, agent_id, scope, &link_s, created_dir)?;
                }
            }
        } else if let Some(l) = existing {
            self.remove_link(&l)?;
        }
        Ok(())
    }

    fn record_link(
        &self,
        skill_id: &str,
        agent_id: &str,
        scope: &str,
        link: &str,
        created_dir: bool,
    ) -> Result<(), SkillError> {
        self.db.c().execute("INSERT OR REPLACE INTO skill_links (skill_id, agent_id, scope, link_path, created_dir, created_at) VALUES (?1,?2,?3,?4,?5,?6)", params![skill_id, agent_id, scope, link, created_dir as i64, iso8601_utc_now()])?;
        Ok(())
    }

    /// Remove a link we created, only if it still points at our copy. Returns whether it was removed.
    fn remove_link(&self, l: &SkillLink) -> Result<bool, SkillError> {
        let removed = match self.link_health(l) {
            LinkHealth::Ok => {
                crate::platform::remove_symlink(std::path::Path::new(&l.link_path))
                    .map_err(|e| SkillError::Io(e.to_string()))?;
                if l.created_dir {
                    if let Some(parent) = Path::new(&l.link_path).parent() {
                        let _ = std::fs::remove_dir(parent); // only succeeds if empty
                    }
                }
                true
            }
            LinkHealth::Missing => true,
            LinkHealth::Replaced => false,
        };
        if removed {
            self.db.c().execute(
                "DELETE FROM skill_links WHERE skill_id=?1 AND link_path=?2",
                params![l.skill_id, l.link_path],
            )?;
        }
        Ok(removed)
    }

    // ---------------------------------------------------------------- update / revert / uninstall

    /// Look for a newer version (downloads again for git sources). Nothing is changed.
    pub fn check_update(&self, id: &str) -> Result<UpdatePlan, SkillError> {
        let s = self.get(id)?;
        let (staging_id, root, revision) = match s.source_kind {
            SourceKind::Local => (String::new(), canonical_dir(&s.source_url)?, String::new()),
            SourceKind::Git => {
                let ins = self.inspect_git_raw(&s.source_url)?;
                (ins.0, ins.1, ins.2)
            }
            SourceKind::Builtin => {
                let (id, root) = self.stage_builtin(&s.source_url)?;
                (id, root, String::new())
            }
        };
        let cand = match inspect_candidate(&root, Path::new(&s.source_subpath)) {
            Ok(c) => c,
            Err(e) => {
                self.discard_staging(&staging_id);
                return Err(e);
            }
        };
        let old = file_hashes(&self.skill_dir(id).join(&s.rev_dir))?;
        let new = file_hashes(&root.join(&s.source_subpath))?;
        let added = new
            .keys()
            .filter(|k| !old.contains_key(*k))
            .cloned()
            .collect();
        let removed = old
            .keys()
            .filter(|k| !new.contains_key(*k))
            .cloned()
            .collect();
        let changed = new
            .iter()
            .filter(|(k, v)| old.get(*k).is_some_and(|o| o != *v))
            .map(|(k, _)| k.clone())
            .collect();
        let had: std::collections::HashSet<_> = s.flags.iter().map(|f| f.kind).collect();
        Ok(UpdatePlan {
            skill_id: id.into(),
            up_to_date: cand.tree_hash == s.tree_hash,
            staging_id,
            new_revision: revision,
            new_tree_hash: cand.tree_hash.clone(),
            added,
            removed,
            changed,
            new_flags: cand
                .flags
                .into_iter()
                .filter(|f| !had.contains(f.kind))
                .collect(),
        })
    }

    fn inspect_git_raw(&self, url: &str) -> Result<(String, PathBuf, String), SkillError> {
        let id = new_id();
        let dest = self.staging_dir(&id).join("repo");
        std::fs::create_dir_all(self.staging_dir(&id))
            .map_err(|e| SkillError::Io(e.to_string()))?;
        match clone_shallow(url, &dest, self.clone, GIT_TIMEOUT) {
            Ok(rev) => Ok((id, dest, rev)),
            Err(e) => {
                self.discard_staging(&id);
                Err(e)
            }
        }
    }

    /// Apply a reviewed update: copy the new version beside the old one and re-point `current`
    /// atomically, so links keep working and the previous version can be restored.
    pub fn apply_update(
        &self,
        id: &str,
        staging_id: &str,
        expected_hash: &str,
        new_revision: &str,
    ) -> Result<Skill, SkillError> {
        let s = self.get(id)?;
        let root = self.source_root(s.source_kind, &s.source_url, staging_id)?;
        let cand = inspect_candidate(&root, Path::new(&s.source_subpath))?;
        if cand.tree_hash != expected_hash {
            return Err(SkillError::Conflict(
                "The files changed after you reviewed the update. Check for updates again.".into(),
            ));
        }
        if cand.tree_hash == s.tree_hash {
            return Ok(s);
        }
        let rev_dir = format!("rev-{}", &cand.tree_hash[..12]);
        let dest = self.skill_dir(id).join(&rev_dir);
        if !dest.exists() {
            copy_skill(&root.join(&s.source_subpath), &dest)?;
        }
        point_current(&self.skill_dir(id), &rev_dir)?;
        let flags = serde_json::to_string(
            &cand
                .flags
                .iter()
                .map(|f| (f.kind, f.detail.clone()))
                .collect::<Vec<_>>(),
        )
        .expect("serializable");
        let now = iso8601_utc_now();
        self.db.c().execute(
            "UPDATE skills SET prev_rev_dir=rev_dir, prev_tree_hash=tree_hash, prev_revision=revision, rev_dir=?2, tree_hash=?3, revision=?4, description=?5, flags_json=?6, updated_at=?7 WHERE id=?1",
            params![id, rev_dir, cand.tree_hash, new_revision, cand.description, flags, now],
        )?;
        if s.source_kind != SourceKind::Local {
            self.discard_staging(staging_id);
        }
        self.prune_revisions(id)?;
        self.get(id)
    }

    /// Switch back to the previous version (and make the current one "previous", so it toggles).
    pub fn revert(&self, id: &str) -> Result<Skill, SkillError> {
        let s = self.get(id)?;
        let prev = s
            .prev_rev_dir
            .clone()
            .ok_or_else(|| SkillError::Invalid("There is no previous version.".into()))?;
        if !self.skill_dir(id).join(&prev).is_dir() {
            return Err(SkillError::Invalid(
                "The previous version is no longer on disk.".into(),
            ));
        }
        point_current(&self.skill_dir(id), &prev)?;
        self.db.c().execute(
            "UPDATE skills SET rev_dir=prev_rev_dir, tree_hash=prev_tree_hash, revision=prev_revision, prev_rev_dir=?2, prev_tree_hash=?3, prev_revision=?4, updated_at=?5 WHERE id=?1",
            params![id, s.rev_dir, s.tree_hash, s.revision, iso8601_utc_now()],
        )?;
        self.get(id)
    }

    fn prune_revisions(&self, id: &str) -> Result<(), SkillError> {
        let s = self.get(id)?;
        let keep: Vec<String> = [Some(s.rev_dir), s.prev_rev_dir]
            .into_iter()
            .flatten()
            .collect();
        if let Ok(rd) = std::fs::read_dir(self.skill_dir(id)) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("rev-")
                    && !keep.contains(&n)
                    && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        Ok(())
    }

    /// Remove the skill: the links Naurix created (if still its own), its stored copy and its record.
    /// A link that was replaced or edited is left alone and reported.
    pub fn uninstall(&self, id: &str) -> Result<UninstallReport, SkillError> {
        self.get(id)?;
        let mut report = UninstallReport::default();
        for l in self.links(id)? {
            if self.remove_link(&l)? {
                report.removed_links.push(l.link_path);
            } else {
                report.left_alone.push(l.link_path);
            }
        }
        let dir = self.skill_dir(id);
        if dir.starts_with(&self.root) && dir != self.root {
            let _ = std::fs::remove_dir_all(&dir);
        }
        // Rows for links left alone are dropped with the skill (ON DELETE CASCADE): we no longer manage them.
        self.db
            .c()
            .execute("DELETE FROM skills WHERE id=?1", [id])?;
        Ok(report)
    }
}

fn canonical_dir(path: &str) -> Result<PathBuf, SkillError> {
    if !path.starts_with('/') || path.contains('\0') {
        return Err(SkillError::Invalid(
            "Choose a folder (absolute path).".into(),
        ));
    }
    let p = std::fs::canonicalize(path)
        .map_err(|_| SkillError::Invalid(format!("Folder not found: {path}")))?;
    if p.is_dir() {
        Ok(p)
    } else {
        Err(SkillError::Invalid("That is not a folder.".into()))
    }
}

/// Atomically point `<skill dir>/current` at `rev` (a sibling folder), replacing only our own symlink.
fn point_current(skill_dir: &Path, rev: &str) -> Result<(), SkillError> {
    let tmp = skill_dir.join(format!("current.new-{}", new_id()));
    crate::platform::symlink_dir(Path::new(rev), &tmp).map_err(SkillError::Io)?;
    let current = skill_dir.join("current");
    // Windows cannot rename over an existing directory symlink; ours is replaced in two steps.
    #[cfg(windows)]
    if std::fs::symlink_metadata(&current).is_ok() {
        let _ = crate::platform::remove_symlink(&current);
    }
    std::fs::rename(&tmp, current).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        SkillError::Io(e.to_string())
    })
}
