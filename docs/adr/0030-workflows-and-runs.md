# ADR 0030 — Reusable workflows and runs (orchestration stage 8)
**Status:** Proposed (maintainer to accept; the three decisions were answered on 2026-11-23, see the end). **Design only; no code.** Implements stage 8 of ADR 0022 (§4.8) and the requirements ORCH-005 and WF-001, and builds the Task/Run model that ADR 0027 §6 moved here. It adds one approval class (`run_workflow`) and one scope (`run`), which the gate reserved a column for (`approvals.scope_ref`).

## Context
Stages 1–7 give Naurix agent sessions that survive the view, one approval gate, an agent limit, opt-in worktrees and opt-in status signals. What is missing is the "reusable workflows" principle of ADR 0022: a saved set of agent runs and dependencies that can be started many times.

Constraints that shape the design (all from earlier decisions):
- **Metadata only** (ADR 0022 §4.5): ids, user-typed names, dependency edges, timestamps and states. **No prompts, no output**, and no automated input to an agent (ADR 0022 §1, the maintainer's answer below).
- **State is never guessed** (ADR 0022 §1.5): a step's state comes from the process or from a verified signal, never from output heuristics or quiet timers.
- **Every step passes the approval gate** (ADR 0027), grants are bound to a fingerprint and revocable, and there is no "approve everything" scope.
- **Sessions already exist** and already record kind, adapter, approval and worktree (ADR 0023, 0027). A run must not duplicate them.
- The agent limit (ADR 0027) applies to workflow steps like any other agent launch.

## Decision

### 1. Vocabulary and data model
- **Workflow**: a named template in one workspace. **Step**: one node, "run agent X" (a preset or a custom agent, including a custom agent that is just a command such as `npm test`). **Dependency**: step B waits for step A. There is **no separate Task entity**: a step is the task, and a one-off task is a one-step workflow. (ORCH-005's "Task" is satisfied by steps; a standalone task type would have nothing to add.)
- **Run**: one execution of a workflow. **Run step**: that step's state in this run.
- Migration 6, append-only:
  - `workflows(id, workspace_id → workspaces ON DELETE CASCADE, name, created_at, updated_at)`
  - `workflow_steps(id, workflow_id → workflows CASCADE, position, name, agent_id)`
  - `workflow_deps(step_id → workflow_steps CASCADE, depends_on → workflow_steps CASCADE, PRIMARY KEY both)`
  - `runs(id, workflow_id → workflows ON DELETE SET NULL, workspace_id → workspaces CASCADE, name, state, created_at, finished_at)` with the workflow name copied in, so a deleted workflow leaves a readable history
  - `run_steps(id, run_id → runs CASCADE, position, name, agent_id, state, started_at, finished_at, exit_code, note)`
  - `run_deps(run_step_id → run_steps CASCADE, depends_on → run_steps CASCADE)`
- **A run is a snapshot**: it copies the steps and edges when it starts, so editing a workflow never changes a run in progress. A workflow cannot be edited or deleted while one of its runs is active.
- Names are user-typed text, length-limited and stripped of control characters (as titles are). `agent_id` is an id, never a path or an argument. `note` is a short fixed-vocabulary reason (for example `agent not found`, `approval missing`, `agent limit`), never process output. The runtime session id is held **in memory** (sessions do not survive a restart, ADR 0026); the database row records no terminal content.
- **Validation on save:** at most 24 steps, 64 edges, names 1–60 characters, every dependency inside the same workflow, no self-dependency, **no cycle** (checked by a topological sort, in Core, not the UI). An unknown agent id is allowed to be saved (it may be installed later) and is reported when the run is reviewed.

### 2. States, and when a step is done (maintainer decision 2)
Step states: `pending`, `running`, `succeeded`, `failed`, `skipped`, `cancelled`. Run states: `running`, `succeeded`, `failed`, `cancelled`, `interrupted`.
- A step becomes **running** when Core starts its session. It becomes **succeeded** when its process **exits with code 0**, and **failed** on any other exit, on a launch error, or on a signal exit.
- Because an interactive agent does not exit by itself, the user can press **Mark done** on a running step (it becomes `succeeded`; the session is left running and is not touched) or **Skip** on a pending step (it becomes `skipped`).
- **Dependents start when every dependency is `succeeded` or `skipped`.** A step with a `failed` or `cancelled` dependency stays `pending` and is shown as **blocked**; it never starts by itself. Skipping is the user's explicit choice to proceed without that step.
- A run is `succeeded` when every step is `succeeded` or `skipped`, `failed` when nothing is running and nothing can start but some step failed, and `cancelled` when the user cancels (pending steps become `cancelled`; running sessions are **left alone**, they are ordinary sessions the user can close).
- **No status signal and no output pattern completes a step.** A verified `turn_done` signal (ADR 0029) does **not** finish a step in this stage; it would not mean the work is complete. It stays a badge.
- **A restart marks every `running` run `interrupted`** and its running steps `failed` with the note `interrupted`, as sessions are marked today. Nothing resumes by itself.

### 3. Approval: one reviewed grant per run (maintainer decision 1)
- A new action class `run_workflow`, **once only**. Its review lists **every step's exact launch command** (the same text a single launch shows: program, arguments, extra environment, status hooks if enabled), the dependency edges, and whether each agent is already approved. Its fingerprint covers the workspace, the workflow's steps in order, each step's agent id, resolved program path and argument array, and the edges. A change to any of them asks again.
- On approval Core **consumes** that once-grant and creates a **run-scoped grant** (`scope='run'`, `scope_ref=<run id>`) for each distinct `launch_agent` action in the run. Step launches go through the same `authorize` as every launch; a run-scoped grant matches only a launch made **by that run** (the gate takes the run id; an ordinary launch can never use it). Grants are listed under Settings, Security like any other and can be revoked, which stops further steps from starting.
- Run-scoped grants end when the run ends (succeeded, failed, cancelled or interrupted) and are revoked by Core then; nothing about them outlives the run. A launch that would need an approval the run did not cover (for example an agent whose program path changed after review) **fails that step with `approval missing`**; it never asks in the middle of a run and never widens the scope.
- Worktrees are **not part of workflows in this stage**. Creating one is a Git mutation approved "once", which cannot be pre-approved for a run. A step runs in the workspace folder; the user can still launch a worktree agent by hand.
- `workspace` and `app` scopes and the session scope are unchanged; a `session` scope is not added (nothing needs it).

### 4. Content of a step (maintainer decision 3)
A step is **only a reference to an agent**. It stores no prompt and sends no input; the user types in the agent's terminal as for any agent. A step that needs fixed arguments uses a **custom agent** (already a reviewed, stored definition, ADR 0011). A per-step user-authored prompt is a separate, later decision: it needs an exception to the metadata-first rule and a verified "prompt as launch argument" capability per agent, so it gets its own ADR if wanted.

### 5. Execution
- A `workflow` module in Core holds the model, the store and an **engine**. The engine owns no processes: it asks the session manager to start sessions through a launcher supplied by the shell (the existing agent launch path: executable resolution, MCP environment, status hooks), so the Terminal Runtime still knows nothing about workflows (ORCH-001).
- Step sessions start **detached** (no output consumer); output goes to the bounded in-memory ring like any session. The step row offers **Open terminal**, which attaches a pane to that session. Nothing is shown in the sidebar or tabs until the user opens it.
- The engine reacts to the existing `SessionExited` event. When a dependency finishes it starts every ready step, **subject to the agent limit**: a ready step that finds no free slot stays `pending` and is retried when a session exits. It never exceeds the limit and never kills anything to make room.
- Several independent steps can run at the same time. There is no retry, no timeout and no loop in this stage.
- Run changes are announced with a metadata event (`run://changed`: run id and workspace); the UI re-reads state. Delivery is lossy by design, so the view also re-reads when it opens and when the window regains focus.

### 6. What the UI shows
A **Workflows** view per workspace: the list, an editor (name, steps, an agent picker per step, dependencies), **Run** (which opens the run review), and the run's progress: each step with its state, an *Open terminal* button while it runs, *Mark done* / *Skip* where allowed, and *Cancel run*. A step blocked by a failed dependency says so. A short note states that "Mark done" does not stop the agent, and that nothing here reads what an agent does. Nothing in the Workflows view is required to use terminals or agents.

### 7. Limits stated up front
- Steps are agents; there is no shell-command step type (use a custom agent), no conditions, loops, retries, schedules or triggers, and no parameters or variables.
- "Succeeded" for an interactive agent is the user's word (Mark done); exit code 0 is the only automatic evidence, and it means the process ended cleanly, not that its work is correct.
- A run does not survive a restart (it is marked interrupted). Windows and Linux are unvalidated as elsewhere (ADR 0021).

## Consequences
- Core gains `workflow` (model, store, validation, engine), `ActionClass::RunWorkflow`, `Scope::Run` and a run-aware `authorize`, and migration 6. Tauri gains `workflow_*` and `run_*` commands, an event bridge for `run://changed`, and the launcher it passes to the engine. The UI gains the Workflows view.
- The agent launch path in `src-tauri` is split so the same code serves a user launch (attached to a pane) and a workflow step (detached). No behaviour change for a user launch.
- Tests required: cycle, self-dependency, cross-workflow and over-limit validation; the dependency order of a diamond and of independent steps; failed and cancelled dependencies block, skipped ones do not; the agent limit delays a ready step and starts it when a slot frees; a changed program path invalidates the run approval and a step launched outside the review fails with `approval missing`; run-scoped grants authorize only that run's launches and end with the run; revoking mid-run stops further steps; a once run approval cannot be reused; editing or deleting a workflow with an active run is refused; deleting a workflow keeps the run history; a restart marks running runs interrupted; no prompt, output or path is stored (the database is inspected for a planted marker); the status of an exited session never completes a step.
- Risk: workflows start processes without a click per step. Mitigation: one reviewed approval that lists every exact command, run-scoped grants that cannot be used outside the run and are revoked at its end, the limit, no worktree or Git mutation, and no stored input.

## Decided by the maintainer (2026-11-23)
1. **Approval: one reviewed approval for the whole run.** Steps then start as dependencies finish; the grants are tied to the workflow's fingerprint and end with the run.
2. **A step is done when its process exits (0 = success, anything else = failure) or when the user presses Mark done / Skip.** No output heuristics and no signal-based completion.
3. **A step is only a reference to an agent: no stored prompts, no automated input.**
