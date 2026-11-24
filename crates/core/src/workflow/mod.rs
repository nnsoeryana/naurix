//! Reusable workflows and runs (ADR 0030, orchestration stage 8). No Tauri dependency.
//!
//! A workflow is a named set of steps ("run agent X") and dependencies. A run is one execution of it,
//! stored as a snapshot. Everything here is metadata: ids, user-typed names, edges, states and times.
//! There are no prompts, no arguments, no paths and no output, and nothing in this module sends
//! input to an agent.

pub mod engine;
pub mod store;

pub use engine::{Engine, LaunchProblem, Launcher, StartVerdict};
pub use store::{RunStore, WorkflowStore};

use serde::Serialize;

pub const MAX_STEPS: usize = 24;
pub const MAX_EDGES: usize = 64;
pub const MAX_NAME_CHARS: usize = 60;

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("{0}")]
    Invalid(String),
    #[error("That workflow or run no longer exists.")]
    NotFound,
    #[error("A run of this workflow is still active. Cancel or finish it first.")]
    Active,
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<rusqlite::Error> for WorkflowError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}

impl From<crate::persistence::DbError> for WorkflowError {
    fn from(e: crate::persistence::DbError) -> Self {
        Self::Storage(e.to_string())
    }
}

fn invalid<T>(m: impl Into<String>) -> Result<T, WorkflowError> {
    Err(WorkflowError::Invalid(m.into()))
}

/// A step as the user edits it. Dependencies are positions in the same list, because a new step has
/// no id yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepInput {
    pub name: String,
    pub agent_id: String,
    pub depends_on: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub steps: Vec<Step>,
    pub created_at: String,
    pub updated_at: String,
}

/// A name the database and the UI accept: no control characters, trimmed, 1–60 characters.
pub fn clean_name(raw: &str, what: &str) -> Result<String, WorkflowError> {
    let s: String = raw.chars().filter(|c| !c.is_control()).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        return invalid(format!("{what} needs a name."));
    }
    if s.chars().count() > MAX_NAME_CHARS {
        return invalid(format!(
            "{what}: the name is {} characters long, at most {MAX_NAME_CHARS} are allowed.",
            s.chars().count()
        ));
    }
    Ok(s)
}

/// Check a workflow before it is saved. Returns the cleaned name and step names.
pub fn validate(name: &str, steps: &[StepInput]) -> Result<(String, Vec<String>), WorkflowError> {
    let name = clean_name(name, "The workflow")?;
    if steps.is_empty() {
        return invalid("A workflow needs at least one step.");
    }
    if steps.len() > MAX_STEPS {
        return invalid(format!("A workflow has at most {MAX_STEPS} steps."));
    }
    let mut names = Vec::new();
    let mut edges = 0;
    for (i, s) in steps.iter().enumerate() {
        names.push(clean_name(&s.name, &format!("Step {}", i + 1))?);
        if s.agent_id.is_empty()
            || s.agent_id.len() > 64
            || s.agent_id.chars().any(|c| c.is_control())
        {
            return invalid(format!("Step {} needs an agent.", i + 1));
        }
        let mut seen = std::collections::HashSet::new();
        for d in &s.depends_on {
            if *d >= steps.len() {
                return invalid(format!(
                    "Step {} depends on a step that does not exist.",
                    i + 1
                ));
            }
            if *d == i {
                return invalid(format!("Step {} cannot depend on itself.", i + 1));
            }
            if !seen.insert(*d) {
                return invalid(format!("Step {} lists the same dependency twice.", i + 1));
            }
            edges += 1;
        }
    }
    if edges > MAX_EDGES {
        return invalid(format!("A workflow has at most {MAX_EDGES} dependencies."));
    }
    // Kahn's algorithm: a cycle leaves steps that never become free.
    let mut indeg: Vec<usize> = steps.iter().map(|s| s.depends_on.len()).collect();
    let mut free: Vec<usize> = (0..steps.len()).filter(|i| indeg[*i] == 0).collect();
    let mut done = 0;
    while let Some(n) = free.pop() {
        done += 1;
        for (i, s) in steps.iter().enumerate() {
            if s.depends_on.contains(&n) {
                indeg[i] -= 1;
                if indeg[i] == 0 {
                    free.push(i);
                }
            }
        }
    }
    if done != steps.len() {
        return invalid(
            "The dependencies form a cycle: some steps would wait for each other forever.",
        );
    }
    Ok((name, names))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

impl StepState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
    /// Satisfies a dependent step.
    pub fn satisfies(self) -> bool {
        matches!(self, Self::Succeeded | Self::Skipped)
    }
    pub fn is_final(self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            _ => Self::Running,
        }
    }
}

/// Short fixed reasons a step ended up failed. Never process output.
pub mod note {
    pub const AGENT_NOT_FOUND: &str = "agent not found";
    pub const APPROVAL_MISSING: &str = "approval missing";
    pub const COULD_NOT_START: &str = "could not start";
    pub const EXITED: &str = "exited with an error";
    pub const INTERRUPTED: &str = "interrupted";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub id: String,
    pub position: i64,
    pub name: String,
    pub agent_id: String,
    pub state: StepState,
    pub depends_on: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub exit_code: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub workflow_id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub state: RunState,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub steps: Vec<RunStep>,
}

impl Run {
    /// Steps that may start now: pending, every dependency satisfied.
    pub fn ready(&self) -> Vec<&RunStep> {
        self.steps
            .iter()
            .filter(|s| s.state == StepState::Pending)
            .filter(|s| {
                s.depends_on.iter().all(|d| {
                    self.steps
                        .iter()
                        .find(|x| &x.id == d)
                        .is_some_and(|x| x.state.satisfies())
                })
            })
            .collect()
    }

    /// The state the run should be in given its steps, or `None` while it is still going.
    pub fn settled(&self) -> Option<RunState> {
        if self.steps.iter().any(|s| s.state == StepState::Running) || !self.ready().is_empty() {
            return None;
        }
        if self.steps.iter().all(|s| s.state.satisfies()) {
            return Some(RunState::Succeeded);
        }
        Some(RunState::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(name: &str, deps: &[usize]) -> StepInput {
        StepInput {
            name: name.into(),
            agent_id: "claude-code".into(),
            depends_on: deps.to_vec(),
        }
    }

    #[test]
    fn a_valid_diamond_passes_and_names_are_cleaned() {
        let steps = [
            st(" a\n", &[]),
            st("b", &[0]),
            st("c", &[0]),
            st("d", &[1, 2]),
        ];
        let (name, names) = validate("  Ship \u{7}it ", &steps).unwrap();
        assert_eq!(name, "Ship it");
        assert_eq!(names, ["a", "b", "c", "d"]);
    }

    #[test]
    fn bad_shapes_are_refused_with_a_reason() {
        let msg = |r: Result<_, WorkflowError>| r.unwrap_err().to_string();
        assert!(msg(validate("", &[st("a", &[])])).contains("needs a name"));
        assert!(msg(validate("w", &[])).contains("at least one step"));
        assert!(msg(validate("w", &[st("a", &[0])])).contains("itself"));
        assert!(msg(validate("w", &[st("a", &[5])])).contains("does not exist"));
        assert!(msg(validate("w", &[st("a", &[]), st("b", &[0, 0])])).contains("twice"));
        assert!(msg(validate("w", &[st("a", &[1]), st("b", &[0])])).contains("cycle"));
        assert!(msg(validate(
            "w",
            &[st("a", &[1]), st("b", &[2]), st("c", &[0])]
        ))
        .contains("cycle"));
        assert!(msg(validate(&"x".repeat(61), &[st("a", &[])])).contains("at most 60"));
        let long_step = msg(validate("w", &[st("ok", &[]), st(&"y".repeat(75), &[0])]));
        assert_eq!(
            long_step,
            "Step 2: the name is 75 characters long, at most 60 are allowed."
        );
        let many: Vec<_> = (0..25).map(|i| st(&format!("s{i}"), &[])).collect();
        assert!(msg(validate("w", &many)).contains("at most 24"));
        let mut s = st("a", &[]);
        s.agent_id = String::new();
        assert!(msg(validate("w", &[s])).contains("needs an agent"));
    }

    fn rs(id: &str, state: StepState, deps: &[&str]) -> RunStep {
        RunStep {
            id: id.into(),
            position: 0,
            name: id.into(),
            agent_id: "a".into(),
            state,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            note: None,
        }
    }

    fn run(steps: Vec<RunStep>) -> Run {
        Run {
            id: "r".into(),
            workflow_id: None,
            workspace_id: "w".into(),
            name: "r".into(),
            state: RunState::Running,
            created_at: String::new(),
            finished_at: None,
            steps,
        }
    }

    #[test]
    fn ready_waits_for_every_dependency_and_skipped_counts_but_failed_blocks() {
        use StepState::*;
        let r = run(vec![
            rs("a", Succeeded, &[]),
            rs("b", Skipped, &[]),
            rs("c", Pending, &["a", "b"]),
            rs("d", Pending, &["a", "x"]),
            rs("x", Failed, &[]),
            rs("e", Pending, &["x"]),
        ]);
        let ready: Vec<_> = r.ready().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ready, ["c"], "d and e are blocked by the failed step");
    }

    #[test]
    fn a_run_settles_only_when_nothing_runs_and_nothing_can_start() {
        use StepState::*;
        assert_eq!(run(vec![rs("a", Running, &[])]).settled(), None);
        assert_eq!(
            run(vec![rs("a", Succeeded, &[]), rs("b", Pending, &["a"])]).settled(),
            None
        );
        assert_eq!(
            run(vec![rs("a", Succeeded, &[]), rs("b", Skipped, &["a"])]).settled(),
            Some(RunState::Succeeded)
        );
        assert_eq!(
            run(vec![rs("a", Failed, &[]), rs("b", Pending, &["a"])]).settled(),
            Some(RunState::Failed)
        );
        assert_eq!(
            run(vec![rs("a", Cancelled, &[])]).settled(),
            Some(RunState::Failed)
        );
    }
}
