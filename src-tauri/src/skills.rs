//! Tauri commands for the skills manager. Skills are untrusted: these commands download, inspect,
//! copy and link them, and never run anything from them. Fields are snake_case, as serialized by core.

use crate::mcp::DataDir;
use naurix_core::agents::{adapter, Feature, Level, PRESETS};
use naurix_core::persistence::Db;
use naurix_core::skills::{
    Compat, Inspection, LinkHealth, LinkTarget, Skill, SkillLink, SkillStore, SourceKind,
    UninstallReport, UpdatePlan,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn home() -> PathBuf {
    naurix_core::platform::home_dir_or_empty()
}

fn workspace_root(db: &Db, workspace_id: Option<&str>) -> Result<Option<PathBuf>, String> {
    match workspace_id {
        None => Ok(None),
        Some(id) => {
            let w = db.workspace(id).map_err(err)?;
            let p = PathBuf::from(w.root_path);
            if p.is_dir() {
                Ok(Some(p))
            } else {
                Err(format!("The workspace folder is missing: {}", p.display()))
            }
        }
    }
}

#[derive(Serialize)]
pub struct LinkView {
    #[serde(flatten)]
    link: SkillLink,
    health: LinkHealth,
}

#[derive(Serialize)]
pub struct SkillView {
    skill: Skill,
    links: Vec<LinkView>,
    targets: Vec<LinkTarget>,
    compat: Vec<Compat>,
}

fn views(store: &SkillStore<'_>, root: Option<&std::path::Path>) -> Result<Vec<SkillView>, String> {
    let home = home();
    store
        .list()
        .map_err(err)?
        .into_iter()
        .map(|skill| {
            let links = store
                .links(&skill.id)
                .map_err(err)?
                .into_iter()
                .map(|l| LinkView {
                    health: store.link_health(&l),
                    link: l,
                })
                .collect();
            let targets = store.targets(&skill, root, &home);
            let compat = store.compat(&skill.id).unwrap_or_default();
            Ok(SkillView {
                skill,
                links,
                targets,
                compat,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn skill_list(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    workspace_id: Option<String>,
) -> Result<Vec<SkillView>, String> {
    let root = workspace_root(&db, workspace_id.as_deref())?;
    views(&SkillStore::new(&db, &dir.0), root.as_deref())
}

#[derive(Serialize)]
pub struct SkillAgent {
    agent_id: &'static str,
    name: &'static str,
    supported: bool,
    user_dir: Option<&'static str>,
    project_dir: Option<&'static str>,
    note: String,
}

/// Which agents skills can be linked for, with their documented folders and the honest caveat.
#[tauri::command]
pub async fn skill_agents() -> Vec<SkillAgent> {
    PRESETS
        .iter()
        .filter_map(|(id, name, _)| {
            let a = adapter(id)?;
            let dir = |scope| a.config_paths.iter().find(|c| c.purpose == "skills" && c.scope == scope).map(|c| c.path);
            let (supported, note) = match a.level(Feature::Skills) {
                Level::Verified { .. } => (true, "Verified.".to_string()),
                Level::DocumentedAccepted { .. } => (true, "Naurix adds a link in the folder this agent's documentation names. Whether the agent then loads the skill has not been verified.".to_string()),
                Level::DocumentedOnly { .. } => (false, "Documented but not enabled.".to_string()),
                Level::Unsupported { reason } => (false, reason.to_string()),
            };
            Some(SkillAgent { agent_id: id, name, supported, user_dir: dir(naurix_core::agents::Scope::User), project_dir: dir(naurix_core::agents::Scope::Project), note })
        })
        .collect()
}

#[tauri::command]
pub async fn skill_inspect_git(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    url: String,
) -> Result<Inspection, String> {
    let (db, dir) = (db.inner().clone(), dir.0.clone());
    tauri::async_runtime::spawn_blocking(move || {
        SkillStore::new(&db, &dir).inspect_git(&url).map_err(err)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn skill_inspect_local(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    path: String,
) -> Result<Inspection, String> {
    let (db, dir) = (db.inner().clone(), dir.0.clone());
    tauri::async_runtime::spawn_blocking(move || {
        SkillStore::new(&db, &dir).inspect_local(&path).map_err(err)
    })
    .await
    .map_err(err)?
}

#[derive(Deserialize)]
pub struct Selection {
    rel_path: String,
    tree_hash: String,
}

#[tauri::command]
pub async fn skill_install(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    source_kind: SourceKindArg,
    source: String,
    staging_id: String,
    revision: String,
    selections: Vec<Selection>,
) -> Result<Vec<Skill>, String> {
    let (db, dir) = (db.inner().clone(), dir.0.clone());
    let sel: Vec<(String, String)> = selections
        .into_iter()
        .map(|s| (s.rel_path, s.tree_hash))
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        SkillStore::new(&db, &dir)
            .install(source_kind.into(), &source, &staging_id, &revision, &sel)
            .map_err(err)
    })
    .await
    .map_err(err)?
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SourceKindArg {
    Git,
    Local,
    Builtin,
}

impl From<SourceKindArg> for SourceKind {
    fn from(k: SourceKindArg) -> Self {
        match k {
            SourceKindArg::Git => SourceKind::Git,
            SourceKindArg::Local => SourceKind::Local,
            SourceKindArg::Builtin => SourceKind::Builtin,
        }
    }
}

#[tauri::command]
pub async fn skill_discard_staging(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    staging_id: String,
) -> Result<(), String> {
    SkillStore::new(&db, &dir.0).discard_staging(&staging_id);
    Ok(())
}

#[tauri::command]
pub async fn skill_set_link(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    skill_id: String,
    agent_id: String,
    scope: String,
    enabled: bool,
    workspace_id: Option<String>,
) -> Result<Vec<SkillView>, String> {
    let root = workspace_root(&db, workspace_id.as_deref())?;
    let store = SkillStore::new(&db, &dir.0);
    store
        .set_link(
            &skill_id,
            &agent_id,
            &scope,
            enabled,
            root.as_deref(),
            &home(),
        )
        .map_err(err)?;
    views(&store, root.as_deref())
}

#[tauri::command]
pub async fn skill_check_update(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    skill_id: String,
) -> Result<UpdatePlan, String> {
    let (db, dir) = (db.inner().clone(), dir.0.clone());
    tauri::async_runtime::spawn_blocking(move || {
        SkillStore::new(&db, &dir)
            .check_update(&skill_id)
            .map_err(err)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn skill_apply_update(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    skill_id: String,
    staging_id: String,
    expected_hash: String,
    new_revision: String,
) -> Result<Skill, String> {
    SkillStore::new(&db, &dir.0)
        .apply_update(&skill_id, &staging_id, &expected_hash, &new_revision)
        .map_err(err)
}

#[tauri::command]
pub async fn skill_revert(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    skill_id: String,
) -> Result<Skill, String> {
    SkillStore::new(&db, &dir.0).revert(&skill_id).map_err(err)
}

#[tauri::command]
pub async fn skill_uninstall(
    db: State<'_, Arc<Db>>,
    dir: State<'_, DataDir>,
    skill_id: String,
) -> Result<UninstallReport, String> {
    SkillStore::new(&db, &dir.0)
        .uninstall(&skill_id)
        .map_err(err)
}
