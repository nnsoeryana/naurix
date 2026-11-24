//! Persistence for workflows and runs (migration 6). Metadata only.

use super::*;
use crate::persistence::{iso8601_utc_now, new_id, Db};
use rusqlite::{params, OptionalExtension};

pub struct WorkflowStore<'a> {
    db: &'a Db,
}

impl<'a> WorkflowStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Create (`id` is `None`) or replace a workflow. Replacing is refused while one of its runs is active.
    pub fn save(
        &self,
        workspace_id: &str,
        id: Option<&str>,
        name: &str,
        steps: &[StepInput],
    ) -> Result<Workflow, WorkflowError> {
        let (name, step_names) = validate(name, steps)?;
        self.db.workspace(workspace_id)?;
        if let Some(id) = id {
            let w = self.get(id)?;
            if w.workspace_id != workspace_id {
                return Err(WorkflowError::NotFound);
            }
            if RunStore::new(self.db).has_active_run(id)? {
                return Err(WorkflowError::Active);
            }
        }
        let now = iso8601_utc_now();
        let wid = id.map(str::to_string).unwrap_or_else(new_id);
        {
            let c = self.db.c();
            let tx = c.unchecked_transaction()?;
            if id.is_some() {
                tx.execute("DELETE FROM workflow_steps WHERE workflow_id = ?1", [&wid])?;
                tx.execute(
                    "UPDATE workflows SET name = ?2, updated_at = ?3 WHERE id = ?1",
                    params![wid, name, now],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO workflows (id, workspace_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![wid, workspace_id, name, now],
                )?;
            }
            let ids: Vec<String> = steps.iter().map(|_| new_id()).collect();
            for (i, s) in steps.iter().enumerate() {
                tx.execute(
                    "INSERT INTO workflow_steps (id, workflow_id, position, name, agent_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![ids[i], wid, i as i64, step_names[i], s.agent_id],
                )?;
            }
            for (i, s) in steps.iter().enumerate() {
                for d in &s.depends_on {
                    tx.execute(
                        "INSERT INTO workflow_deps (step_id, depends_on) VALUES (?1, ?2)",
                        params![ids[i], ids[*d]],
                    )?;
                }
            }
            tx.commit()?;
        }
        self.get(&wid)
    }

    pub fn get(&self, id: &str) -> Result<Workflow, WorkflowError> {
        let c = self.db.c();
        let head = c
            .query_row(
                "SELECT id, workspace_id, name, created_at, updated_at FROM workflows WHERE id = ?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)),
            )
            .optional()?
            .ok_or(WorkflowError::NotFound)?;
        let mut stmt = c.prepare(
            "SELECT id, name, agent_id FROM workflow_steps WHERE workflow_id = ?1 ORDER BY position",
        )?;
        let mut steps: Vec<Step> = stmt
            .query_map([id], |r| {
                Ok(Step {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    agent_id: r.get(2)?,
                    depends_on: Vec::new(),
                })
            })?
            .collect::<Result<_, _>>()?;
        for s in &mut steps {
            let mut q = c.prepare(
                "SELECT d.depends_on FROM workflow_deps d JOIN workflow_steps p ON p.id = d.depends_on WHERE d.step_id = ?1 ORDER BY p.position",
            )?;
            s.depends_on = q
                .query_map([&s.id], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
        }
        Ok(Workflow {
            id: head.0,
            workspace_id: head.1,
            name: head.2,
            steps,
            created_at: head.3,
            updated_at: head.4,
        })
    }

    pub fn list(&self, workspace_id: &str) -> Result<Vec<Workflow>, WorkflowError> {
        let ids: Vec<String> = {
            let c = self.db.c();
            let mut q = c.prepare("SELECT id FROM workflows WHERE workspace_id = ?1 ORDER BY name COLLATE NOCASE, created_at")?;
            let rows = q.query_map([workspace_id], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.iter().map(|i| self.get(i)).collect()
    }

    /// Delete a workflow. Its runs stay (the link is cleared). Refused while a run is active.
    pub fn delete(&self, id: &str) -> Result<(), WorkflowError> {
        self.get(id)?;
        if RunStore::new(self.db).has_active_run(id)? {
            return Err(WorkflowError::Active);
        }
        self.db
            .c()
            .execute("DELETE FROM workflows WHERE id = ?1", [id])?;
        Ok(())
    }
}

pub struct RunStore<'a> {
    db: &'a Db,
}

impl<'a> RunStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn has_active_run(&self, workflow_id: &str) -> Result<bool, WorkflowError> {
        let n: i64 = self.db.c().query_row(
            "SELECT COUNT(*) FROM runs WHERE workflow_id = ?1 AND state = 'running'",
            [workflow_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Copy `wf` into a new running run. Every step starts `pending`.
    pub fn create(&self, wf: &Workflow) -> Result<Run, WorkflowError> {
        let rid = new_id();
        {
            let c = self.db.c();
            let tx = c.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO runs (id, workflow_id, workspace_id, name, state, created_at) VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
                params![rid, wf.id, wf.workspace_id, wf.name, iso8601_utc_now()],
            )?;
            let mut map = std::collections::HashMap::new();
            for (i, s) in wf.steps.iter().enumerate() {
                let sid = new_id();
                tx.execute(
                    "INSERT INTO run_steps (id, run_id, position, name, agent_id, state) VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
                    params![sid, rid, i as i64, s.name, s.agent_id],
                )?;
                map.insert(s.id.clone(), sid);
            }
            for s in &wf.steps {
                for d in &s.depends_on {
                    tx.execute(
                        "INSERT INTO run_deps (run_step_id, depends_on) VALUES (?1, ?2)",
                        params![map[&s.id], map[d]],
                    )?;
                }
            }
            tx.commit()?;
        }
        self.get(&rid)
    }

    pub fn get(&self, id: &str) -> Result<Run, WorkflowError> {
        let c = self.db.c();
        let (workflow_id, workspace_id, name, state, created_at, finished_at) = c
            .query_row(
                "SELECT workflow_id, workspace_id, name, state, created_at, finished_at FROM runs WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?
            .ok_or(WorkflowError::NotFound)?;
        let mut q = c.prepare(
            "SELECT id, position, name, agent_id, state, started_at, finished_at, exit_code, note FROM run_steps WHERE run_id = ?1 ORDER BY position",
        )?;
        let mut steps: Vec<RunStep> = q
            .query_map([id], |r| {
                Ok(RunStep {
                    id: r.get(0)?,
                    position: r.get(1)?,
                    name: r.get(2)?,
                    agent_id: r.get(3)?,
                    state: StepState::parse(&r.get::<_, String>(4)?),
                    depends_on: Vec::new(),
                    started_at: r.get(5)?,
                    finished_at: r.get(6)?,
                    exit_code: r.get(7)?,
                    note: r.get(8)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        for s in &mut steps {
            let mut d = c.prepare("SELECT depends_on FROM run_deps WHERE run_step_id = ?1")?;
            s.depends_on = d
                .query_map([&s.id], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
        }
        Ok(Run {
            id: id.to_string(),
            workflow_id,
            workspace_id,
            name,
            state: RunState::parse(&state),
            created_at,
            finished_at,
            steps,
        })
    }

    /// Newest first.
    pub fn list(&self, workspace_id: &str, limit: u32) -> Result<Vec<Run>, WorkflowError> {
        let ids: Vec<String> = {
            let c = self.db.c();
            let mut q = c.prepare("SELECT id FROM runs WHERE workspace_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT ?2")?;
            let rows = q.query_map(params![workspace_id, limit], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.iter().map(|i| self.get(i)).collect()
    }

    pub fn active(&self) -> Result<Vec<Run>, WorkflowError> {
        let ids: Vec<String> = {
            let c = self.db.c();
            let mut q =
                c.prepare("SELECT id FROM runs WHERE state = 'running' ORDER BY created_at")?;
            let rows = q.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.iter().map(|i| self.get(i)).collect()
    }

    pub fn start_step(&self, step_id: &str) -> Result<(), WorkflowError> {
        self.db.c().execute(
            "UPDATE run_steps SET state = 'running', started_at = ?2 WHERE id = ?1 AND state = 'pending'",
            params![step_id, iso8601_utc_now()],
        )?;
        Ok(())
    }

    /// Move a step to a final state. Only a step that is not already final changes, so a late exit
    /// after "Mark done" cannot rewrite the outcome.
    pub fn finish_step(
        &self,
        step_id: &str,
        state: StepState,
        exit_code: Option<i64>,
        note: Option<&str>,
    ) -> Result<bool, WorkflowError> {
        let n = self.db.c().execute(
            "UPDATE run_steps SET state = ?2, finished_at = ?3, exit_code = ?4, note = ?5
             WHERE id = ?1 AND state IN ('pending', 'running')",
            params![step_id, state.as_str(), iso8601_utc_now(), exit_code, note],
        )?;
        Ok(n == 1)
    }

    pub fn set_run_state(&self, run_id: &str, state: RunState) -> Result<(), WorkflowError> {
        self.db.c().execute(
            "UPDATE runs SET state = ?2, finished_at = COALESCE(finished_at, ?3) WHERE id = ?1 AND state = 'running'",
            params![run_id, state.as_str(), iso8601_utc_now()],
        )?;
        Ok(())
    }

    /// After a restart no run is still going: running runs become `interrupted` and their running
    /// steps `failed` with the note `interrupted`. Returns how many runs changed.
    pub fn mark_interrupted(&self) -> Result<usize, WorkflowError> {
        let now = iso8601_utc_now();
        let c = self.db.c();
        c.execute(
            "UPDATE run_steps SET state = 'failed', finished_at = ?1, note = ?2
             WHERE state = 'running' AND run_id IN (SELECT id FROM runs WHERE state = 'running')",
            params![now, note::INTERRUPTED],
        )?;
        Ok(c.execute(
            "UPDATE runs SET state = 'interrupted', finished_at = ?1 WHERE state = 'running'",
            [now],
        )?)
    }
}
