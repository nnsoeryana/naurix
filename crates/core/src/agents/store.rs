use super::def::{plan_launch, validate_custom, AgentDef, AgentKind, CustomAgent, LaunchPlan};
use super::AgentError;
use crate::persistence::Db;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const K_CUSTOM: &str = "agents.custom";
const K_APPROVED: &str = "agents.approved";
const MAX_CUSTOM: usize = 20;

fn k_path(id: &str) -> String {
    format!("agents.path.{id}")
}

/// Agent configuration persisted in the `settings` table (no secrets, no terminal content).
pub struct AgentStore<'a> {
    pub db: &'a Db,
}

impl<'a> AgentStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    fn read<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> Result<T, AgentError> {
        match self.db.get_setting(key)? {
            Some(j) => Ok(serde_json::from_str(&j).unwrap_or_default()),
            None => Ok(T::default()),
        }
    }

    fn write<T: serde::Serialize>(&self, key: &str, v: &T) -> Result<(), AgentError> {
        self.db.set_setting(
            key,
            &serde_json::to_string(v).map_err(|e| AgentError::Storage(e.to_string()))?,
        )?;
        Ok(())
    }

    pub fn custom_agents(&self) -> Result<Vec<CustomAgent>, AgentError> {
        self.read(K_CUSTOM)
    }

    /// All agents: presets first, then custom ones.
    pub fn all(&self) -> Result<Vec<AgentDef>, AgentError> {
        let mut v: Vec<AgentDef> = super::def::PRESETS
            .iter()
            .filter_map(|p| AgentDef::preset(p.0))
            .collect();
        v.extend(self.custom_agents()?.iter().map(AgentDef::from));
        Ok(v)
    }

    pub fn get(&self, id: &str) -> Result<AgentDef, AgentError> {
        self.all()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or(AgentError::Unknown)
    }

    /// Create (empty `id`) or update a custom agent. Returns the saved agent.
    pub fn save_custom(&self, mut agent: CustomAgent) -> Result<CustomAgent, AgentError> {
        agent.name = agent.name.trim().to_string();
        validate_custom(&agent)?;
        let mut all = self.custom_agents()?;
        if agent.id.is_empty() {
            if all.len() >= MAX_CUSTOM {
                return Err(AgentError::Invalid(format!(
                    "At most {MAX_CUSTOM} custom agents."
                )));
            }
            agent.id = format!("custom-{}", crate::persistence::new_id());
            all.push(agent.clone());
        } else {
            let slot = all
                .iter_mut()
                .find(|a| a.id == agent.id)
                .ok_or(AgentError::Unknown)?;
            *slot = agent.clone();
            // The command changed, so any earlier approval no longer applies.
            self.revoke_approval(&agent.id)?;
        }
        self.write(K_CUSTOM, &all)?;
        Ok(agent)
    }

    pub fn delete_custom(&self, id: &str) -> Result<(), AgentError> {
        let mut all = self.custom_agents()?;
        let before = all.len();
        all.retain(|a| a.id != id);
        if all.len() == before {
            return Err(AgentError::Unknown);
        }
        self.write(K_CUSTOM, &all)?;
        self.revoke_approval(id)?;
        self.db.delete_setting(&k_path(id))?;
        Ok(())
    }

    pub fn path_override(&self, id: &str) -> Result<Option<String>, AgentError> {
        Ok(self
            .db
            .get_setting(&k_path(id))?
            .and_then(|j| serde_json::from_str::<String>(&j).ok()))
    }

    /// Set (or clear with `None`) the user-chosen executable for an agent.
    pub fn set_path_override(&self, id: &str, path: Option<&str>) -> Result<(), AgentError> {
        self.get(id)?;
        match path {
            None => self.db.delete_setting(&k_path(id))?,
            Some(p) => {
                let ok = p.starts_with('/')
                    && Path::new(p)
                        .metadata()
                        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false);
                if !ok {
                    return Err(AgentError::Invalid(
                        "Choose an executable file (absolute path).".into(),
                    ));
                }
                self.write(&k_path(id), &p)?;
            }
        }
        // A different executable must be reviewed again.
        self.revoke_approval(id)
    }

    fn approvals(&self) -> Result<BTreeMap<String, String>, AgentError> {
        self.read(K_APPROVED)
    }

    /// The program path the user approved for this agent, if any.
    pub fn approved_program(&self, id: &str) -> Result<Option<String>, AgentError> {
        Ok(self.approvals()?.get(id).cloned())
    }

    fn revoke_approval(&self, id: &str) -> Result<(), AgentError> {
        let mut a = self.approvals()?;
        if a.remove(id).is_some() {
            self.write(K_APPROVED, &a)?;
        }
        Ok(())
    }

    pub fn approve(&self, id: &str, program: &Path) -> Result<(), AgentError> {
        let mut a = self.approvals()?;
        a.insert(id.to_string(), program.to_string_lossy().into_owned());
        self.write(K_APPROVED, &a)
    }

    /// The version reported by an agent, only for an executable the user has already approved
    /// (running `--version` executes the program, so unreviewed binaries are never run).
    pub fn version_of_approved(
        &self,
        id: &str,
        dirs: &[PathBuf],
    ) -> Result<Option<String>, AgentError> {
        let def = self.get(id)?;
        let plan = plan_launch(&def, self.path_override(id)?.as_deref(), dirs)?;
        if self.approved_program(id)?.as_deref() != Some(&*plan.program.to_string_lossy()) {
            return Err(AgentError::ReviewRequired);
        }
        Ok(super::version::detect_version(
            &plan.program,
            std::time::Duration::from_secs(5),
        ))
    }

    /// Build the launch plan and enforce the review rule (ADR 0007 §4): custom agents and
    /// not-yet-approved presets must be explicitly reviewed (`reviewed == true`) for this launch.
    pub fn prepare_launch(
        &self,
        id: &str,
        dirs: &[PathBuf],
        reviewed: bool,
    ) -> Result<(AgentDef, LaunchPlan), AgentError> {
        let def = self.get(id)?;
        let over = self.path_override(id)?;
        let plan = plan_launch(&def, over.as_deref(), dirs)?;
        let pre_approved = def.kind == AgentKind::Preset
            && self.approved_program(id)?.as_deref() == Some(&*plan.program.to_string_lossy());
        if !reviewed && !pre_approved {
            return Err(AgentError::ReviewRequired);
        }
        Ok((def, plan))
    }
}
