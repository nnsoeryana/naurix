use super::def::{launch_action, plan_launch, validate_custom, AgentDef, CustomAgent, LaunchPlan};
use super::AgentError;
use crate::approval::{ApprovalGate, Scope};
use crate::persistence::Db;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const K_CUSTOM: &str = "agents.custom";
/// Pre-gate preset approvals (agent id -> program path). Read once by [`migrate_legacy_approvals`].
const K_LEGACY_APPROVED: &str = "agents.approved";
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
            // Editing the command changes its approval fingerprint, so an earlier grant no longer matches.
            *slot = agent.clone();
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
                let ok = crate::platform::is_absolute_str(p)
                    && crate::platform::is_executable_file(Path::new(p));
                if !ok {
                    return Err(AgentError::Invalid(
                        "Choose an executable file (absolute path).".into(),
                    ));
                }
                self.write(&k_path(id), &p)?;
            }
        }
        // A different executable has a different fingerprint, so it is reviewed again.
        Ok(())
    }

    /// The version reported by an agent, only for a preset whose exact executable the user has
    /// approved for every workspace (running `--version` executes the program, so unreviewed
    /// binaries are never run). Custom agents cannot hold an app-wide grant, so they never qualify.
    pub fn version_if_approved(
        &self,
        id: &str,
        dirs: &[PathBuf],
        gate: &ApprovalGate,
    ) -> Result<Option<String>, AgentError> {
        let def = self.get(id)?;
        let plan = plan_launch(&def, self.path_override(id)?.as_deref(), dirs)?;
        let action = launch_action(&def, &plan.program, "");
        if !gate
            .has_app_grant(&action)
            .map_err(|e| AgentError::Storage(e.to_string()))?
        {
            return Err(AgentError::ReviewRequired);
        }
        Ok(super::version::detect_version(
            &plan.program,
            std::time::Duration::from_secs(5),
        ))
    }

    /// Resolve an agent to a concrete launch. Approval is the gate's job (see `launch_action`).
    pub fn prepare_launch(
        &self,
        id: &str,
        dirs: &[PathBuf],
    ) -> Result<(AgentDef, LaunchPlan), AgentError> {
        self.prepare_launch_with(id, dirs, &[])
    }

    /// Like [`prepare_launch`](Self::prepare_launch) with `extra_args` appended to the agent's
    /// arguments before anything is derived from them, so the reviewed command and its approval
    /// fingerprint cover exactly what will run (used for status hooks, ADR 0029).
    pub fn prepare_launch_with(
        &self,
        id: &str,
        dirs: &[PathBuf],
        extra_args: &[String],
    ) -> Result<(AgentDef, LaunchPlan), AgentError> {
        let mut def = self.get(id)?;
        def.args.extend(extra_args.iter().cloned());
        let plan = plan_launch(&def, self.path_override(id)?.as_deref(), dirs)?;
        Ok((def, plan))
    }
}

/// Turn the pre-gate preset approvals into gate grants, once, so users are not asked again for
/// something they already approved. Each was an approval of an exact executable path for every
/// workspace, which is the "app" scope. The old setting is removed afterwards. Entries for agents
/// that no longer exist are dropped. Returns how many grants were created.
pub fn migrate_legacy_approvals(db: &Db, gate: &ApprovalGate) -> Result<usize, AgentError> {
    let store = AgentStore::new(db);
    let legacy: BTreeMap<String, String> = store.read(K_LEGACY_APPROVED)?;
    let mut created = 0;
    for (id, program) in &legacy {
        let Some(def) = AgentDef::preset(id) else {
            continue;
        };
        let action = launch_action(&def, Path::new(program), "");
        if gate
            .import_grant(&action, Scope::App)
            .map_err(|e| AgentError::Storage(e.to_string()))?
        {
            created += 1;
        }
    }
    db.delete_setting(K_LEGACY_APPROVED)?;
    Ok(created)
}
